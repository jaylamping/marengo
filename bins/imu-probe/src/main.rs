//! Read BNO085 rotation quaternions over I2C and print samples to stdout.

use std::env;
use std::process;
use std::time::Duration;

use marengo_imu::{DEFAULT_I2C_ADDRESS, DEFAULT_REPORT_INTERVAL_US};
use marengo_support::init_tracing;

const DEFAULT_BUS: &str = "/dev/i2c-1";
const DEFAULT_SAMPLES: usize = 10;
const DEFAULT_TIMEOUT_SECS: u64 = 5;

#[cfg_attr(not(all(target_os = "linux", feature = "linux-i2c")), allow(dead_code))]
struct Args {
    bus: String,
    address: u16,
    samples: usize,
    report_interval_us: u32,
    timeout: Duration,
}

fn print_usage() {
    eprintln!(
        "Usage: imu-probe [--bus PATH] [--address HEX] [--samples N] [--report-interval-us US] [--timeout SEC]\n\
         Defaults: bus={DEFAULT_BUS}, address={DEFAULT_I2C_ADDRESS:#x}, samples={DEFAULT_SAMPLES}, \
         report-interval-us={DEFAULT_REPORT_INTERVAL_US}, timeout={DEFAULT_TIMEOUT_SECS}s"
    );
}

fn parse_args() -> Result<Args, String> {
    let mut bus = DEFAULT_BUS.to_string();
    let mut address = DEFAULT_I2C_ADDRESS;
    let mut samples = DEFAULT_SAMPLES;
    let mut report_interval_us = DEFAULT_REPORT_INTERVAL_US;
    let mut timeout_secs = DEFAULT_TIMEOUT_SECS;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--bus" => {
                bus = iter
                    .next()
                    .ok_or_else(|| "--bus requires a path".to_string())?;
            }
            "--address" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--address requires a hex value".to_string())?;
                address = u16::from_str_radix(raw.trim_start_matches("0x"), 16)
                    .map_err(|_| format!("invalid --address: {raw}"))?;
            }
            "--samples" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--samples requires a number".to_string())?;
                samples = raw
                    .parse()
                    .map_err(|_| format!("invalid --samples: {raw}"))?;
            }
            "--report-interval-us" => {
                let raw = iter.next().ok_or_else(|| {
                    "--report-interval-us requires a microsecond value".to_string()
                })?;
                report_interval_us = raw
                    .parse()
                    .map_err(|_| format!("invalid --report-interval-us: {raw}"))?;
            }
            "--timeout" => {
                let raw = iter
                    .next()
                    .ok_or_else(|| "--timeout requires seconds".to_string())?;
                timeout_secs = raw
                    .parse()
                    .map_err(|_| format!("invalid --timeout: {raw}"))?;
            }
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    if samples == 0 {
        return Err("--samples must be > 0".to_string());
    }

    Ok(Args {
        bus,
        address,
        samples,
        report_interval_us,
        timeout: Duration::from_secs(timeout_secs),
    })
}

fn main() {
    init_tracing();

    let args = match parse_args() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("error: {err}");
            print_usage();
            process::exit(2);
        }
    };

    if let Err(err) = run(args) {
        eprintln!("imu-probe failed: {err}");
        process::exit(1);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(all(target_os = "linux", feature = "linux-i2c"))]
    {
        use marengo_imu::{Bno085, LinuxI2cBus};

        tracing::info!(
            bus = %args.bus,
            address = args.address,
            samples = args.samples,
            "opening BNO085"
        );

        let bus = LinuxI2cBus::open(&args.bus, args.address)?;
        let mut imu = Bno085::new(bus);
        imu.initialize()?;
        imu.enable_rotation_vector(args.report_interval_us)?;

        let mut collected = 0usize;
        let deadline = std::time::Instant::now() + args.timeout;
        while collected < args.samples && std::time::Instant::now() < deadline {
            if let Some(sample) = imu.poll()? {
                let q = sample.quaternion;
                println!(
                    "sample={} i={:.6} j={:.6} k={:.6} real={:.6} accuracy={:?}",
                    collected + 1,
                    q.i,
                    q.j,
                    q.k,
                    q.real,
                    sample.accuracy
                );
                collected += 1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        if collected == 0 {
            return Err(
                "no rotation vector samples received (check wiring, address, and i2c group)".into(),
            );
        }
        if collected < args.samples {
            eprintln!(
                "warning: received {collected}/{} samples before timeout",
                args.samples
            );
        }
        Ok(())
    }

    #[cfg(not(all(target_os = "linux", feature = "linux-i2c")))]
    {
        Err(format!(
            "imu-probe requires Linux with feature linux-i2c (build with --features linux-i2c); bus={} address={:#x}",
            args.bus, args.address
        )
        .into())
    }
}
