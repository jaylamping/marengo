//! Robstride RS-series motor protocol over CAN.

#[cfg(all(feature = "vcan", target_os = "linux"))]
pub mod vcan {
    //! Virtual CAN helpers for bench tests.

    /// Default SocketCAN interface used in compose `vcan` profile.
    pub const DEFAULT_INTERFACE: &str = "vcan0";
}

#[cfg(all(feature = "vcan", target_os = "linux", test))]
mod vcan_tests {
    #![allow(clippy::expect_used)]

    use super::vcan::DEFAULT_INTERFACE;
    use socketcan::{CanSocket, Socket};

    #[test]
    #[ignore = "requires vcan0 (docker compose --profile vcan)"]
    fn vcan0_exists() {
        let socket = CanSocket::open(DEFAULT_INTERFACE)
            .expect("open vcan0 — run scripts/vcan-up.sh or compose profile vcan");
        drop(socket);
    }
}
