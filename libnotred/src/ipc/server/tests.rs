#[cfg(feature = "server")]
mod server_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::io::BufReader;
    use tokio::net::UnixStream;

    use crate::host::state::HostState;
    use crate::ipc::codec;
    use crate::ipc::server::Server;
    use crate::model::Notification;
    use crate::queue::Queue;
    use crate::wire::{Cmd, Event, OkPayload, Request, Response, Urgency};

    fn temp_socket(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("notred-test-{}-{}.sock", tag, std::process::id()))
    }

    fn notif(summary: &str) -> Notification {
        Notification {
            id: 0,
            replaces_id: 0,
            app_id: "app".into(),
            summary: summary.into(),
            body: String::new(),
            urgency: Urgency::Normal,
            timeout_ms: -1,
            icon: None,
            action_keys: vec!["default".into()],
            has_actions: true,
            timestamp: 0,
        }
    }

    async fn start_server(path: &std::path::Path) -> (tokio::task::JoinHandle<()>, Arc<HostState>) {
        let queue = Arc::new(Queue::new());
        let state = HostState::new(Default::default(), Arc::clone(&queue));
        let server = Server::new(path, Arc::clone(&state), None);
        let state_out = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            let _ = server.run().await;
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        (handle, state_out)
    }

    async fn connect(
        path: &std::path::Path,
    ) -> (
        BufReader<tokio::net::unix::OwnedReadHalf>,
        tokio::net::unix::OwnedWriteHalf,
    ) {
        let stream = UnixStream::connect(path).await.unwrap();
        let (r, w) = stream.into_split();
        (BufReader::new(r), w)
    }

    #[tokio::test]
    async fn ping_returns_pong() {
        let path = temp_socket("ping");
        let (handle, _) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Ping))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(resp, Response::ok(OkPayload::Pong));
    }

    #[tokio::test]
    async fn list_returns_empty_items() {
        let path = temp_socket("list");
        let (handle, _) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::List))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(resp, Response::ok(OkPayload::Items { items: vec![] }));
    }

    #[tokio::test]
    async fn subscribe_initial_empty_update() {
        let path = temp_socket("subscribe");
        let (handle, _) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Subscribe))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        drop(w);
        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            resp,
            Response::ok(OkPayload::Event {
                event: Event::Update { items: vec![] },
            })
        );
    }

    #[tokio::test]
    async fn dismiss_not_found_returns_error() {
        let path = temp_socket("dismiss-nf");
        let (handle, _) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Dismiss { id: 999 }))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert!(matches!(resp, Response::Err(_)));
    }

    #[tokio::test]
    async fn close_all_returns_ok() {
        let path = temp_socket("close-all");
        let (handle, _) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::CloseAll))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(resp, Response::ok(OkPayload::Ok));
    }

    #[tokio::test]
    async fn activate_not_found_returns_error() {
        let path = temp_socket("activate-nf");
        let (handle, _) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Activate { id: 1, key: None }))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert!(matches!(resp, Response::Err(_)));
    }

    #[tokio::test]
    async fn activate_ok_for_notification_with_default_action() {
        let path = temp_socket("activate-ok");
        let (handle, state) = start_server(&path).await;
        let id = state.queue.push(notif("x")).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Activate { id, key: None }))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(resp, Response::ok(OkPayload::Ok));
    }

    #[tokio::test]
    async fn pause_holds_notifications_from_list() {
        let path = temp_socket("pause");
        let (handle, state) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Pause))
            .await
            .unwrap();
        let _ = codec::read_response(&mut r).await.unwrap().unwrap();

        state.queue.push(notif("held")).await;

        codec::write_request(&mut w, &Request::new(Cmd::List))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(resp, Response::ok(OkPayload::Items { items: vec![] }));
    }

    #[tokio::test]
    async fn unpause_surfaces_held_notifications() {
        let path = temp_socket("unpause");
        let (handle, state) = start_server(&path).await;

        state.queue.set_paused(true).await;
        state.queue.push(notif("held")).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Unpause))
            .await
            .unwrap();
        let _ = codec::read_response(&mut r).await.unwrap().unwrap();

        codec::write_request(&mut w, &Request::new(Cmd::List))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        match resp {
            Response::Ok(ok) => match ok.payload {
                OkPayload::Items { items } => assert_eq!(items.len(), 1),
                _ => panic!("expected items"),
            },
            Response::Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[tokio::test]
    async fn reload_without_handler_returns_not_implemented() {
        let path = temp_socket("reload");
        let (handle, _) = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::Reload))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert!(matches!(resp, Response::Err(_)));
    }

    #[tokio::test]
    async fn input_dismisses_without_actions() {
        let path = temp_socket("input-dismiss");
        let (handle, state) = start_server(&path).await;
        let mut plain = notif("plain");
        plain.has_actions = false;
        plain.action_keys.clear();
        let id = state.queue.push(plain).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(
            &mut w,
            &Request::new(Cmd::Input {
                id,
                event_kind: "button_left".into(),
            }),
        )
        .await
        .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(resp, Response::ok(OkPayload::Ok));
        assert!(state.queue.is_empty().await);
    }

    #[tokio::test]
    async fn input_invalid_kind_returns_error() {
        let path = temp_socket("input-bad-kind");
        let (handle, state) = start_server(&path).await;
        let id = state.queue.push(notif("x")).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(
            &mut w,
            &Request::new(Cmd::Input {
                id,
                event_kind: "left_button_click".into(),
            }),
        )
        .await
        .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert!(matches!(resp, Response::Err(_)));
    }
}

mod golden_tests {
    use crate::wire::{Request, Response};

    #[test]
    fn golden_fixtures_from_examples() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/ipc-examples");
        for name in [
            "ping",
            "list",
            "subscribe",
            "activate",
            "input",
            "reload",
            "pause",
            "list_history",
            "remove",
        ] {
            let path = root.join(format!("{name}.jsonl"));
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("read {}: {e}", path.display());
            });
            let mut lines = text.lines().filter(|l| !l.is_empty());
            let req_line = lines.next().expect("request line");
            let resp_line = lines.next().expect("response line");
            let _: Request = serde_json::from_str(req_line).unwrap();
            let _: Response = serde_json::from_str(resp_line).unwrap();
        }
    }
}
