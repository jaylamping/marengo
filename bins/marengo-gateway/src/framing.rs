use std::io;

use armee_proto::prost::Message;
use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast;

pub const MAX_FRAME: usize = 4 * 1024 * 1024;
pub const CHAPPE_STREAM_CONTENT_TYPE: &str = "application/vnd.marengo.chappe-stream";

pub fn encode_length_prefixed(payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

#[allow(dead_code)]
pub async fn read_length_prefixed<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Vec<u8>, io::Error> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(buf)
}

pub async fn write_length_prefixed<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<(), io::Error> {
    writer.write_all(&encode_length_prefixed(payload)).await
}

pub async fn read_length_prefixed_quinn(
    recv: &mut web_transport_quinn::RecvStream,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err("frame too large".into());
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await?;
    Ok(buf)
}

pub async fn write_length_prefixed_quinn(
    send: &mut web_transport_quinn::SendStream,
    payload: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut buf = BytesMut::with_capacity(4 + payload.len());
    buf.put_u32_le(payload.len() as u32);
    buf.extend_from_slice(payload);
    send.write_chunk(buf.freeze()).await?;
    Ok(())
}

/// Fan out matching topic envelopes as length-prefixed protobuf bytes.
pub async fn pump_envelope_stream<W: AsyncWrite + Unpin>(
    mut rx: broadcast::Receiver<(String, Vec<u8>)>,
    topics: &[String],
    writer: &mut W,
) -> Result<(), io::Error> {
    loop {
        match rx.recv().await {
            Ok((topic, payload)) => {
                if !topics.iter().any(|t| t == &topic) {
                    continue;
                }
                let envelope = match armee_proto::Envelope::decode(payload.as_slice()) {
                    Ok(env) => env,
                    Err(_) => continue,
                };
                let out = envelope.encode_to_vec();
                if write_length_prefixed(writer, &out).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
    Ok(())
}
