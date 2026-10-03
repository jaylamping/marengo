#![allow(clippy::expect_used, dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use davout::simulation::SimulationBus;
use davout::Supervisor;
use marengo_config::{MotorEntry, MotorType};
use robstride::CanFrame;

/// Literal vendor status input, independent of the production encoder/decoder.
/// This constructs observations, never a reference permit or a pose cache.
pub fn status(motor: &MotorEntry, position_rad: f64, velocity_rad_s: f64) -> CanFrame {
    let scale = f64::from(motor.direction) * motor.gear_ratio;
    let velocity_scale = match motor.motor_type {
        MotorType::Rs00 => 50.0,
        MotorType::Rs03 => 20.0,
        MotorType::Rs02 => 44.0,
        MotorType::Rs04 => 15.0,
    };
    let position =
        ((position_rad * scale / (4.0 * std::f64::consts::PI) + 1.0) * 32767.0).round() as u16;
    let velocity = ((velocity_rad_s * scale / velocity_scale + 1.0) * 32767.0).round() as u16;
    let mut data = [0x7f, 0xff, 0x7f, 0xff, 0x7f, 0xff, 0, 0xc8];
    data[0..2].copy_from_slice(&position.to_be_bytes());
    data[2..4].copy_from_slice(&velocity.to_be_bytes());
    CanFrame {
        id: 0x0280_0000 | (u32::from(motor.device_id) << 8) | 0xfd,
        data,
        extended: true,
    }
}

pub fn queue_joint_status(
    supervisor: &mut Supervisor<SimulationBus>,
    joint: &str,
    position: f64,
    velocity: f64,
) {
    let motor = supervisor
        .motors
        .motors
        .iter()
        .find(|motor| motor.joint == joint)
        .expect("configured joint")
        .clone();
    supervisor
        .bus_mut()
        .queue_frame(status(&motor, position, velocity))
        .expect("finite raw status queue");
}

pub fn queue_all_status(
    supervisor: &mut Supervisor<SimulationBus>,
    moving: Option<(&str, f64, f64)>,
) {
    for motor in supervisor.motors.motors.clone() {
        let (position, velocity) = moving
            .filter(|(joint, _, _)| *joint == motor.joint)
            .map(|(_, position, velocity)| (position, velocity))
            .unwrap_or((0.0, 0.0));
        supervisor
            .bus_mut()
            .queue_frame(status(&motor, position, velocity))
            .expect("finite raw status queue");
    }
}

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Owns an exclusively-created test resource tree until the case finishes.
pub struct FixtureTree {
    root: PathBuf,
    parent: PathBuf,
}

impl FixtureTree {
    pub fn new(label: &str, source: &Path) -> Self {
        let parent = std::env::temp_dir().canonicalize().expect("test temp root");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = parent.join(format!(
            "berthier-{label}-{}-{nonce}-{}",
            std::process::id(),
            DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&root).expect("exclusive fixture ownership");
        let fixture = Self { root, parent };
        std::fs::create_dir(fixture.root.join("config")).expect("fixture config directory");
        for name in ["robot.yaml", "motors.yaml", "control.yaml", "homing.yaml"] {
            std::fs::copy(
                source.join("config").join(name),
                fixture.root.join("config").join(name),
            )
            .expect("copy immutable fixture input");
        }
        let model = fixture.root.join("assets/urdf");
        std::fs::create_dir_all(&model).expect("fixture model directory");
        std::fs::copy(
            source.join("assets/urdf/marengo.urdf"),
            model.join("marengo.urdf"),
        )
        .expect("copy immutable model input");
        fixture
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for FixtureTree {
    fn drop(&mut self) {
        let Ok(actual) = self.root.canonicalize() else {
            return;
        };
        if actual.parent() == Some(self.parent.as_path()) {
            let _ = std::fs::remove_dir_all(actual);
        }
    }
}
