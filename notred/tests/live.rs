//! End-to-end tests: the real `notred` on a private session bus.
//!
//! See `common/mod.rs` for the harness; without `dbus-daemon` these are skipped.

mod common;

use std::time::Duration;

use common::{Notify, Signal, Signals, active_ids};

#[tokio::test]
async fn notify_list_and_dismiss() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;
    let mut signals = Signals::new(&conn).await;

    let id = Notify {
        body: "<b>hi</b>",
        ..Notify::new("hello")
    }
    .send(&conn)
    .await;
    let items = notred.client().await.list().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].id, items[0].summary.as_str()), (id, "hello"));
    assert_eq!(items[0].body, "<b>hi</b>");

    notred.client().await.dismiss(id).await.unwrap();
    assert_eq!(signals.next().await, Signal::Closed { id, reason: 2 });
    assert!(active_ids(&notred).await.is_empty());
}

#[tokio::test]
async fn close_notification_emits_closed_by_call() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;
    let mut signals = Signals::new(&conn).await;

    let id = Notify::new("x").send(&conn).await;
    conn.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "CloseNotification",
        &(id,),
    )
    .await
    .unwrap();
    assert_eq!(signals.next().await, Signal::Closed { id, reason: 3 });
    // Exactly one signal, not one from the call and one from the queue.
    assert_eq!(signals.next_within(Duration::from_millis(300)).await, None);
    assert!(active_ids(&notred).await.is_empty());
}

#[tokio::test]
async fn expire_timeout_closes_as_expired() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;
    let mut signals = Signals::new(&conn).await;

    let id = Notify {
        timeout: 100,
        ..Notify::new("brief")
    }
    .send(&conn)
    .await;
    assert_eq!(signals.next().await, Signal::Closed { id, reason: 1 });
    assert!(active_ids(&notred).await.is_empty());
}

#[tokio::test]
async fn replaces_id_updates_in_place() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;

    let id = Notify::new("v1").send(&conn).await;
    let again = Notify {
        replaces: id,
        ..Notify::new("v2")
    }
    .send(&conn)
    .await;
    assert_eq!(again, id);
    let items = notred.client().await.list().await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].summary, "v2");
}

#[tokio::test]
async fn activate_with_a_key_emits_action_invoked() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;
    let mut signals = Signals::new(&conn).await;

    let id = Notify {
        actions: &["default", "Open", "reply", "Reply"],
        ..Notify::new("chat")
    }
    .send(&conn)
    .await;
    notred
        .client()
        .await
        .activate(id, Some("reply"))
        .await
        .unwrap();
    assert_eq!(
        signals.next().await,
        Signal::Action {
            id,
            key: "reply".into()
        }
    );
}

#[tokio::test]
async fn on_notify_hook_runs_with_the_notification_env() {
    let bus = bus_or_skip!();
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("hook.txt");
    let notred = common::Notred::start(
        &bus,
        &format!(
            "[events]\non_notify = [\"sh\", \"-c\", \"echo $NOTRED_APP_ID:$NOTRED_SUMMARY > {}\"]\n",
            out.display()
        ),
    )
    .await;
    let conn = bus.connect().await;
    Notify {
        app: "mail",
        ..Notify::new("new mail")
    }
    .send(&conn)
    .await;
    let text = common::wait_for("the hook", || std::fs::read_to_string(&out).ok());
    assert_eq!(text, "mail:new mail\n");
    drop(notred);
}

