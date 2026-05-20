//! Robstride RS-series motor protocol over CAN.

#![forbid(unsafe_code)]

#[cfg(all(feature = "vcan", target_os = "linux"))]
pub mod vcan {
    //! Virtual CAN helpers for bench tests.

    /// Default SocketCAN interface used in compose `vcan` profile.
    pub const DEFAULT_INTERFACE: &str = "vcan0";
}

#[cfg(all(feature = "vcan", target_os = "linux", test))]
mod vcan_tests {
    use super::vcan::DEFAULT_INTERFACE;

    #[test]
    #[ignore = "requires vcan0 (docker compose --profile vcan)"]
    fn vcan0_exists() {
        use socketcan::{CanSocket, Socket};
        let socket = CanSocket::open(DEFAULT_INTERFACE)
            .expect("open vcan0 — run scripts/vcan-up.sh or compose profile vcan");
        drop(socket);
    }
}
