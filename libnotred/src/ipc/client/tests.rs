use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

use super::Client;
use crate::ipc::IpcError;

/// A one-connection server that answers each request line with `reply`.
async fn scripted_server(reply: &'static str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.sock");
    let listener = UnixListener::bind(&path).unwrap();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        while let Ok(Some(_)) = lines.next_line().await {
            write.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    (dir, path)
}

const ERR: &str = "{\"v\":1,\"error\":{\"code\":\"NOT_FOUND\",\"message\":\"nope\"}}\n";

#[tokio::test]
async fn list_and_ping_pass_on_the_server_error() {
    let (_dir, path) = scripted_server(ERR).await;
    let mut client = Client::connect(&path).await.unwrap();
    assert!(matches!(client.list().await, Err(IpcError::ServerError(m)) if m == "nope"));
    assert!(matches!(client.ping().await, Err(IpcError::ServerError(m)) if m == "nope"));
}

#[tokio::test]
async fn list_reads_items() {
    let (_dir, path) = scripted_server("{\"v\":1,\"type\":\"items\",\"items\":[]}\n").await;
    let mut client = Client::connect(&path).await.unwrap();
    assert!(client.list().await.unwrap().is_empty());
}
