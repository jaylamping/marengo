//! Unix-domain socket bridge between `marengo-pi` and `marengo-gateway`.
//!
//! Framing (all directions): `u8 direction` + `u32 topic_len` + topic utf-8 + `u32 len` + payload.
//! - `0` = runtime → gateway (telemetry)
//! - `1` = gateway → runtime (commands)

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::ipc_outbox::Outbox;
pub use crate::ipc_outbox::{
    ForwardOutcome, IpcQueueStats, EVENT_BYTE_CAPACITY, EVENT_CAPACITY, MAX_PAYLOAD_BYTES,
    QUEUE_BYTE_CAPACITY, QUEUE_ITEM_CAPACITY,
};
use thiserror::Error;
use tracing::{debug, error, warn};

pub const DIRECTION_RUNTIME_TO_GATEWAY: u8 = 0;
pub const DIRECTION_GATEWAY_TO_RUNTIME: u8 = 1;
const MAX_TOPIC_BYTES: usize = 128;
const MAX_FRAME_BYTES: usize = 9 + MAX_TOPIC_BYTES + MAX_PAYLOAD_BYTES;

/// Default socket path when `MARENGO_CHAPPE_SOCKET` is unset.
pub fn default_socket_path() -> PathBuf {
    PathBuf::from("/run/marengo/chappe.sock")
}

pub fn socket_path_from_env() -> Option<PathBuf> {
    std::env::var_os("MARENGO_CHAPPE_SOCKET").map(PathBuf::from)
}

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("framing: {0}")]
    Framing(String),
}

