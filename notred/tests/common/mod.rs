//! End-to-end harness: a private `dbus-daemon` and the real `notred` binary.
//!
//! Every test gets its own session bus and its own runtime, config and cache
//! directories, so it never touches the desktop's notification daemon. Without
//! `dbus-daemon` the tests are skipped, unless `NOTRED_LIVE_TESTS=1` (as in
//! CI) makes that a failure.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use libnotred::ipc::Client;
use zbus::Variant;
use zbus::export::futures_core::Stream;

/// Skip the test (return) when `dbus-daemon` is missing, unless required.
#[macro_export]
macro_rules! bus_or_skip {
    () => {
        match common::Bus::start() {
            Some(bus) => bus,
            None => return,
        }
    };
}

/// Poll `f` every 20 ms for up to 5 s.
pub fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A private session bus.
pub struct Bus {
    child: Child,
    pub address: String,
}

impl Bus {
    pub fn start() -> Option<Self> {
        let spawned = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(e) => {
                assert!(
                    std::env::var_os("NOTRED_LIVE_TESTS").is_none(),
                    "NOTRED_LIVE_TESTS is set but dbus-daemon can't run: {e}"
                );
                eprintln!("skipping: dbus-daemon not available ({e})");
                return None;
            }
        };
        let mut address = String::new();
        BufReader::new(child.stdout.take().expect("piped"))
            .read_line(&mut address)
            .expect("dbus-daemon address");
        Some(Self {
            child,
            address: address.trim().to_owned(),
        })
    }

    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .build()
            .await
            .expect("connect to the private bus")
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The `notred` daemon with private directories.
pub struct Notred {
    child: Child,
    dir: tempfile::TempDir,
}

impl Notred {
    /// Start notred with `config` as its `notred.toml`, and wait until it answers.
    pub async fn start(bus: &Bus, config: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("notred-")
            .tempdir_in("/tmp")
            .expect("tempdir");
        let config_dir = dir.path().join("config/notred");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("notred.toml"), config).unwrap();
        let mut notred = Self {
            child: Self::spawn(bus, &dir),
            dir,
        };
        notred.wait_until_ready().await;
        notred
    }

    fn spawn(bus: &Bus, dir: &tempfile::TempDir) -> Child {
        let path = |p: &str| dir.path().join(p);
        let run = path("run");
        std::fs::create_dir_all(&run).unwrap();
        let log = std::fs::File::create(path("notred.log")).unwrap();
        Command::new(env!("CARGO_BIN_EXE_notred"))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            // Under cargo llvm-cov, where to write coverage data.
            .envs(std::env::var_os("LLVM_PROFILE_FILE").map(|v| ("LLVM_PROFILE_FILE", v)))
            .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
            .env("XDG_RUNTIME_DIR", &run)
            .env("XDG_CONFIG_HOME", path("config"))
            .env("XDG_CACHE_HOME", path("cache"))
            .env("HOME", path("home"))
            .env("RUST_LOG", "debug")
            .env("NO_COLOR", "1")
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .expect("spawn notred")
    }

    async fn wait_until_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(mut client) = Client::connect(&self.socket()).await
                && client.ping().await.is_ok()
            {
                return;
            }
            if let Some(status) = self.child.try_wait().unwrap() {
                panic!("notred exited ({status}):\n{}", self.log());
            }
            assert!(
                Instant::now() < deadline,
                "notred not ready:\n{}",
                self.log()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Stop notred and start it again with the same directories.
    pub async fn restart(&mut self, bus: &Bus) {
        self.terminate();
        self.child = Self::spawn(bus, &self.dir);
        self.wait_until_ready().await;
    }

    /// Send SIGTERM and wait for notred to exit.
    pub fn terminate(&mut self) -> std::process::ExitStatus {
        let _ = Command::new("kill")
            .arg(self.child.id().to_string())
            .status();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "notred ignored SIGTERM");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn socket(&self) -> PathBuf {
        self.dir.path().join("run/notred.sock")
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// Write a file relative to notred's config directory.
    pub fn write_config(&self, rel: &str, text: &str) {
        let p = self.dir.path().join("config/notred").join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    pub async fn client(&self) -> Client {
        Client::connect(&self.socket()).await.expect("IPC connect")
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("notred.log")).unwrap_or_default()
    }

    pub async fn wait_for_log(&self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.log().contains(needle) {
            assert!(
                Instant::now() < deadline,
                "no {needle:?} in the log:\n{}",
                self.log()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Drop for Notred {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            self.terminate();
        }
    }
}

/// One FDN `Notify` call.
pub struct Notify<'a> {
    pub app: &'a str,
    pub replaces: u32,
    pub icon: &'a str,
    pub summary: &'a str,
    pub body: &'a str,
    pub actions: &'a [&'a str],
    pub hints: HashMap<&'a str, Variant<'a>>,
    pub timeout: i32,
}

impl<'a> Notify<'a> {
    pub fn new(summary: &'a str) -> Self {
        Self {
            app: "test",
            replaces: 0,
            icon: "",
            summary,
            body: "",
            actions: &[],
            hints: HashMap::new(),
            timeout: 0,
        }
    }

    pub async fn send(&self, conn: &zbus::Connection) -> u32 {
        conn.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "Notify",
            &(
                self.app,
                self.replaces,
                self.icon,
                self.summary,
                self.body,
                self.actions,
                &self.hints,
                self.timeout,
            ),
        )
        .await
        .expect("Notify")
        .body()
        .deserialize()
        .expect("Notify reply")
    }
}

/// Records `NotificationClosed` and `ActionInvoked` signals.
pub struct Signals {
    stream: zbus::MessageStream,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    Closed { id: u32, reason: u32 },
    Action { id: u32, key: String },
}

impl Signals {
    pub async fn new(conn: &zbus::Connection) -> Self {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .interface("org.freedesktop.Notifications")
            .build()
            .unwrap();
        let stream = zbus::MessageStream::for_match_rule(rule, conn, None)
            .await
            .expect("signal subscription");
        Self { stream }
    }

    /// The next signal, or `None` after `timeout` without one.
    pub async fn next_within(&mut self, timeout: Duration) -> Option<Signal> {
        let next = std::future::poll_fn(|cx| Pin::new(&mut self.stream).poll_next(cx));
        let msg = tokio::time::timeout(timeout, next).await.ok()??.ok()?;
        let header = msg.header();
        match header.member()?.as_str() {
            "NotificationClosed" => {
                let (id, reason): (u32, u32) = msg.body().deserialize().ok()?;
                Some(Signal::Closed { id, reason })
            }
            "ActionInvoked" => {
                let (id, key): (u32, String) = msg.body().deserialize().ok()?;
                Some(Signal::Action { id, key })
            }
            _ => None,
        }
    }

    pub async fn next(&mut self) -> Signal {
        self.next_within(Duration::from_secs(5))
            .await
            .expect("a signal within 5 s")
    }
}

/// Ids of the active notifications, in queue order.
pub async fn active_ids(notred: &Notred) -> Vec<u32> {
    notred
        .client()
        .await
        .list()
        .await
        .expect("list")
        .iter()
        .map(|n| n.id)
        .collect()
}

pub fn exists(p: &Path) -> bool {
    p.exists()
}
