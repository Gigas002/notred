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