pub fn encode_frame(direction: u8, topic: &str, payload: &[u8]) -> Result<Vec<u8>, IpcError> {
    let topic_bytes = topic.as_bytes();
    if topic_bytes.len() > MAX_TOPIC_BYTES {
        return Err(IpcError::Framing("topic too long".into()));
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(IpcError::Framing("payload too long".into()));
    }
    let mut out = Vec::with_capacity(1 + 8 + topic_bytes.len() + payload.len());
    out.push(direction);
    out.extend_from_slice(&(topic_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(topic_bytes);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

pub fn decode_frame(mut data: &[u8]) -> Result<(u8, String, Vec<u8>), IpcError> {
    if data.is_empty() {
        return Err(IpcError::Framing("empty frame".into()));
    }
    let direction = data[0];
    data = &data[1..];
    let topic_len = read_u32(&mut data)? as usize;
    if topic_len > MAX_TOPIC_BYTES {
        return Err(IpcError::Framing("topic too long".into()));
    }
    if data.len() < topic_len {
        return Err(IpcError::Framing("truncated topic".into()));
    }
    let topic = std::str::from_utf8(&data[..topic_len])
        .map_err(|e| IpcError::Framing(e.to_string()))?
        .to_string();
    data = &data[topic_len..];
    let payload_len = read_u32(&mut data)? as usize;
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(IpcError::Framing("payload too long".into()));
    }
    if data.len() < payload_len {
        return Err(IpcError::Framing("truncated payload".into()));
    }
    let payload = data[..payload_len].to_vec();
    Ok((direction, topic, payload))
}

fn read_u32(data: &mut &[u8]) -> Result<u32, IpcError> {
    if data.len() < 4 {
        return Err(IpcError::Framing("truncated u32".into()));
    }
    let (head, tail) = data.split_at(4);
    *data = tail;
    Ok(u32::from_le_bytes(
        head.try_into()
            .map_err(|_| IpcError::Framing("u32".into()))?,
    ))
}

/// Non-blocking forwarder used by `marengo-pi` on publish; also ingests gateway commands.
pub struct IpcFanout {
    outbox: Arc<Outbox>,
}

impl IpcFanout {
    pub fn spawn_client(socket_path: PathBuf, bus: crate::Bus) -> Result<Arc<Self>, IpcError> {
        let outbox = Arc::new(Outbox::new());
        let worker_outbox = Arc::clone(&outbox);
        thread::Builder::new()
            .name("chappe-ipc-client".into())
            .spawn(move || ipc_client_loop(socket_path, worker_outbox, bus))
            .map_err(IpcError::Io)?;
        Ok(Arc::new(Self { outbox }))
    }

    pub fn forward_runtime_to_gateway(&self, topic: &str, payload: &[u8]) -> ForwardOutcome {
        self.outbox.admit(topic, payload)
    }

    pub fn queue_stats(&self) -> IpcQueueStats {
        self.outbox.stats()
    }

    /// Bounded notifications of actual connection transitions, independent of configuration.
    pub fn subscribe_connection(&self) -> tokio::sync::broadcast::Receiver<bool> {
        self.outbox.subscribe_connection()
    }

    /// Stop reconnecting and wake the current writer. No new publication is admitted.
    pub fn shutdown(&self) {
        self.outbox.close();
    }
}

impl Drop for IpcFanout {
    fn drop(&mut self) {
        self.outbox.close();
    }
}

fn ipc_client_loop(socket_path: PathBuf, outbound: Arc<Outbox>, bus: crate::Bus) {
    while !outbound.closed() {
        let Some(mut stream) = connect_with_retry(&socket_path, &outbound) else {
            if outbound.closed() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            continue;
        };
        let mut reader = match stream.try_clone() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let read_bus = bus.clone();
        outbound.set_connected(true);
        let reader_outbound = Arc::clone(&outbound);
        let reader_task = thread::spawn(move || {
            read_inbound_commands(&mut reader, read_bus);
            reader_outbound.set_connected(false);
        });
        while let Some(publication) = outbound.next() {
            let frame = match encode_frame(
                DIRECTION_RUNTIME_TO_GATEWAY,
                publication.topic,
                &publication.payload,
            ) {
                Ok(f) => f,
                Err(e) => {
                    warn!(error = %e, "ipc encode");
                    continue;
                }
            };
            if write_with_deadline(&mut stream, &frame).is_err() {
                break;
            }
        }
        outbound.set_connected(false);
        let _ = stream.shutdown(std::net::Shutdown::Both);
        if reader_task.join().is_err() {
            warn!("ipc reader panicked");
        }
    }
}

fn write_with_deadline(
    stream: &mut std::os::unix::net::UnixStream,
    mut frame: &[u8],
) -> std::io::Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while !frame.is_empty() {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "ipc frame write deadline",
            ));
        }
        stream.set_write_timeout(Some(remaining))?;
        match stream.write(frame) {
            Ok(0) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "ipc frame write",
                ))
            }
            Ok(written) => frame = &frame[written..],
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_inbound_commands(stream: &mut std::os::unix::net::UnixStream, bus: crate::Bus) {
    let mut buf = Vec::new();
    let mut scratch = [0u8; 4096];
    loop {
        match stream.read(&mut scratch) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() + n > MAX_FRAME_BYTES + scratch.len() {
                    break;
                }
                buf.extend_from_slice(&scratch[..n]);
            }
            Err(e) => {
                warn!(error = %e, "ipc command read");
                break;
            }
        }
        loop {
            let frame = match take_frame(&mut buf) {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(error) => {
                    warn!(error = %error, "ipc command frame refused");
                    return;
                }
            };
            if let Ok((DIRECTION_GATEWAY_TO_RUNTIME, topic, payload)) = decode_frame(&frame) {
                let _ = bus.publish_bytes(&topic, payload);
            }
        }
    }
}

fn connect_with_retry(path: &Path, outbound: &Outbox) -> Option<std::os::unix::net::UnixStream> {
    const MAX_ATTEMPTS: u32 = 20;
    const RETRY_MS: u64 = 250;
    for attempt in 0..MAX_ATTEMPTS {
        if outbound.closed() {
            return None;
        }
        match std::os::unix::net::UnixStream::connect(path) {
            Ok(s) => {
                debug!(path = %path.display(), "chappe ipc connected");
                return Some(s);
            }
            Err(e) => {
                if attempt == 0 {
                    debug!(path = %path.display(), error = %e, "chappe ipc connect retry");
                }
                std::thread::sleep(std::time::Duration::from_millis(RETRY_MS));
            }
        }
    }
    let unreachable_ms = MAX_ATTEMPTS as u64 * RETRY_MS;
    error!(
        path = %path.display(),
        attempts = MAX_ATTEMPTS,
        unreachable_ms,
        "chappe ipc connect failed — gateway unreachable"
    );
    None
}

/// Gateway-side listener: ingests runtime frames; writes commands on the active peer connection.
pub struct IpcListener {
    peer: Arc<Mutex<Option<std::os::unix::net::UnixStream>>>,
}

