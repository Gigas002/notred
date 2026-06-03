use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::ipc::IpcError;
use crate::ipc::codec;
use crate::wire::{Cmd, ErrorCode, Event, OkPayload, Request, Response};

pub struct Server {
    socket_path: PathBuf,
}

impl Server {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    pub async fn run(&self) -> Result<(), IpcError> {
        let _ = std::fs::remove_file(&self.socket_path);
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(&self.socket_path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.socket_path, std::fs::Permissions::from_mode(0o600))?;
        }
        tracing::info!(path = %self.socket_path.display(), "IPC server listening");

        loop {
            let (stream, _) = listener.accept().await?;
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream).await {
                    tracing::warn!(%e, "IPC connection ended with error");
                }
            });
        }
    }
}

async fn handle_connection(stream: UnixStream) -> Result<(), IpcError> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);

    loop {
        let req = match codec::read_request(&mut reader).await? {
            Some(r) => r,
            None => return Ok(()),
        };

        if !dispatch(&mut reader, &mut write, req).await? {
            return Ok(());
        }
    }
}

/// Returns `true` to keep reading requests on this connection.
async fn dispatch<R, W>(reader: &mut R, write: &mut W, req: Request) -> Result<bool, IpcError>
where
    R: AsyncBufReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    match req.cmd {
        Cmd::Ping => {
            codec::write_response(write, &Response::ok(OkPayload::Pong)).await?;
            Ok(true)
        }
        Cmd::List => {
            codec::write_response(write, &Response::ok(OkPayload::Items { items: vec![] })).await?;
            Ok(true)
        }
        Cmd::Subscribe => {
            run_subscribe(reader, write).await?;
            Ok(false)
        }
    }
}

async fn run_subscribe<R, W>(reader: &mut R, write: &mut W) -> Result<(), IpcError>
where
    R: AsyncBufReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let initial = Response::ok(OkPayload::Event {
        event: Event::Update { items: vec![] },
    });
    codec::write_response(write, &initial).await?;

    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let extra: Request = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::InvalidRequest, e.to_string()),
                )
                .await?;
                continue;
            }
        };
        match extra.cmd {
            Cmd::Ping => {
                codec::write_response(write, &Response::ok(OkPayload::Pong)).await?;
            }
            Cmd::List => {
                codec::write_response(write, &Response::ok(OkPayload::Items { items: vec![] }))
                    .await?;
            }
            Cmd::Subscribe => {
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::InvalidRequest, "already subscribed"),
                )
                .await?;
            }
        }
    }
    Ok(())
}
