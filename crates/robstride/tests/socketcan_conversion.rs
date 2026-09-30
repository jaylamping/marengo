#![cfg(all(feature = "socketcan", target_os = "linux"))]
#![allow(clippy::expect_used)]

use robstride::{BusError, ReceivedCanFrame, RxFrameKind};
use socketcan::{CanAnyFrame, CanErrorFrame, CanFdFrame, CanFrame, EmbeddedFrame, ExtendedId};

#[test]
fn actual_socketcan_typed_conversion_preserves_short_data_and_remote_requests() {
    let id = ExtendedId::new(0x0280_01fd).expect("literal id");
    for length in 0..=8 {
        let bytes = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
        let normal = CanFrame::new(id, &bytes[..length]).expect("classic data");
        let received = ReceivedCanFrame::from_socketcan(Some("vcan0".into()), normal.into())
            .expect("supported classic frame");
        assert_eq!(received.payload_len, length as u8);
        assert_eq!(received.kind, RxFrameKind::Data);
        assert_eq!(received.payload(), Some(&bytes[..length]));
        assert_eq!(received.frame.id, 0x0280_01fd);
        assert!(received.frame.extended);

        let remote = CanFrame::new_remote(id, length).expect("classic remote");
        let received =
            ReceivedCanFrame::from_socketcan(None, remote.into()).expect("remote evidence");
        assert_eq!(
            received.kind,
            RxFrameKind::Remote {
                requested_len: length as u8
            }
        );
        assert_eq!(received.payload_len, 0);
        assert_eq!(received.payload(), Some([].as_slice()));
    }
}

#[test]
fn actual_socketcan_error_conversion_retains_unknown_high_classes_and_bytes() {
    // The dependency's public test constructor produces typed incoming evidence;
    // this does not claim that vCAN injects physical bus-off/error notifications.
    let bytes = [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0];
    let error = CanErrorFrame::new_error(0x1000_0040, &bytes).expect("typed error fixture");
    let received =
        ReceivedCanFrame::from_socketcan(Some("vcan0".into()), CanAnyFrame::Error(error))
            .expect("error evidence");
    assert_eq!(received.kind, RxFrameKind::Error);
    assert_eq!(received.frame.id, 0x1000_0040);
    assert!(!received.frame.extended);
    assert_eq!(received.payload(), Some(bytes.as_slice()));
}

#[test]
fn actual_socketcan_fd_conversion_fails_closed_even_for_eight_bytes() {
    let fd =
        CanFdFrame::new(ExtendedId::new(0x0280_01fd).expect("id"), &[0; 8]).expect("FD fixture");
    assert!(matches!(
        ReceivedCanFrame::from_socketcan(None, CanAnyFrame::Fd(fd)),
        Err(BusError::Driver(_))
    ));
}

#[test]
fn socketcan_short_error_retains_only_received_bytes() {
    // Copy the dependency's own safe raw representation rather than inventing a
    // CAN ABI. This qualifies conversion of raw envelopes, not kernel delivery.
    let error =
        CanErrorFrame::new_error(0x1000_0040, &[1, 2, 3, 4, 5, 6, 7, 8]).expect("error fixture");
    let mut raw = *error.as_ref();
    raw.can_dlc = 3;
    let short = ReceivedCanFrame::from_socketcan(None, CanAnyFrame::from(raw))
        .expect("short error retains its available bytes");
    assert_eq!(short.payload_len, 3);
    assert_eq!(short.payload(), Some([1, 2, 3].as_slice()));
}

#[test]
fn socketcan_raw_classic_dlc_is_checked_before_accessing_storage() {
    let error = CanErrorFrame::new_error(0x1000_0040, &[0; 8]).expect("error fixture");
    let id = ExtendedId::new(0x0280_01fd).expect("id");
    let normal = socketcan::CanDataFrame::new(id, &[0; 8]).expect("data fixture");
    let remote = socketcan::CanRemoteFrame::new_remote(id, 8).expect("remote fixture");
    for mut raw in [*normal.as_ref(), *remote.as_ref(), *error.as_ref()] {
        raw.can_dlc = 9;
        assert!(matches!(
            ReceivedCanFrame::from_socketcan(None, CanAnyFrame::from(raw)),
            Err(BusError::Driver(_))
        ));
    }
}
