//! IPC command dispatch.

use std::io::{self, Write};

use crate::error::CtlError;
use crate::settings::Settings;

pub async fn run(settings: Settings) -> Result<(), CtlError> {
    let Settings {
        socket_path,
        command,
    } = settings;

    let mut client = libnotred::ipc::Client::connect(&socket_path)
        .await
        .map_err(|_| CtlError::DaemonUnreachable(socket_path.display().to_string()))?;

    match command {
        crate::cli::Command::Ping => {
            client.ping().await?;
            println!("pong");
        }
        crate::cli::Command::List => {
            let items = client.list().await?;
            let json = serde_json::to_string_pretty(&items).map_err(CtlError::Json)?;
            println!("{json}");
        }
        crate::cli::Command::Subscribe => {
            client
                .subscribe(|line| {
                    let mut out = io::stdout().lock();
                    writeln!(out, "{line}").map_err(libnotred::IpcError::from)?;
                    out.flush().map_err(libnotred::IpcError::from)?;
                    Ok(())
                })
                .await?;
        }
        crate::cli::Command::Close { id } => {
            client.dismiss(id).await.map_err(map_server_err)?;
            println!("closed {id}");
        }
        crate::cli::Command::CloseAll => {
            client.close_all().await?;
            println!("closed all");
        }
        crate::cli::Command::Reload => {
            client.reload().await.map_err(map_server_err)?;
            println!("reloaded");
        }
        crate::cli::Command::Pause => {
            client.pause().await.map_err(map_server_err)?;
            println!("paused");
        }
        crate::cli::Command::Unpause => {
            client.unpause().await.map_err(map_server_err)?;
            println!("unpaused");
        }
        crate::cli::Command::Activate { id, key } => {
            client
                .activate(id, key.as_deref())
                .await
                .map_err(map_server_err)?;
            println!("activated {id}");
        }
        crate::cli::Command::Input { id, event_kind } => {
            client
                .input(id, &event_kind)
                .await
                .map_err(map_server_err)?;
            println!("input {id} {event_kind}");
        }
        #[cfg(feature = "history")]
        crate::cli::Command::ListHistory => {
            let rows = client
                .list_history(None, None, None)
                .await
                .map_err(map_server_err)?;
            let json = serde_json::to_string_pretty(&rows).map_err(CtlError::Json)?;
            println!("{json}");
        }
        #[cfg(feature = "history")]
        crate::cli::Command::Remove { id } => {
            client.remove(id).await.map_err(map_server_err)?;
            println!("removed {id}");
        }
    }
    Ok(())
}

fn map_server_err(e: libnotred::IpcError) -> CtlError {
    match e {
        libnotred::IpcError::ServerError(msg) => CtlError::Server(msg),
        other => CtlError::Ipc(other),
    }
}

#[cfg(test)]
mod tests;