impl IpcListener {
    pub fn spawn_server(
        socket_path: PathBuf,
        on_runtime_frame: Arc<dyn Fn(String, Vec<u8>) + Send + Sync>,
    ) -> Result<Arc<Self>, IpcError> {
        if socket_path.exists() {
            let _ = std::fs::remove_file(&socket_path);
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(IpcError::Io)?;
        }
        let listener = std::os::unix::net::UnixListener::bind(&socket_path)?;
        let _ = std::fs::set_permissions(
            &socket_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o660),
        );
        let peer = Arc::new(Mutex::new(None));
        let peer_accept = Arc::clone(&peer);
        thread::Builder::new()
            .name("chappe-ipc-server".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    if let Ok(mut guard) = peer_accept.lock() {
                        *guard = stream.try_clone().ok();
                    }
                    let on_frame = Arc::clone(&on_runtime_frame);
                    thread::spawn(move || read_connection(stream, on_frame));
                }
            })
            .map_err(IpcError::Io)?;
        Ok(Arc::new(Self { peer }))
    }

    pub fn send_command(&self, topic: &str, payload: &[u8]) -> Result<(), IpcError> {
        let frame = encode_frame(DIRECTION_GATEWAY_TO_RUNTIME, topic, payload)?;
        let mut guard = self
            .peer
            .lock()
            .map_err(|e| IpcError::Framing(e.to_string()))?;
        if let Some(s) = guard.as_mut() {
            match write_with_deadline(s, &frame) {
                Ok(()) => Ok(()),
                Err(error) => {
                    let _ = s.shutdown(std::net::Shutdown::Both);
                    *guard = None;
                    Err(IpcError::Io(error))
                }
            }
        } else {
            Err(IpcError::Framing("no ipc peer connected".into()))
        }
    }
}

fn read_connection(
    mut stream: std::os::unix::net::UnixStream,
    on_runtime_frame: Arc<dyn Fn(String, Vec<u8>) + Send + Sync>,
) {
    let mut buf = Vec::new();
    let mut scratch = [0u8; 4096];
    loop {
        match stream.read(&mut scratch) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() + n > MAX_FRAME_BYTES + scratch.len() {
                    break;
                }
                buf.extend_from_slice(&scratch[..n]);
            }
            Err(e) => {
                warn!(error = %e, "ipc read");
                break;
            }
        }
        loop {
            let frame = match take_frame(&mut buf) {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(error) => {
                    warn!(error = %error, "ipc runtime frame refused");
                    return;
                }
            };
            match decode_frame(&frame) {
                Ok((DIRECTION_RUNTIME_TO_GATEWAY, topic, payload)) => {
                    on_runtime_frame(topic, payload);
                }
                Ok((DIRECTION_GATEWAY_TO_RUNTIME, _, _)) => {}
                Ok((dir, _, _)) => warn!(direction = dir, "ipc unknown direction"),
                Err(e) => warn!(error = %e, "ipc decode"),
            }
        }
    }
}

fn take_frame(buf: &mut Vec<u8>) -> Result<Option<Vec<u8>>, IpcError> {
    if buf.len() < 1 + 4 {
        return Ok(None);
    }
    let direction = buf[0];
    let _ = direction;
    let topic_len = u32::from_le_bytes(
        buf[1..5]
            .try_into()
            .map_err(|_| IpcError::Framing("topic length".into()))?,
    ) as usize;
    if topic_len > MAX_TOPIC_BYTES {
        return Err(IpcError::Framing("topic too long".into()));
    }
    if buf.len() < 5 + topic_len + 4 {
        return Ok(None);
    }
    let payload_len = u32::from_le_bytes(
        buf[5 + topic_len..9 + topic_len]
            .try_into()
            .map_err(|_| IpcError::Framing("payload length".into()))?,
    ) as usize;
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(IpcError::Framing("payload too long".into()));
    }
    let total = 1 + 4 + topic_len + 4 + payload_len;
    if buf.len() < total {
        return Ok(None);
    }
    let frame = buf.drain(..total).collect();
    Ok(Some(frame))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    #[test]
    fn frame_roundtrip() {
        let raw =
            encode_frame(DIRECTION_RUNTIME_TO_GATEWAY, "robot/state", &[1, 2, 3]).expect("encode");
        let (dir, topic, payload) = decode_frame(&raw).expect("decode");
        assert_eq!(dir, DIRECTION_RUNTIME_TO_GATEWAY);
        assert_eq!(topic, "robot/state");
        assert_eq!(payload, vec![1, 2, 3]);
    }
}
