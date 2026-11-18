//! `notredctl` end to end against an in-process libnotred IPC server (no D-Bus).

use std::process::{Command, Output};
use std::sync::Arc;

use libnotred::host::state::{HostState, RuntimeConfig};
use libnotred::ipc::Server;
use libnotred::model::Notification;
use libnotred::queue::Queue;
use libnotred::wire::Urgency;

struct Daemon {
    _dir: tempfile::TempDir,
    socket: std::path::PathBuf,
    state: Arc<HostState>,
}

async fn daemon() -> Daemon {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("notred.sock");
    let state = HostState::new(RuntimeConfig::default(), Arc::new(Queue::new()));
    let server = Server::new(&socket, Arc::clone(&state), None);
    tokio::spawn(async move { server.run().await });
    for _ in 0..250 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Daemon {
        _dir: dir,
        socket,
        state,
    }
}

fn notif(summary: &str, actions: &[&str]) -> Notification {
    Notification {
        id: 0,
        replaces_id: 0,
        app_id: "app".into(),
        summary: summary.into(),
        body: String::new(),
        urgency: Urgency::Normal,
        timeout_ms: 0,
        icon: None,
        action_keys: actions.iter().map(|a| (*a).to_owned()).collect(),
        has_actions: !actions.is_empty(),
        timestamp: 0,
        value: None,
        category: None,
        desktop_entry: None,
        body_markup: false,
        resident: false,
    }
}

/// Run notredctl off the async runtime's threads.
async fn ctl(d: &Daemon, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_notredctl"));
    cmd.arg("--socket").arg(&d.socket).args(args);
    tokio::task::spawn_blocking(move || cmd.output().unwrap())
        .await
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn ping_prints_pong() {
    let d = daemon().await;
    let out = ctl(&d, &["ping"]).await;
    assert!(out.status.success());
    assert_eq!(stdout(&out), "pong\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn list_prints_json_and_close_removes() {
    let d = daemon().await;
    let id = d.state.push_notification(notif("hello", &[])).await;
    let out = ctl(&d, &["list"]).await;
    assert!(out.status.success());
    let items: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(items[0]["summary"], "hello");

    let out = ctl(&d, &["close", &id.to_string()]).await;
    assert_eq!(stdout(&out), format!("closed {id}\n"));
    assert!(d.state.queue.is_empty().await);

    let out = ctl(&d, &["close", &id.to_string()]).await;
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not found"));
    assert_eq!(stdout(&out), "", "errors stay off stdout");
}

#[tokio::test(flavor = "multi_thread")]
async fn close_all_pause_unpause() {
    let d = daemon().await;
    d.state.push_notification(notif("a", &[])).await;
    assert_eq!(stdout(&ctl(&d, &["pause"]).await), "paused\n");
    d.state.push_notification(notif("held", &[])).await;
    assert_eq!(d.state.queue.len().await, 1);
    assert_eq!(stdout(&ctl(&d, &["unpause"]).await), "unpaused\n");
    assert_eq!(d.state.queue.len().await, 2);
    assert_eq!(stdout(&ctl(&d, &["close-all"]).await), "closed all\n");
    assert!(d.state.queue.is_empty().await);
}

#[tokio::test(flavor = "multi_thread")]
async fn activate_and_input() {
    let d = daemon().await;
    let id = d.state.push_notification(notif("chat", &["reply"])).await;
    let ok = ctl(&d, &["activate", &id.to_string(), "reply"]).await;
    assert_eq!(stdout(&ok), format!("activated {id}\n"));
    let bad = ctl(&d, &["activate", &id.to_string(), "nope"]).await;
    assert_eq!(bad.status.code(), Some(1));
    let bad_kind = ctl(&d, &["input", &id.to_string(), "wiggle"]).await;
    assert_eq!(bad_kind.status.code(), Some(1));
    let right = ctl(&d, &["input", &id.to_string(), "button_right"]).await;
    assert_eq!(stdout(&right), format!("input {id} button_right\n"));
    assert!(d.state.queue.is_empty().await);
}

#[tokio::test(flavor = "multi_thread")]
async fn reload_without_a_config_is_an_error() {
    let d = daemon().await;
    let out = ctl(&d, &["reload"]).await;
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("reload not configured"));
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribe_prints_the_first_update() {
    let d = daemon().await;
    d.state.push_notification(notif("streamed", &[])).await;
    let mut child = Command::new(env!("CARGO_BIN_EXE_notredctl"))
        .arg("--socket")
        .arg(&d.socket)
        .arg("subscribe")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let line = tokio::task::spawn_blocking({
        let out = child.stdout.take().unwrap();
        move || {
            let mut line = String::new();
            std::io::BufRead::read_line(&mut std::io::BufReader::new(out), &mut line).unwrap();
            line
        }
    })
    .await
    .unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert!(line.contains(r#""summary":"streamed""#), "{line}");
}

#[test]
fn an_unreachable_daemon_is_reported() {
    let out = Command::new(env!("CARGO_BIN_EXE_notredctl"))
        .args(["--socket", "/nonexistent/notred.sock", "ping"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot reach notred daemon"));
}
