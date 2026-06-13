use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

use crate::ipc::IpcError;
use crate::ipc::codec;
use crate::model::CloseReason;
use crate::queue::Queue;
use crate::wire::{Cmd, ErrorCode, Event, OkPayload, Request, Response};

pub struct Server {
    socket_path: PathBuf,
    queue: Arc<Queue>,
}

impl Server {
    pub fn new(socket_path: impl Into<PathBuf>, queue: Arc<Queue>) -> Self {
        Self {
            socket_path: socket_path.into(),
            queue,
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
            let queue = Arc::clone(&self.queue);
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, queue).await {
                    tracing::warn!(%e, "IPC connection ended with error");
                }
            });
        }
    }
}

async fn handle_connection(stream: UnixStream, queue: Arc<Queue>) -> Result<(), IpcError> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);

    loop {
        let req = match codec::read_request(&mut reader).await? {
            Some(r) => r,
            None => return Ok(()),
        };

        if !dispatch(&mut reader, &mut write, req, &queue).await? {
            return Ok(());
        }
    }
}

/// Returns `true` to keep reading requests on this connection.
async fn dispatch<R, W>(
    reader: &mut R,
    write: &mut W,
    req: Request,
    queue: &Arc<Queue>,
) -> Result<bool, IpcError>
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
            let items = queue.snapshot().await;
            codec::write_response(write, &Response::ok(OkPayload::Items { items })).await?;
            Ok(true)
        }
        Cmd::Dismiss { id } => {
            if queue.close(id, CloseReason::DismissedByUser).await {
                codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
            } else {
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::NotFound, format!("notification {id} not found")),
                )
                .await?;
            }
            Ok(true)
        }
        Cmd::CloseAll => {
            queue.close_all(CloseReason::DismissedByUser).await;
            codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
            Ok(true)
        }
        Cmd::Subscribe => {
            run_subscribe(reader, write, queue).await?;
            Ok(false)
        }
    }
}

async fn run_subscribe<R, W>(
    reader: &mut R,
    write: &mut W,
    queue: &Arc<Queue>,
) -> Result<(), IpcError>
where
    R: AsyncBufReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let mut rx = queue.subscribe_changes();

    // Send initial snapshot.
    let items = queue.snapshot().await;
    codec::write_response(
        write,
        &Response::ok(OkPayload::Event {
            event: Event::Update { items },
        }),
    )
    .await?;

    let mut line_buf = String::new();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        let items = queue.snapshot().await;
                        codec::write_response(
                            write,
                            &Response::ok(OkPayload::Event {
                                event: Event::Update { items },
                            }),
                        )
                        .await?;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            n = reader.read_line(&mut line_buf) => {
                let n = n?;
                if n == 0 {
                    break;
                }
                let req: Request = match serde_json::from_str(line_buf.trim()) {
                    Ok(r) => r,
                    Err(e) => {
                        codec::write_response(
                            write,
                            &Response::err(ErrorCode::InvalidRequest, e.to_string()),
                        )
                        .await?;
                        line_buf.clear();
                        continue;
                    }
                };
                line_buf.clear();
                match req.cmd {
                    Cmd::Ping => {
                        codec::write_response(write, &Response::ok(OkPayload::Pong)).await?;
                    }
                    Cmd::List => {
                        let items = queue.snapshot().await;
                        codec::write_response(
                            write,
                            &Response::ok(OkPayload::Items { items }),
                        )
                        .await?;
                    }
                    Cmd::Subscribe => {
                        codec::write_response(
                            write,
                            &Response::err(ErrorCode::InvalidRequest, "already subscribed"),
                        )
                        .await?;
                    }
                    Cmd::Dismiss { id } => {
                        if queue.close(id, CloseReason::DismissedByUser).await {
                            codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
                        } else {
                            codec::write_response(
                                write,
                                &Response::err(
                                    ErrorCode::NotFound,
                                    format!("notification {id} not found"),
                                ),
                            )
                            .await?;
                        }
                    }
                    Cmd::CloseAll => {
                        queue.close_all(CloseReason::DismissedByUser).await;
                        codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
                    }
                }
            }
        }
    }
    Ok(())
}
