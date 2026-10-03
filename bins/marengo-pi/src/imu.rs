//! Optional BNO085 polling thread — publishes `sensors/imu/torso` on Chappe.

use std::env;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use armee_proto::{ImuAccuracy as ProtoImuAccuracy, ImuSample};
use chappe::topics::TOPIC_IMU_TORSO;
use chappe::Bus;
use marengo_imu::{parse_i2c_address, Bno085, LinuxI2cBus, DEFAULT_I2C_ADDRESS};
use tracing::{debug, error, info, warn};

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
    // A malformed address disables the IMU loudly; it never falls back to the
    // default address (a wrong sensor would publish as the torso IMU).
    let address = match env::var("MARENGO_IMU_ADDRESS") {
        Ok(raw) => match parse_i2c_address(&raw) {
            Some(address) => address,
            None => {
                error!(value = %raw, "MARENGO_IMU_ADDRESS is not a 7-bit hex I2C address; IMU disabled");
                return None;
            }
        },
        Err(_) => DEFAULT_I2C_ADDRESS,
    };
    // Clamp the report rate: above 1 kHz the interval rounds to 0 and the
    // poll loop spins. The BNO085 rotation vector tops out far below this.
    let report_hz = env::var("MARENGO_IMU_REPORT_HZ")
        .ok()
        .and_then(|raw| raw.parse::<u32>().ok())
        .unwrap_or(50)
        .clamp(1, 1000);
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
    // Cancellable init: an absent sensor must not wedge shutdown for ~22 s.
    imu.initialize_while(|| !shutdown.load(Ordering::SeqCst))
        .map_err(|e| e.to_string())?;
    if shutdown.load(Ordering::SeqCst) {
        return Ok(());
    }
    imu.enable_rotation_vector(cfg.report_interval_us)
        .map_err(|e| e.to_string())?;

    let poll_period = Duration::from_micros(u64::from(cfg.report_interval_us));
    let mut consecutive_errors = 0u32;
    while !shutdown.load(Ordering::SeqCst) {
        let tick = Instant::now();
        match imu.poll() {
            Ok(Some(sample)) => {
                consecutive_errors = 0;
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
                    // Only fresh samples are published; a repeated seq (or
                    // silence) means the sensor stopped, never a held pose.
                    sample_seq: imu.sample_seq(),
                };
                if let Err(err) =
                    chappe.publish(TOPIC_IMU_TORSO, "marengo-pi", "marengo.v1.ImuSample", &msg)
                {
                    warn!(error = %err, "failed to publish ImuSample");
                } else {
                    debug!(real = q.real, "published ImuSample");
                }
            }
            Ok(None) => {
                consecutive_errors = 0;
            }
            Err(err) => {
                consecutive_errors += 1;
                warn!(error = %err, consecutive_errors, "IMU poll failed");
                if consecutive_errors >= 10 {
                    return Err(format!(
                        "IMU poll failed {consecutive_errors} times in a row: {err}"
                    ));
                }
            }
        }

        let elapsed = tick.elapsed();
        if elapsed < poll_period {
            thread::sleep(poll_period - elapsed);
        }
    }
    Ok(())
}
