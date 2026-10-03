//! Unix-domain socket bridge between `marengo-pi` and `marengo-gateway`.
//!
//! Framing (all directions): `u8 direction` + `u32 topic_len` + topic utf-8 + `u32 len` + payload.
//! - `0` = runtime → gateway (telemetry)
//! - `1` = gateway → runtime (commands)

use std::io::{Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::ipc_outbox::Outbox;
pub use crate::ipc_outbox::{
    ForwardOutcome, IpcQueueStats, EVENT_BYTE_CAPACITY, EVENT_CAPACITY, MAX_PAYLOAD_BYTES,
    QUEUE_BYTE_CAPACITY, QUEUE_ITEM_CAPACITY,
};
use armee_proto::prost::Message;
use thiserror::Error;
use tracing::{debug, warn};

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

pub(crate) fn encode_frame(
    direction: u8,
    topic: &str,
    payload: &[u8],
) -> Result<Vec<u8>, IpcError> {
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

pub(crate) fn decode_frame(mut data: &[u8]) -> Result<(u8, String, Vec<u8>), IpcError> {
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
    worker: Mutex<Option<thread::JoinHandle<()>>>,
}

impl IpcFanout {
    pub fn spawn_client(socket_path: PathBuf, bus: crate::Bus) -> Result<Arc<Self>, IpcError> {
        let outbox = Arc::new(Outbox::new());
        Self::spawn_client_with_outbox(socket_path, bus, outbox)
    }

    fn spawn_client_with_outbox(
        socket_path: PathBuf,
        bus: crate::Bus,
        outbox: Arc<Outbox>,
    ) -> Result<Arc<Self>, IpcError> {
        let worker_outbox = Arc::clone(&outbox);
        let worker = thread::Builder::new()
            .name("chappe-ipc-client".into())
            .spawn(move || ipc_client_loop(socket_path, worker_outbox, bus))
            .map_err(IpcError::Io)?;
        Ok(Arc::new(Self {
            outbox,
            worker: Mutex::new(Some(worker)),
        }))
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
    pub fn shutdown(&self) -> Result<(), IpcError> {
        self.outbox.close();
        if let Some(worker) = self
            .worker
            .lock()
            .map_err(|error| IpcError::Framing(error.to_string()))?
            .take()
        {
            worker
                .join()
                .map_err(|_| IpcError::Framing("ipc client worker panicked".into()))?;
        }
        Ok(())
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
                outbound.record_write_failure();
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
                if command_is_current(&topic, &payload) {
                    let _ = bus.publish_bytes(&topic, payload);
                } else {
                    warn!(topic, "ipc command topic or timestamp refused");
                }
            }
        }
    }
}

fn command_is_current(topic: &str, payload: &[u8]) -> bool {
    const COMMAND_TOPICS: [&str; 7] = crate::topics::COMMAND_TOPICS;
    if !COMMAND_TOPICS.contains(&topic) {
        return false;
    }
    let Ok(envelope) = armee_proto::Envelope::decode(payload) else {
        return false;
    };
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return false;
    };
    let Ok(now_ms) = u64::try_from(now.as_millis()) else {
        return false;
    };
    now_ms
        .checked_sub(envelope.timestamp_ms)
        .is_some_and(|age| age <= 1000)
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
    // warn, not error: a down gateway is routine during deploys/restarts,
    // and this fires every ~6 s until it returns (L-chappe-02).
    warn!(
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
    closed: Arc<AtomicBool>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    accept_health: Arc<AcceptHealth>,
}

/// Accept-loop health for the listener thread (L-chappe-03): a dead accept
/// loop used to be silent, leaving `send_command` failing with "no ipc peer"
/// forever. Transient accept errors now retry with backoff; persistent ones
/// are counted and surface here instead of dying quietly.
#[derive(Debug, Default)]
pub struct AcceptHealth {
    failures: AtomicU64,
    dead: AtomicBool,
}

impl AcceptHealth {
    /// Total non-`WouldBlock` accept errors observed.
    pub fn failures(&self) -> u64 {
        self.failures.load(Ordering::Relaxed)
    }

    /// True once the accept loop gave up after consecutive failures.
    pub fn dead(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }
}

/// Consecutive accept failures tolerated before the listener gives up.
/// Pure policy so the retry bound is unit-testable without a socket.
#[derive(Debug)]
struct AcceptRetry {
    consecutive_failures: u32,
}

impl AcceptRetry {
    const MAX_CONSECUTIVE_FAILURES: u32 = 20;
    const RETRY_DELAY: Duration = Duration::from_millis(250);

    fn new() -> Self {
        Self {
            consecutive_failures: 0,
        }
    }

    fn on_accept(&mut self) {
        self.consecutive_failures = 0;
    }

    /// Backoff before the next accept, or `None` once the loop should give up.
    fn on_error(&mut self) -> Option<Duration> {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= Self::MAX_CONSECUTIVE_FAILURES {
            None
        } else {
            Some(Self::RETRY_DELAY)
        }
    }
}

impl IpcListener {
    /// Serialize peer transitions with frame delivery; retired peers cannot publish
    /// into the replacement connection's state. Callbacks must not block.
    pub fn spawn_server_with_lifecycle(
        socket_path: PathBuf,
        on_runtime_frame: Arc<dyn Fn(String, Vec<u8>) + Send + Sync>,
        on_connection_change: Arc<dyn Fn(bool) + Send + Sync>,
    ) -> Result<Arc<Self>, IpcError> {
        Self::spawn_server_before_install(
            socket_path,
            on_runtime_frame,
            on_connection_change,
            || {},
        )
    }

    fn spawn_server_before_install(
        socket_path: PathBuf,
        on_runtime_frame: Arc<dyn Fn(String, Vec<u8>) + Send + Sync>,
        on_connection_change: Arc<dyn Fn(bool) + Send + Sync>,
        before_install: impl Fn() + Send + Sync + 'static,
    ) -> Result<Arc<Self>, IpcError> {
        // Never unlink a non-socket path: a stale regular file at the socket
        // path is a deployment error, not a leftover to sweep (L-chappe-04).
        if socket_path.exists() {
            let is_socket = std::fs::symlink_metadata(&socket_path)
                .map(|metadata| metadata.file_type().is_socket())
                .unwrap_or(false);
            if !is_socket {
                return Err(IpcError::Framing(
                    "ipc socket path exists and is not a socket; refusing to replace".into(),
                ));
            }
            let _ = std::fs::remove_file(&socket_path);
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(IpcError::Io)?;
        }
        let listener = std::os::unix::net::UnixListener::bind(&socket_path)?;
        listener.set_nonblocking(true)?;
        if let Err(error) = std::fs::set_permissions(
            &socket_path,
            std::os::unix::fs::PermissionsExt::from_mode(0o660),
        ) {
            warn!(error = %error, "ipc socket permissions not set; peer trust unchanged");
        }
        let peer = Arc::new(Mutex::new(None::<std::os::unix::net::UnixStream>));
        let peer_accept = Arc::clone(&peer);
        let delivery = Arc::new(Mutex::new(()));
        let active_generation = Arc::new(AtomicU64::new(0));
        let closed = Arc::new(AtomicBool::new(false));
        let accept_closed = Arc::clone(&closed);
        let accept_health = Arc::new(AcceptHealth::default());
        let accept_health_worker = Arc::clone(&accept_health);
        let worker = thread::Builder::new()
            .name("chappe-ipc-server".into())
            .spawn(move || {
                let mut generation = 0_u64;
                let mut retry = AcceptRetry::new();
                let mut previous_reader: Option<thread::JoinHandle<()>> = None;
                while !accept_closed.load(Ordering::Relaxed) {
                    let stream = match listener.accept() {
                        Ok((stream, _)) => stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(std::time::Duration::from_millis(50));
                            continue;
                        }
                        Err(error) => {
                            accept_health_worker
                                .failures
                                .fetch_add(1, Ordering::Relaxed);
                            match retry.on_error() {
                                Some(delay) => {
                                    warn!(error = %error, "ipc accept failed; retrying");
                                    thread::sleep(delay);
                                    continue;
                                }
                                None => {
                                    warn!(error = %error, "ipc accept failed; listener giving up");
                                    accept_health_worker.dead.store(true, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                    };
                    retry.on_accept();
                    if let Err(error) = stream.set_nonblocking(false) {
                        warn!(error = %error, "ipc peer blocking mode failed");
                        continue;
                    }
                    let writer = match stream.try_clone() {
                        Ok(writer) => writer,
                        Err(error) => {
                            warn!(error = %error, "ipc peer clone failed");
                            continue;
                        }
                    };
                    generation = generation.saturating_add(1);
                    let connection_generation = generation;
                    before_install();
                    {
                        let _delivery = delivery.lock().unwrap_or_else(|error| error.into_inner());
                        let mut guard = peer_accept
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        if accept_closed.load(Ordering::Relaxed) {
                            let _ = writer.shutdown(std::net::Shutdown::Both);
                            break;
                        }
                        if let Some(old) = guard.take() {
                            let _ = old.shutdown(std::net::Shutdown::Both);
                        }
                        *guard = Some(writer);
                        active_generation.store(connection_generation, Ordering::Relaxed);
                        drop(guard);
                        on_connection_change(true);
                    }
                    if let Some(reader) = previous_reader.take() {
                        if reader.join().is_err() {
                            warn!("retired ipc peer reader panicked");
                        }
                    }
                    let handler = Arc::clone(&on_runtime_frame);
                    let frame_delivery = Arc::clone(&delivery);
                    let frame_generation = Arc::clone(&active_generation);
                    let on_frame = Arc::new(move |topic, payload| {
                        let _delivery = frame_delivery
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        if frame_generation.load(Ordering::Relaxed) == connection_generation {
                            handler(topic, payload);
                        }
                    });
                    let reader_delivery = Arc::clone(&delivery);
                    let reader_generation = Arc::clone(&active_generation);
                    let reader_peer = Arc::clone(&peer_accept);
                    let reader_changed = Arc::clone(&on_connection_change);
                    previous_reader = Some(thread::spawn(move || {
                        read_connection(stream, on_frame);
                        let _delivery = reader_delivery
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        if reader_generation.load(Ordering::Relaxed) == connection_generation {
                            reader_generation.store(0, Ordering::Relaxed);
                            let old = reader_peer
                                .lock()
                                .unwrap_or_else(|error| error.into_inner())
                                .take();
                            if let Some(old) = old {
                                let _ = old.shutdown(std::net::Shutdown::Both);
                            }
                            reader_changed(false);
                        }
                    }));
                }
                if let Some(reader) = previous_reader {
                    if reader.join().is_err() {
                        warn!("ipc peer reader panicked during shutdown");
                    }
                }
            })
            .map_err(IpcError::Io)?;
        Ok(Arc::new(Self {
            peer,
            closed,
            worker: Mutex::new(Some(worker)),
            accept_health,
        }))
    }

    /// Accept-loop health: failures counted, `dead` once the loop gave up.
    pub fn accept_health(&self) -> &AcceptHealth {
        &self.accept_health
    }

    fn close(&self) {
        let guard = self.peer.lock().unwrap_or_else(|error| error.into_inner());
        self.closed.store(true, Ordering::Relaxed);
        if let Some(peer) = guard.as_ref() {
            let _ = peer.shutdown(std::net::Shutdown::Both);
        }
    }

    /// Close the active peer, wake accept and join all owned listener/reader tasks.
    /// Call outside callbacks and realtime loops.
    pub fn shutdown(&self) -> Result<(), IpcError> {
        self.close();
        if let Some(worker) = self
            .worker
            .lock()
            .map_err(|error| IpcError::Framing(error.to_string()))?
            .take()
        {
            worker
                .join()
                .map_err(|_| IpcError::Framing("ipc listener worker panicked".into()))?;
        }
        Ok(())
    }

    pub fn send_command(&self, topic: &str, payload: &[u8]) -> Result<(), IpcError> {
        if self.closed.load(Ordering::Relaxed) {
            return Err(IpcError::Framing("ipc listener closed".into()));
        }
        let frame = encode_frame(DIRECTION_GATEWAY_TO_RUNTIME, topic, payload)?;
        let mut guard = self
            .peer
            .lock()
            .map_err(|e| IpcError::Framing(e.to_string()))?;
        if self.closed.load(Ordering::Relaxed) {
            return Err(IpcError::Framing("ipc listener closed".into()));
        }
        if let Some(s) = guard.as_mut() {
            match write_with_deadline(s, &frame) {
                Ok(()) => Ok(()),
                Err(error) => {
                    let _ = s.shutdown(std::net::Shutdown::Both);
                    *guard = None;
                    Err(IpcError::Io(error))
                }
            }
        } else if self.accept_health.dead() {
            Err(IpcError::Framing(
                "no ipc peer connected; listener accept loop failed".into(),
            ))
        } else {
            Err(IpcError::Framing("no ipc peer connected".into()))
        }
    }
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        self.close();
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
                // debug, not warn: the direction byte is peer-controlled, so a
                // warn here is a log-spam vector (L-chappe-10).
                Ok((dir, _, _)) => debug!(direction = dir, "ipc unknown direction"),
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

    #[test]
    fn accept_retry_gives_up_after_bounded_failures() {
        // L-chappe-03: transient accept errors retry; persistent ones give up
        // visibly instead of dying silently on the first error.
        let mut retry = AcceptRetry::new();
        for _ in 0..AcceptRetry::MAX_CONSECUTIVE_FAILURES - 1 {
            assert!(retry.on_error().is_some());
        }
        assert!(retry.on_error().is_none());
    }

    #[test]
    fn accept_retry_resets_on_success() {
        let mut retry = AcceptRetry::new();
        assert!(retry.on_error().is_some());
        retry.on_accept();
        for _ in 0..AcceptRetry::MAX_CONSECUTIVE_FAILURES - 1 {
            assert!(retry.on_error().is_some());
        }
        assert!(retry.on_error().is_none());
    }

    #[test]
    fn take_frame_refuses_oversize_topic_at_reader() {
        // L-chappe-11: inbound bounds pinned at the reader's frame splitter,
        // not just the outbound encoder.
        let mut buf = vec![DIRECTION_RUNTIME_TO_GATEWAY];
        buf.extend_from_slice(
            &u32::try_from(MAX_TOPIC_BYTES + 1)
                .expect("bound fits")
                .to_le_bytes(),
        );
        buf.extend_from_slice(&vec![b'x'; MAX_TOPIC_BYTES + 1]);
        buf.extend_from_slice(&0_u32.to_le_bytes());
        assert!(take_frame(&mut buf).is_err());
    }

    #[test]
    fn take_frame_refuses_oversize_payload_at_reader() {
        let topic = b"robot/state";
        let mut buf = vec![DIRECTION_RUNTIME_TO_GATEWAY];
        buf.extend_from_slice(&(topic.len() as u32).to_le_bytes());
        buf.extend_from_slice(topic);
        buf.extend_from_slice(
            &u32::try_from(MAX_PAYLOAD_BYTES + 1)
                .expect("bound fits")
                .to_le_bytes(),
        );
        assert!(take_frame(&mut buf).is_err());
    }

    #[test]
    fn take_frame_waits_for_truncated_frame() {
        let mut buf = vec![DIRECTION_RUNTIME_TO_GATEWAY];
        buf.extend_from_slice(&3_u32.to_le_bytes());
        buf.extend_from_slice(b"ro");
        assert!(take_frame(&mut buf).expect("frame").is_none());
    }

    fn fresh_command_frame(topic: &str) -> Vec<u8> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .expect("clock");
        let envelope = armee_proto::Envelope {
            timestamp_ms: now_ms,
            source_node: "test".to_string(),
            message_type: "test.v1.Command".to_string(),
            payload: Vec::new(),
        };
        encode_frame(
            DIRECTION_GATEWAY_TO_RUNTIME,
            topic,
            &envelope.encode_to_vec(),
        )
        .expect("command frame")
    }

    #[test]
    fn inbound_reader_closes_on_oversize_frame_without_publishing() {
        use std::os::unix::net::UnixStream;

        let bus = crate::Bus::default();
        let mut commands = bus.subscribe(crate::topics::TOPIC_MOTOR_STATUS_POLL);
        let (mut client, mut server) = UnixStream::pair().expect("socketpair");
        client
            .write_all(&fresh_command_frame(crate::topics::TOPIC_MOTOR_STATUS_POLL))
            .expect("valid command");
        let mut oversize = vec![DIRECTION_GATEWAY_TO_RUNTIME];
        oversize.extend_from_slice(
            &u32::try_from(MAX_TOPIC_BYTES + 1)
                .expect("bound fits")
                .to_le_bytes(),
        );
        oversize.extend_from_slice(&vec![b'y'; MAX_TOPIC_BYTES + 1]);
        oversize.extend_from_slice(&0_u32.to_le_bytes());
        client.write_all(&oversize).expect("oversize frame");
        client
            .shutdown(std::net::Shutdown::Write)
            .expect("client eof");
        read_inbound_commands(&mut server, bus);
        // The valid command ahead of the oversize frame is admitted; the
        // oversize frame closes the connection without publishing.
        assert!(commands.try_recv().is_ok());
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn unknown_direction_frame_is_ignored_and_stream_survives() {
        // L-chappe-10: a misbehaving peer's direction byte must not break the
        // stream (and no longer warns per frame).
        use std::os::unix::net::UnixStream;

        let (mut client, server) = UnixStream::pair().expect("socketpair");
        let bogus = encode_frame(99, "robot/state", &[1, 2, 3]).expect("bogus direction frame");
        let valid =
            encode_frame(DIRECTION_RUNTIME_TO_GATEWAY, "robot/state", &[9]).expect("valid frame");
        client.write_all(&bogus).expect("bogus");
        client.write_all(&valid).expect("valid");
        client
            .shutdown(std::net::Shutdown::Write)
            .expect("client eof");
        let received = Arc::new(Mutex::new(Vec::new()));
        let handler_received = Arc::clone(&received);
        read_connection(
            server,
            Arc::new(move |topic, payload| {
                handler_received
                    .lock()
                    .expect("handler lock")
                    .push((topic, payload));
            }),
        );
        let received = received.lock().expect("result lock");
        assert_eq!(received.as_slice(), &[("robot/state".to_string(), vec![9])]);
    }

    #[test]
    fn listener_refuses_to_replace_non_socket_path() {
        // L-chappe-04: a regular file at the socket path is refused, not unlinked.
        let fixture = tempfile::tempdir().expect("fixture");
        let path = fixture.path().join("chappe.sock");
        std::fs::write(&path, b"not a socket").expect("regular file");
        let result = IpcListener::spawn_server_with_lifecycle(
            path.clone(),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
        );
        assert!(result.is_err());
        assert!(path.is_file());
    }
}

#[cfg(test)]
mod listener_shutdown_race_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn shutdown_winning_before_peer_install_joins_with_idle_peer() {
        let fixture = tempfile::tempdir().expect("fixture");
        let path = fixture.path().join("shutdown.sock");
        let (paused_tx, paused_rx) = mpsc::channel();
        let (resume_tx, resume_rx) = mpsc::channel();
        let resume_rx = Mutex::new(resume_rx);
        let listener = IpcListener::spawn_server_before_install(
            path.clone(),
            Arc::new(|_, _| {}),
            Arc::new(|_| {}),
            move || {
                paused_tx.send(()).expect("accepted peer barrier");
                resume_rx
                    .lock()
                    .expect("resume lock")
                    .recv()
                    .expect("resume install");
            },
        )
        .expect("listener");
        let peer = std::os::unix::net::UnixStream::connect(path).expect("idle peer");
        paused_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("accept barrier");
        listener.close();
        resume_tx.send(()).expect("resume admission after shutdown");
        let owned = Arc::clone(&listener);
        let (complete_tx, complete_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = complete_tx.send(owned.shutdown());
        });
        let completed_while_idle = complete_rx.recv_timeout(Duration::from_secs(5));
        // Even a regressed join is cleaned up by closing the deliberately idle peer.
        drop(peer);
        if completed_while_idle.is_err() {
            complete_rx
                .recv_timeout(Duration::from_secs(5))
                .expect("cleanup join deadline")
                .expect("cleanup shutdown");
        }
        worker.join().expect("join shutdown observer");
        drop(listener);
        fixture.close().expect("remove joined socket fixture");
        completed_while_idle
            .expect("shutdown must not need peer cooperation")
            .expect("shutdown result");
    }
}

#[cfg(test)]
mod telemetry_expiry_tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn reconnect_sends_only_eligible_state_under_injected_monotonic_time() {
        for age_ms in [1000, 1001] {
            let fixture = tempfile::tempdir().expect("fixture");
            let path = fixture.path().join("expiry.sock");
            let elapsed = Arc::new(AtomicU64::new(0));
            let clock_elapsed = Arc::clone(&elapsed);
            let origin = Instant::now();
            let outbox = Arc::new(Outbox::new_with_clock(Arc::new(move || {
                origin + Duration::from_millis(clock_elapsed.load(Ordering::Relaxed))
            })));
            let fanout =
                IpcFanout::spawn_client_with_outbox(path.clone(), crate::Bus::default(), outbox)
                    .expect("fanout");
            fanout.forward_runtime_to_gateway("robot/state", &7_u64.to_le_bytes());
            elapsed.store(age_ms, Ordering::Relaxed);
            assert_eq!(fanout.queue_stats().oldest_age_ms, age_ms);
            fanout.forward_runtime_to_gateway("robot/safety", &8_u64.to_le_bytes());
            fanout.forward_runtime_to_gateway("robot/heartbeat", &9_u64.to_le_bytes());
            fanout.forward_runtime_to_gateway("robot/audit/action", &10_u64.to_le_bytes());
            let listener = std::os::unix::net::UnixListener::bind(path).expect("listener");
            let (mut peer, _) = listener.accept().expect("accept");
            peer.set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read deadline");
            let mut frames = Vec::new();
            loop {
                let mut header = [0; 5];
                peer.read_exact(&mut header).expect("header");
                let size =
                    u32::from_le_bytes(header[1..].try_into().expect("topic length")) as usize;
                assert!(size < MAX_TOPIC_BYTES);
                let mut topic = vec![0; size];
                peer.read_exact(&mut topic).expect("topic");
                let mut length = [0; 4];
                peer.read_exact(&mut length).expect("length");
                assert_eq!(u32::from_le_bytes(length), 8);
                let mut payload = [0; 8];
                peer.read_exact(&mut payload).expect("payload");
                let topic = String::from_utf8(topic).expect("topic utf8");
                let barrier = topic == "robot/audit/action";
                frames.push((topic, u64::from_le_bytes(payload)));
                if barrier {
                    break;
                }
            }
            assert_eq!(
                frames
                    .iter()
                    .any(|(topic, value)| topic == "robot/state" && *value == 7),
                age_ms == 1000
            );
            assert!(
                frames.contains(&("robot/safety".into(), 8))
                    && frames.contains(&("robot/heartbeat".into(), 9))
            );
            assert_eq!(fanout.queue_stats().dropped, u64::from(age_ms > 1000));
            fanout.shutdown().expect("join transport");
            drop(peer);
            fixture.close().expect("remove joined fixture");
        }
    }
}