#[tokio::test]
async fn subscribe_streams_updates() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut client = notred.client().await;
    tokio::spawn(async move {
        let _ = client
            .subscribe(|line| {
                let _ = tx.send(line.to_owned());
                Ok(())
            })
            .await;
    });
    let first = rx.recv().await.unwrap();
    assert!(first.contains(r#""kind":"update","items":[]"#), "{first}");

    Notify::new("streamed").send(&conn).await;
    let next = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(next.contains(r#""summary":"streamed""#), "{next}");
}

#[tokio::test]
async fn a_second_daemon_refuses_to_start() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_notred"))
        .env("XDG_RUNTIME_DIR", notred.path("run"))
        .env("XDG_CONFIG_HOME", notred.path("config"))
        .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
        .output()
        .unwrap();
    assert!(!out.status.success());
    // The first one keeps its socket.
    notred.client().await.ping().await.unwrap();
}

#[tokio::test]
async fn sigterm_exits_cleanly_and_removes_the_socket() {
    let bus = bus_or_skip!();
    let mut notred = common::Notred::start(&bus, "").await;
    let socket = notred.socket();
    assert!(socket.exists());
    let status = notred.terminate();
    assert!(status.success(), "{status}:\n{}", notred.log());
    assert!(!socket.exists(), "the socket is left behind");
}

#[tokio::test]
async fn clicking_without_a_default_action_dismisses() {
    // Chat apps often offer only named actions (e.g. "reply"). poshanka's
    // left click is `activate <id>` with no key.
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;
    let mut signals = Signals::new(&conn).await;
    let id = Notify {
        actions: &["reply", "Reply"],
        ..Notify::new("chat")
    }
    .send(&conn)
    .await;
    notred.client().await.activate(id, None).await.unwrap();
    assert_eq!(signals.next().await, Signal::Closed { id, reason: 2 });
    assert!(active_ids(&notred).await.is_empty());
}

#[tokio::test]
async fn an_invoked_action_closes_the_notification_unless_resident() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;
    let mut signals = Signals::new(&conn).await;

    let id = Notify {
        actions: &["default", "Open"],
        ..Notify::new("open me")
    }
    .send(&conn)
    .await;
    notred.client().await.activate(id, None).await.unwrap();
    let key = "default".to_owned();
    assert_eq!(signals.next().await, Signal::Action { id, key });
    assert_eq!(signals.next().await, Signal::Closed { id, reason: 2 });
    assert!(active_ids(&notred).await.is_empty());

    let mut notify = Notify {
        actions: &["default", "Open"],
        ..Notify::new("resident")
    };
    notify.hints.insert("resident", zbus::Variant::Bool(true));
    let id = notify.send(&conn).await;
    notred.client().await.activate(id, None).await.unwrap();
    let key = "default".to_owned();
    assert_eq!(signals.next().await, Signal::Action { id, key });
    assert_eq!(signals.next_within(Duration::from_millis(300)).await, None);
    assert_eq!(active_ids(&notred).await, vec![id]);
}

#[tokio::test]
async fn paused_notifications_keep_their_timeout_until_shown() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let conn = bus.connect().await;
    notred.client().await.pause().await.unwrap();
    let id = Notify {
        timeout: 200,
        ..Notify::new("while away")
    }
    .send(&conn)
    .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    notred.client().await.unpause().await.unwrap();
    assert_eq!(active_ids(&notred).await, vec![id]);
    let mut signals = Signals::new(&conn).await;
    assert_eq!(signals.next().await, Signal::Closed { id, reason: 1 });
}

#[cfg(feature = "history")]
#[tokio::test]
async fn kept_history_survives_a_restart() {
    let bus = bus_or_skip!();
    let config = "[history]\nenabled = true\nflush = false\nmax_entries = 0\n";
    let mut notred = common::Notred::start(&bus, config).await;
    let conn = bus.connect().await;
    Notify::new("before restart").send(&conn).await;
    notred.restart(&bus).await;
    let conn = bus.connect().await;
    Notify::new("after restart").send(&conn).await;

    let rows = notred
        .client()
        .await
        .list_history(None, None, None)
        .await
        .unwrap();
    let mut summaries: Vec<_> = rows.iter().map(|r| r.summary.as_str()).collect();
    summaries.sort_unstable();
    assert_eq!(summaries, ["after restart", "before restart"]);
}

#[tokio::test]
async fn config_mistakes_are_logged_and_a_broken_reload_is_refused() {
    let bus = bus_or_skip!();
    let notred = common::Notred::start(
        &bus,
        "[paths]\noverrides = [\"apps/missing/config.toml\"]\n[queue]\ndefault_timeout = 5000\n",
    )
    .await;
    notred
        .wait_for_log("unknown key `queue.default_timeout` ignored")
        .await;
    notred.wait_for_log("apps/missing/config.toml").await;

    let err = notred.client().await.reload().await.unwrap_err();
    assert!(
        err.to_string().contains("apps/missing/config.toml"),
        "{err}"
    );
    // A good config reloads.
    notred.write_config("notred.toml", "[queue]\nmax_visible = 1\n");
    notred.client().await.reload().await.unwrap();
}

#[tokio::test]
async fn a_malformed_request_gets_invalid_request_and_the_connection_stays() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let bus = bus_or_skip!();
    let notred = common::Notred::start(&bus, "").await;
    let stream = tokio::net::UnixStream::connect(notred.socket())
        .await
        .unwrap();
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    for bad in ["{\"v\":1,\"cmd\":\"dismis\",\"id\":1}\n", "not json\n"] {
        write.write_all(bad.as_bytes()).await.unwrap();
        let reply = lines.next_line().await.unwrap().expect("a reply");
        assert!(
            reply.contains("\"code\":\"INVALID_REQUEST\""),
            "{bad} -> {reply}"
        );
    }
    write
        .write_all(b"{\"v\":1,\"cmd\":\"ping\"}\n")
        .await
        .unwrap();
    let reply = lines.next_line().await.unwrap().unwrap();
    assert_eq!(reply, r#"{"v":1,"type":"pong"}"#);
}
