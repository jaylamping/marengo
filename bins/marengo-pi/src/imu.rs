//! Optional BNO085 polling thread — publishes `sensors/imu/torso` on Chappe.

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use armee_proto::{ImuAccuracy as ProtoImuAccuracy, ImuSample};
use chappe::Bus;
use marengo_imu::{Bno085, LinuxI2cBus, DEFAULT_I2C_ADDRESS};
use tracing::{debug, info, warn};

pub const TOPIC_IMU_TORSO: &str = "sensors/imu/torso";
const DEFAULT_FRAME_ID: &str = "torso_imu";

struct ImuConfig {
    bus_path: String,
    address: u16,
    report_interval_us: u32,
    frame_id: String,
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn proto_accuracy(accuracy: marengo_imu::ImuAccuracy) -> i32 {
    match accuracy {
        marengo_imu::ImuAccuracy::Unreliable => ProtoImuAccuracy::Unreliable as i32,
        marengo_imu::ImuAccuracy::Low => ProtoImuAccuracy::Low as i32,
        marengo_imu::ImuAccuracy::Medium => ProtoImuAccuracy::Medium as i32,
        marengo_imu::ImuAccuracy::High => ProtoImuAccuracy::High as i32,
    }
}

fn load_config() -> Option<ImuConfig> {
    let bus_path = env::var("MARENGO_IMU_BUS").ok()?;
    if bus_path.trim().is_empty() {
        return None;
    }
    let address = env::var("MARENGO_IMU_ADDRESS")
        .ok()
        .and_then(|raw| u16::from_str_radix(raw.trim_start_matches("0x"), 16).ok())
        .unwrap_or(DEFAULT_I2C_ADDRESS);
    let report_hz = env::var("MARENGO_IMU_REPORT_HZ")
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
        .unwrap_or(50)
        .max(1);
    let report_interval_us = 1_000_000 / report_hz;
    let frame_id =
        env::var("MARENGO_IMU_FRAME_ID").unwrap_or_else(|_| DEFAULT_FRAME_ID.to_string());
    Some(ImuConfig {
        bus_path,
        address,
        report_interval_us,
        frame_id,
    })
}

pub fn spawn_imu_publisher(chappe: Arc<Bus>, shutdown: Arc<AtomicBool>) {
    let Some(cfg) = load_config() else {
        return;
    };

    info!(
        bus = %cfg.bus_path,
        address = cfg.address,
        frame_id = %cfg.frame_id,
        report_interval_us = cfg.report_interval_us,
        "starting IMU publisher thread"
    );

    thread::spawn(move || run_imu_loop(chappe, shutdown, cfg));
}

fn run_imu_loop(chappe: Arc<Bus>, shutdown: Arc<AtomicBool>, cfg: ImuConfig) {
    let mut backoff = Duration::from_secs(1);
    const MAX_BACKOFF: Duration = Duration::from_secs(10);

    while !shutdown.load(Ordering::SeqCst) {
        info!("IMU session starting");
        match run_imu_session(&chappe, &shutdown, &cfg) {
            Ok(()) => {
                info!("IMU session ended cleanly");
                return;
            }
            Err(err) => {
                warn!(error = %err, backoff_sec = backoff.as_secs(), "IMU session failed, restarting");
                let deadline = Instant::now() + backoff;
                while !shutdown.load(Ordering::SeqCst) && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(100));
                }
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
}

fn run_imu_session(
    chappe: &Arc<Bus>,
    shutdown: &Arc<AtomicBool>,
    cfg: &ImuConfig,
) -> Result<(), String> {
    let bus = LinuxI2cBus::open(&cfg.bus_path, cfg.address).map_err(|e| e.to_string())?;
    let mut imu = Bno085::new(bus);
    imu.initialize().map_err(|e| e.to_string())?;
    imu.enable_rotation_vector(cfg.report_interval_us)
        .map_err(|e| e.to_string())?;

    let poll_period = Duration::from_micros(u64::from(cfg.report_interval_us));
    while !shutdown.load(Ordering::SeqCst) {
        let tick = Instant::now();
        match imu.poll() {
            Ok(Some(sample)) => {
                let q = sample.quaternion;
                let msg = ImuSample {
                    timestamp_ms: timestamp_ms(),
                    frame_id: cfg.frame_id.clone(),
                    quaternion_i: q.i,
                    quaternion_j: q.j,
                    quaternion_k: q.k,
                    quaternion_real: q.real,
                    accuracy: proto_accuracy(sample.accuracy),
                    accel_x_m_s2: 0.0,
                    accel_y_m_s2: 0.0,
                    accel_z_m_s2: 0.0,
                    gyro_x_rad_s: 0.0,
                    gyro_y_rad_s: 0.0,
                    gyro_z_rad_s: 0.0,
                    has_accel: false,
                    has_gyro: false,
                };
                if let Err(err) =
                    chappe.publish(TOPIC_IMU_TORSO, "marengo-pi", "marengo.v1.ImuSample", &msg)
                {
                    warn!(error = %err, "failed to publish ImuSample");
                } else {
                    debug!(real = q.real, "published ImuSample");
                }
            }
            Ok(None) => {}
            Err(err) => warn!(error = %err, "IMU poll failed"),
        }

        let elapsed = tick.elapsed();
        if elapsed < poll_period {
            thread::sleep(poll_period - elapsed);
        }
    }
    Ok(())
}
