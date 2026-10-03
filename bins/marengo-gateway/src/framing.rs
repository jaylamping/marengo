use std::io;

use armee_proto::prost::Message;
use bytes::{BufMut, BytesMut};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::broadcast;

pub const MAX_FRAME: usize = 4 * 1024 * 1024;
pub const CHAPPE_STREAM_CONTENT_TYPE: &str = "application/vnd.marengo.chappe-stream";

pub fn encode_length_prefixed(payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

pub async fn write_length_prefixed<W: AsyncWrite + Unpin>(
    writer: &mut W,
    payload: &[u8],
) -> Result<(), io::Error> {
    writer.write_all(&encode_length_prefixed(payload)).await
}

pub async fn read_length_prefixed_quinn(
    recv: &mut web_transport_quinn::RecvStream,
    maximum: usize,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len == 0 || len > maximum {
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
    while let Some(out) = next_stream_envelope(&mut rx, topics).await {
        if write_length_prefixed(writer, &out).await.is_err() {
            break;
        }
    }
    Ok(())
}

/// Shared HTTP/WebTransport loss boundary; invalidate before fresh observations.
pub async fn next_stream_envelope(
    rx: &mut broadcast::Receiver<(String, Vec<u8>)>,
    topics: &[String],
) -> Option<Vec<u8>> {
    loop {
        match rx.recv().await {
            Ok((topic, payload)) => {
                if topics.iter().any(|t| t == &topic) {
                    if let Ok(env) = armee_proto::Envelope::decode(payload.as_slice()) {
                        return Some(env.encode_to_vec());
                    }
                }
            }
            Err(broadcast::error::RecvError::Lagged(lagged_envelopes)) => {
                *rx = rx.resubscribe();
                return Some(
                    armee_proto::Envelope {
                        source_node: "marengo-gateway".into(),
                        message_type: "marengo.v1.RuntimeObservationGap".into(),
                        payload: armee_proto::RuntimeObservationGap { lagged_envelopes }
                            .encode_to_vec(),
                        ..Default::default()
                    }
                    .encode_to_vec(),
                );
            }
            Err(broadcast::error::RecvError::Closed) => return None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn lag_discards_old_transition_and_backlog_before_fresh_state() {
        let (tx, mut rx) = broadcast::channel(4);
        let frame = |kind: &str| {
            armee_proto::Envelope {
                message_type: kind.into(),
                ..Default::default()
            }
            .encode_to_vec()
        };
        tx.send(("transition".into(), frame("lost-transition")))
            .expect("subscriber");
        for _ in 0..8 {
            tx.send(("state".into(), frame("old-state")))
                .expect("subscriber");
        }
        let gap = next_stream_envelope(&mut rx, &["state".into()])
            .await
            .expect("gap");
        let gap = armee_proto::Envelope::decode(gap.as_slice()).expect("envelope");
        assert_eq!(gap.message_type, "marengo.v1.RuntimeObservationGap");
        assert!(
            armee_proto::RuntimeObservationGap::decode(gap.payload.as_slice())
                .expect("typed gap")
                .lagged_envelopes
                > 0
        );
        tx.send(("state".into(), frame("fresh-state")))
            .expect("subscriber");
        let next = next_stream_envelope(&mut rx, &["state".into()])
            .await
            .expect("fresh");
        assert_eq!(
            armee_proto::Envelope::decode(next.as_slice())
                .expect("envelope")
                .message_type,
            "fresh-state"
        );
    }
}
