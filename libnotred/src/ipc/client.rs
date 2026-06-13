//! In-process IPC client for `notredctl` and the `notred ping` subcommand.

use std::path::Path;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;

use crate::ipc::IpcError;
use crate::ipc::codec;
use crate::wire::{Cmd, OkPayload, Request, Response};

pub struct Client {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    write: tokio::net::unix::OwnedWriteHalf,
}

impl Client {
    pub async fn connect(socket_path: &Path) -> Result<Self, IpcError> {
        let stream = UnixStream::connect(socket_path).await?;
        let (read, write) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read),
            write,
        })
    }

    pub async fn ping(&mut self) -> Result<(), IpcError> {
        codec::write_request(&mut self.write, &Request::new(Cmd::Ping)).await?;
        match codec::read_response(&mut self.reader).await? {
            Some(Response::Ok(ok)) if ok.payload == OkPayload::Pong => Ok(()),
            _ => Err(IpcError::UnexpectedResponse("ping")),
        }
    }

    pub async fn list(&mut self) -> Result<Vec<crate::wire::MinimalNotification>, IpcError> {
        codec::write_request(&mut self.write, &Request::new(Cmd::List)).await?;
        match codec::read_response(&mut self.reader).await? {
            Some(Response::Ok(ok)) => match ok.payload {
                OkPayload::Items { items } => Ok(items),
                _ => Err(IpcError::UnexpectedResponse("list payload")),
            },
            _ => Err(IpcError::UnexpectedResponse("list")),
        }
    }

    /// Dismiss one notification by id. Returns an error if the id was not found.
    pub async fn dismiss(&mut self, id: u32) -> Result<(), IpcError> {
        codec::write_request(&mut self.write, &Request::new(Cmd::Dismiss { id })).await?;
        match codec::read_response(&mut self.reader).await? {
            Some(Response::Ok(_)) => Ok(()),
            Some(Response::Err(e)) => Err(IpcError::ServerError(e.error.message)),
            None => Err(IpcError::UnexpectedResponse("dismiss")),
        }
    }

    /// Dismiss all active notifications.
    pub async fn close_all(&mut self) -> Result<(), IpcError> {
        codec::write_request(&mut self.write, &Request::new(Cmd::CloseAll)).await?;
        match codec::read_response(&mut self.reader).await? {
            Some(Response::Ok(_)) => Ok(()),
            Some(Response::Err(e)) => Err(IpcError::ServerError(e.error.message)),
            None => Err(IpcError::UnexpectedResponse("close_all")),
        }
    }

    /// Block and invoke `on_line` for each NDJSON response line until EOF.
    pub async fn subscribe<F>(&mut self, mut on_line: F) -> Result<(), IpcError>
    where
        F: FnMut(&str) -> Result<(), IpcError>,
    {
        codec::write_request(&mut self.write, &Request::new(Cmd::Subscribe)).await?;
        loop {
            let mut line = String::new();
            let n = self.reader.read_line(&mut line).await?;
            if n == 0 {
                break;
            }
            on_line(line.trim_end())?;
        }
        Ok(())
    }
}
