//! Interactive motor exercise REPL — all motion goes through Davout.

use std::env;
use std::path::PathBuf;

use davout::{JointCommand, Supervisor};
use robstride::MemoryBus;
use tracing::info;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn usage() {
    eprintln!(
        "motor-repl — bench motor exercise (Davout → robstride)\n\
         Usage:\n  \
           motor-repl status\n  \
           motor-repl home\n  \
           motor-repl enable <operator_id>\n  \
           motor-repl disable\n  \
           motor-repl jog <joint> <position_rad>\n\
         Prefer vcan or simulation before live CAN."
    );
}

fn main() {
    marengo_support::init_tracing();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
        std::process::exit(1);
    }

    let bus = MemoryBus::default();
    let mut sup = match Supervisor::from_repo(repo_root(), bus) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("config/supervisor: {e}");
            std::process::exit(1);
        }
    };

    match args[1].as_str() {
        "status" => {
            info!(mode = ?sup.mode(), "motor-repl status");
            println!("mode: {:?}", sup.mode());
        }
        "home" => {
            sup.set_homing_complete();
            println!("homing complete → Ready");
        }
        "enable" => {
            let op = args.get(2).map(String::as_str).unwrap_or("bench");
            if let Err(e) = sup.request_enable(true) {
                eprintln!("enable failed: {e}");
                std::process::exit(1);
            }
            println!("enabled (operator={op})");
        }
        "disable" => {
            if let Err(e) = sup.request_enable(false) {
                eprintln!("disable failed: {e}");
                std::process::exit(1);
            }
            println!("disabled");
        }
        "jog" => {
            let joint = args.get(2).map(String::as_str).unwrap_or_else(|| {
                eprintln!("missing joint name");
                std::process::exit(1);
            });
            let pos: f64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                eprintln!("missing or invalid position_rad");
                std::process::exit(1);
            });
            if let Err(e) = sup.send_joint_command(JointCommand {
                joint: joint.to_string(),
                position_rad: pos,
                velocity_rad_s: 0.0,
                torque_nm: 0.0,
            }) {
                eprintln!("jog failed: {e}");
                std::process::exit(1);
            }
            println!("jog {joint} → {pos} rad (memory bus)");
        }
        _ => {
            usage();
            std::process::exit(1);
        }
    }
}
