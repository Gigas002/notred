use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;

#[cfg(feature = "history")]
use crate::history::HistoryFilter;
#[cfg(feature = "history")]
use crate::host::state::HistoryError;
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

async fn handle_cmd<W>(write: &mut W, req: Request, ctx: &CmdContext<'_>) -> Result<(), IpcError>
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
        Cmd::ListHistory {
            active_only,
            app_id,
            since,
        } => {
            #[cfg(feature = "history")]
            {
                let filter = HistoryFilter {
                    active_only: active_only.unwrap_or(false),
                    app_id,
                    since,
                };
                match ctx.state.list_history(filter).await {
                    Ok(rows) => {
                        codec::write_response(write, &Response::ok(OkPayload::History { rows }))
                            .await?;
                    }
                    Err(HistoryError::Disabled) => {
                        codec::write_response(
                            write,
                            &Response::err(ErrorCode::NotImplemented, "history disabled"),
                        )
                        .await?;
                    }
                    Err(HistoryError::NotFound) => {
                        codec::write_response(
                            write,
                            &Response::err(ErrorCode::NotFound, "history not available"),
                        )
                        .await?;
                    }
                }
            }
            #[cfg(not(feature = "history"))]
            {
                let _ = (active_only, app_id, since);
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::NotImplemented, "history feature not compiled"),
                )
                .await?;
            }
        }
        Cmd::Remove { id } => {
            #[cfg(feature = "history")]
            {
                match ctx.state.remove_history(id).await {
                    Ok(()) => {
                        codec::write_response(write, &Response::ok(OkPayload::Ok)).await?;
                    }
                    Err(HistoryError::NotFound) => {
                        codec::write_response(
                            write,
                            &Response::err(
                                ErrorCode::NotFound,
                                format!("notification {id} not found"),
                            ),
                        )
                        .await?;
                    }
                    Err(HistoryError::Disabled) => {
                        codec::write_response(
                            write,
                            &Response::err(ErrorCode::NotImplemented, "history disabled"),
                        )
                        .await?;
                    }
                }
            }
            #[cfg(not(feature = "history"))]
            {
                let _ = id;
                codec::write_response(
                    write,
                    &Response::err(ErrorCode::NotImplemented, "history feature not compiled"),
                )
                .await?;
            }
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
    let ctx = CmdContext { state, reload };

    #[cfg(feature = "history")]
    {
        let mut history_rx = state.subscribe_history_changes();
        loop {
            tokio::select! {
                result = change_rx.recv() => {
                    if !handle_change_event(result, write, state).await? {
                        break;
                    }
                }
                result = reload_rx.recv() => {
                    if !handle_reload_event(result, write).await? {
                        break;
                    }
                }
                result = history_rx.recv() => {
                    if !handle_history_event(result, write).await? {
                        break;
                    }
                }
                n = reader.read_line(&mut line_buf) => {
                    if !handle_subscribe_line(n?, &mut line_buf, reader, write, &ctx).await? {
                        break;
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "history"))]
    {
        loop {
            tokio::select! {
                result = change_rx.recv() => {
                    if !handle_change_event(result, write, state).await? {
                        break;
                    }
                }
                result = reload_rx.recv() => {
                    if !handle_reload_event(result, write).await? {
                        break;
                    }
                }
                n = reader.read_line(&mut line_buf) => {
                    if !handle_subscribe_line(n?, &mut line_buf, reader, write, &ctx).await? {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn handle_change_event<W>(
    result: Result<(), broadcast::error::RecvError>,
    write: &mut W,
    state: &HostState,
) -> Result<bool, IpcError>
where
    W: AsyncWriteExt + Unpin,
{
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
            Ok(true)
        }
        Err(broadcast::error::RecvError::Closed) => Ok(false),
    }
}

async fn handle_reload_event<W>(
    result: Result<(), broadcast::error::RecvError>,
    write: &mut W,
) -> Result<bool, IpcError>
where
    W: AsyncWriteExt + Unpin,
{
    match result {
        Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
            codec::write_response(
                write,
                &Response::ok(OkPayload::Event {
                    event: Event::Reload,
                }),
            )
            .await?;
            Ok(true)
        }
        Err(broadcast::error::RecvError::Closed) => Ok(false),
    }
}

#[cfg(feature = "history")]
async fn handle_history_event<W>(
    result: Result<(), broadcast::error::RecvError>,
    write: &mut W,
) -> Result<bool, IpcError>
where
    W: AsyncWriteExt + Unpin,
{
    match result {
        Ok(()) | Err(broadcast::error::RecvError::Lagged(_)) => {
            codec::write_response(
                write,
                &Response::ok(OkPayload::Event {
                    event: Event::HistoryChanged,
                }),
            )
            .await?;
            Ok(true)
        }
        Err(broadcast::error::RecvError::Closed) => Ok(false),
    }
}

async fn handle_subscribe_line<R, W>(
    n: usize,
    line_buf: &mut String,
    _reader: &mut R,
    write: &mut W,
    ctx: &CmdContext<'_>,
) -> Result<bool, IpcError>
where
    R: AsyncBufReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    if n == 0 {
        return Ok(false);
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
            return Ok(true);
        }
    };
    line_buf.clear();
    if matches!(req.cmd, Cmd::Subscribe) {
        codec::write_response(
            write,
            &Response::err(ErrorCode::InvalidRequest, "already subscribed"),
        )
        .await?;
        return Ok(true);
    }
    handle_cmd(write, req, ctx).await?;
    Ok(true)
}

#[cfg(test)]
mod tests;
