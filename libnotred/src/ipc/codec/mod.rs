use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use crate::ipc::IpcError;
use crate::wire::{Request, Response};

pub async fn read_request<R>(reader: &mut R) -> Result<Option<Request>, IpcError>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(line.trim())?))
}

pub async fn write_response<W>(writer: &mut W, resp: &Response) -> Result<(), IpcError>
where
    W: AsyncWriteExt + Unpin,
{
    let mut json = serde_json::to_string(resp)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    Ok(())
}

pub async fn write_request<W>(writer: &mut W, req: &Request) -> Result<(), IpcError>
where
    W: AsyncWriteExt + Unpin,
{
    let mut json = serde_json::to_string(req)?;
    json.push('\n');
    writer.write_all(json.as_bytes()).await?;
    Ok(())
}

pub async fn read_response<R>(reader: &mut R) -> Result<Option<Response>, IpcError>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(line.trim())?))
}

#[cfg(test)]
mod tests;
