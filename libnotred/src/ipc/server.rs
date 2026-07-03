use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

use crate::host::state::{ActivateError, HostState, RuntimeConfig};
use crate::ipc::IpcError;
use crate::ipc::codec;
use crate::model::CloseReason;
use crate::wire::{Cmd, ErrorCode, Event, OkPayload, Request, Response};

pub struct Server {
    socket_path: PathBuf,
    state: Arc<HostState>,
    reload: Option<Arc<dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync>>,
}

impl Server {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        state: Arc<HostState>,
        reload: Option<Arc<dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync>>,
    ) -> Self {
        Self {
            socket_path: socket_path.into(),
            state,
            reload,
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
            let state = Arc::clone(&self.state);
            let reload = self.reload.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, state, reload).await {
                    tracing::warn!(%e, "IPC connection ended with error");
                }
            });
        }
    }
}

async fn handle_connection(
    stream: UnixStream,
    state: Arc<HostState>,
    reload: Option<Arc<dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync>>,
) -> Result<(), IpcError> {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);

    loop {
        let req = match codec::read_request(&mut reader).await? {
            Some(r) => r,
            None => return Ok(()),
        };

        if !dispatch(&mut reader, &mut write, req, &state, reload.as_deref()).await? {
            return Ok(());
        }
    }
}

struct CmdContext<'a> {
    state: &'a HostState,
    reload: Option<&'a (dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync)>,
}

async fn handle_cmd<W>(
    write: &mut W,
    req: Request,
    ctx: &CmdContext<'_>,
) -> Result<(), IpcError>
where
    W: AsyncWriteExt + Unpin,
{
    match req.cmd {
        Cmd::Ping => {
            codec::write_response(write, &Response::ok(OkPayload::Pong)).await?;
        }
        Cmd::List => {
            let items = ctx.state.queue.snapshot().await;
            codec::write_response(write, &Response::ok(OkPayload::Items { items })).await?;
        }
        Cmd::Dismiss { id } => {
            if ctx
                .state
                .queue
                .close(id, CloseReason::DismissedByUser)
                .await
            {
                codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
            } else {
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::NotFound, format!("notification {id} not found")),
                )
                .await?;
            }
        }
        Cmd::CloseAll => {
            ctx.state
                .queue
                .close_all(CloseReason::DismissedByUser)
                .await;
            codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
        }
        Cmd::Activate { id, key } => match ctx.state.activate(id, key).await {
            Ok(()) => {
                codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
            }
            Err(ActivateError::NotFound) => {
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::NotFound, format!("notification {id} not found")),
                )
                .await?;
            }
            Err(ActivateError::InvalidActionKey { key }) => {
                codec::write_response(
                    write,
                    &Response::err(
                        ErrorCode::InvalidRequest,
                        format!("unknown action key {key:?}"),
                    ),
                )
                .await?;
            }
        },
        Cmd::Reload => match ctx.reload {
            Some(reload) => match reload() {
                Ok(cfg) => {
                    ctx.state.apply_config(cfg).await;
                    codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
                }
                Err(msg) => {
                    codec::write_response(write, &Response::err(ErrorCode::InvalidRequest, msg))
                        .await?;
                }
            },
            None => {
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::NotImplemented, "reload not configured"),
                )
                .await?;
            }
        },
        Cmd::Pause => {
            ctx.state.queue.set_paused(true).await;
            codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
        }
        Cmd::Unpause => {
            ctx.state.queue.set_paused(false).await;
            codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
        }
        Cmd::Subscribe => {
            return Err(IpcError::UnexpectedResponse("subscribe in handle_cmd"));
        }
    }
    Ok(())
}

/// Returns `true` to keep reading requests on this connection.
async fn dispatch<R, W>(
    reader: &mut R,
    write: &mut W,
    req: Request,
    state: &Arc<HostState>,
    reload: Option<&(dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync)>,
) -> Result<bool, IpcError>
where
    R: AsyncBufReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    if matches!(req.cmd, Cmd::Subscribe) {
        run_subscribe(reader, write, state, reload).await?;
        return Ok(false);
    }

    let ctx = CmdContext { state, reload };
    handle_cmd(write, req, &ctx).await?;
    Ok(true)
}

async fn run_subscribe<R, W>(
    reader: &mut R,
    write: &mut W,
    state: &Arc<HostState>,
    reload: Option<&(dyn Fn() -> Result<RuntimeConfig, String> + Send + Sync)>,
) -> Result<(), IpcError>
where
    R: AsyncBufReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    let mut change_rx = state.queue.subscribe_changes();
    let mut reload_rx = state.subscribe_reload();

    let items = state.queue.snapshot().await;
    codec::write_response(
        write,
        &Response::ok(OkPayload::Event {
            event: Event::Update { items },
        }),
    )
    .await?;

    let mut line_buf = String::new();
    let ctx = CmdContext {
        state,
        reload,
    };

    loop {
        tokio::select! {
            result = change_rx.recv() => {
                match result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        let items = state.queue.snapshot().await;
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
            result = reload_rx.recv() => {
                match result {
                    Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
                        codec::write_response(
                            write,
                            &Response::ok(OkPayload::Event {
                                event: Event::Reload,
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
                if matches!(req.cmd, Cmd::Subscribe) {
                    codec::write_response(
                        write,
                        &Response::err(ErrorCode::InvalidRequest, "already subscribed"),
                    )
                    .await?;
                    continue;
                }
                handle_cmd(write, req, &ctx).await?;
            }
        }
    }
    Ok(())
}
