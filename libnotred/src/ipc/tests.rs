#[cfg(feature = "server")]
mod server_tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::io::BufReader;
    use tokio::net::UnixStream;

    use crate::ipc::codec;
    use crate::ipc::server::Server;
    use crate::queue::Queue;
    use crate::wire::{Cmd, Event, OkPayload, Request, Response};

    fn temp_socket(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("notred-test-{}-{}.sock", tag, std::process::id()))
    }

    async fn start_server(path: &std::path::Path) -> tokio::task::JoinHandle<()> {
        let queue = Arc::new(Queue::new());
        let server = Server::new(path, queue);
        let handle = tokio::spawn(async move {
            let _ = server.run().await;
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        handle
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
        let handle = start_server(&path).await;

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
        let handle = start_server(&path).await;

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
        let handle = start_server(&path).await;

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
        let handle = start_server(&path).await;

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
        let handle = start_server(&path).await;

        let (mut r, mut w) = connect(&path).await;
        codec::write_request(&mut w, &Request::new(Cmd::CloseAll))
            .await
            .unwrap();
        let resp = codec::read_response(&mut r).await.unwrap().unwrap();

        handle.abort();
        let _ = std::fs::remove_file(&path);

        assert_eq!(resp, Response::ok(OkPayload::Ok));
    }
}

mod golden_tests {
    use crate::wire::{Request, Response};

    #[test]
    fn golden_fixtures_from_examples() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/ipc-examples");
        for name in ["ping", "list", "subscribe"] {
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
