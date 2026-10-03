//! Generated Protocol Buffer types from [`proto/`](../../proto/).
//!
//! Do not hand-edit this crate — change `.proto` files and rebuild.

include!(concat!(env!("OUT_DIR"), "/marengo.v1.rs"));

pub use prost;

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::JointHomingState;

    #[test]
    fn joint_homing_state_unspecified_is_zero() {
        assert_eq!(JointHomingState::Unspecified as i32, 0);
        assert_eq!(JointHomingState::Unhomed as i32, 1);
        assert_eq!(JointHomingState::Homing as i32, 2);
        assert_eq!(JointHomingState::Verified as i32, 3);
        assert_eq!(JointHomingState::Faulted as i32, 4);
    }
}
