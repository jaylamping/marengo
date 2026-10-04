//! Archive bench sessions, maintain SQLite log store on Pi.
//! Candump inspection uses `marengo-candump` directly (no DB required).
//! Explicit historical recovery dispatches before opening the normal Store.
//! `gravity-fit` fits right-arm gravity and friction to `pi_joint_calibrate` /
//! `pi_gravity_calibrate` sessions (workstation, no DB).
//! `firmware-timing` measures Robstride firmware timing from candump captures (no DB).

mod firmware_timing;
mod gravity_fit;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use marengo_candump::{
    format_inspection_text, Candump, FramePage, InspectRequest, Inspection, TimestampMode,
};
use marengo_store::{
    import_journal, recover_known_v2, resolve_db_path, resolve_marengo_root, SessionArtifact,
    Store, DEFAULT_HOT_KEEP, JOURNAL_UNITS,
};
use marengo_support::init_tracing;

#[derive(Parser)]
#[command(name = "marengo-log-cli")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, env = "MARENGO_ROOT")]
    root: Option<PathBuf>,
    #[arg(long, env = "MARENGO_DB_PATH")]
    db: Option<PathBuf>,
}

// Subcommands that operate on the SQLite log store (a doc comment would leak into the root --help).
#[derive(Subcommand)]
enum StoreCommand {
    /// Register or update a bench session row.
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Archive hot files beyond keep count and gzip to blobs/.
    Archive {
        #[arg(long, default_value_t = DEFAULT_HOT_KEEP)]
        keep: usize,
    },
    /// Enforce the stored `log_archive_days` and `log_disk_budget_bytes` settings.
    Purge,
    /// One-time import of existing hot log files.
    ImportLegacy {
        #[arg(long, default_value_t = DEFAULT_HOT_KEEP)]
        keep: usize,
    },
    /// Import systemd journal into log_events (marengo-* units).
    JournalImport,
}

#[derive(Subcommand)]
enum Commands {
    #[command(flatten)]
    Store(StoreCommand),
    /// Preserve a verified backup and recover recognized complete v2 with stale marker1.
    RecoverKnownV2 {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Inspect a candump capture (plain or gzip).
    Candump {
        #[command(subcommand)]
        action: CandumpAction,
    },
    /// Fit right-arm gravity to `pi_joint_calibrate` / `pi_gravity_calibrate` sessions and
    /// propose a URDF patch, a `control.yaml` friction patch and a dated record.
    /// Exit 0 = URDF patch proposed, 2 = refused, 1 = error.
    GravityFit {
        /// Session directory (`var/gravity-calibration/<TS>`); repeat to fuse sweeps.
        #[arg(long = "dir", required = true)]
        dirs: Vec<PathBuf>,
        /// Parameter `mass:<link>` or `com:<link>`; repeat. Default: identifiable mass
        /// scales (then COM offsets) downstream of the swept joint.
        #[arg(long)]
        fit: Vec<String>,
        /// Joint whose torque enters the fit; repeat. Default: every joint.
        #[arg(long = "fit-joint")]
        fit_joints: Vec<String>,
        #[arg(long, required = true)]
        out_dir: PathBuf,
        /// Local URDF compared with the Pi base (reported, never modified).
        #[arg(long, required = true)]
        repo_urdf: PathBuf,
        /// `wave` bins local waves (lumped A·sin q + B·cos q per swept joint); `static` fits
        /// link inertials to holds; `auto` follows the plans' `method`.
        #[arg(long, value_enum, default_value_t = FitMethod::Auto)]
        method: FitMethod,
    },
    /// Measure Robstride firmware timing (Enable→Run, post-SetZero silence, reply
    /// latencies, report period) from `candump -L` or `candump -t z|a` captures.
    /// Exit 0 = ok, 1 = error.
    FirmwareTiming {
        /// Print the JSON profile instead of the text report.
        #[arg(long)]
        json: bool,
        #[arg(required = true)]
        captures: Vec<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum FitMethod {
    Auto,
    Wave,
    Static,
}

#[derive(Subcommand)]
enum SessionAction {
    Register {
        #[arg(long)]
        id: String,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        bench: Option<PathBuf>,
        #[arg(long)]
        candump: Option<PathBuf>,
        #[arg(long)]
        trace: Option<PathBuf>,
        #[arg(long)]
        started_ms: Option<u64>,
    },
    Finalize {
        #[arg(long)]
        id: String,
    },
    /// Remove one registered reference; preserve its file and sibling artifacts.
    ClearArtifact {
        #[arg(long)]
        id: String,
        #[arg(long, value_enum)]
        artifact: CliSessionArtifact,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliSessionArtifact {
    Bench,
    Candump,
    Trace,
}

impl From<CliSessionArtifact> for SessionArtifact {
    fn from(value: CliSessionArtifact) -> Self {
        match value {
            CliSessionArtifact::Bench => Self::Bench,
            CliSessionArtifact::Candump => Self::Candump,
            CliSessionArtifact::Trace => Self::Trace,
        }
    }
}

#[derive(Subcommand)]
enum CandumpAction {
    Summary(CandumpArgs),
    Page {
        #[command(flatten)]
        common: CandumpArgs,
        #[arg(long, default_value_t = 0)]
        offset: u64,
        #[arg(long, default_value_t = 200)]
        limit: u32,
    },
}

#[derive(clap::Args)]
struct CandumpArgs {
    #[arg(long)]
    file: PathBuf,
    #[arg(long, value_enum)]
    timestamp: CliTimestampMode,
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,
    #[arg(long)]
    enrich: bool,
    #[arg(long, requires = "enrich")]
    config_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliTimestampMode {
    Delta,
    Absolute,
}

impl From<CliTimestampMode> for TimestampMode {
    fn from(value: CliTimestampMode) -> Self {
        match value {
            CliTimestampMode::Delta => TimestampMode::Delta,
            CliTimestampMode::Absolute => TimestampMode::Absolute,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

fn build_candump(args: &CandumpArgs) -> Result<Candump, Box<dyn std::error::Error>> {
    if !args.enrich {
        return Ok(Candump::plain());
    }
    #[cfg(feature = "robstride-enrichment")]
    {
        let dir = args
            .config_dir
            .as_ref()
            .ok_or("--enrich requires --config-dir pointing at a motors.yaml directory")?;
        Ok(Candump::with_robstride_from_config_dir(dir)?)
    }
    #[cfg(not(feature = "robstride-enrichment"))]
    {
        let _ = &args.config_dir;
        Err("--enrich requires the robstride-enrichment feature".into())
    }
}

fn inspect_request(action: &CandumpAction) -> Result<InspectRequest, Box<dyn std::error::Error>> {
    match action {
        CandumpAction::Summary(args) => Ok(InspectRequest::summary(args.timestamp.into())),
        CandumpAction::Page {
            common,
            offset,
            limit,
        } => {
            let page = FramePage::new(*offset, *limit)?;
            Ok(InspectRequest::page(common.timestamp.into(), page))
        }
    }
}

fn candump_args(action: &CandumpAction) -> &CandumpArgs {
    match action {
        CandumpAction::Summary(args) => args,
        CandumpAction::Page { common, .. } => common,
    }
}

fn write_inspection(
    format: OutputFormat,
    inspection: &Inspection,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(std::io::stdout(), inspection)?;
            println!();
        }
        OutputFormat::Text => {
            print!("{}", format_inspection_text(inspection));
        }
    }
    Ok(())
}

fn run_candump(action: CandumpAction) -> Result<(), Box<dyn std::error::Error>> {
    let args = candump_args(&action);
    let request = inspect_request(&action)?;
    let candump = build_candump(args)?;
    let report = candump.inspect_path(&args.file, request)?;
    write_inspection(args.format, &report)?;
    Ok(())
}

fn run_recovery(
    source: PathBuf,
    backup: PathBuf,
    output: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let receipt = recover_known_v2(source, backup, output)?;
    let mut stdout = std::io::stdout().lock();
    let presented = (|| -> Result<(), Box<dyn std::error::Error>> {
        serde_json::to_writer_pretty(&mut stdout, &receipt)?;
        writeln!(stdout)?;
        stdout.flush()?;
        Ok(())
    })();
    presented.map_err(|error| {
        format!(
            "recovery artifacts completed but receipt output failed: {error}; retained published backup={}; retained published output={}; canonical paths: backup={:?}; output={:?}",
            receipt.backup_path.display(), receipt.output_path.display(),
            receipt.backup_path, receipt.output_path,
        ).into()
    })
}

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    let Cli { command, root, db } = cli;

    let result = match command {
        Commands::GravityFit {
            dirs,
            fit,
            fit_joints,
            out_dir,
            repo_urdf,
            method,
        } => {
            return run_gravity_fit(&gravity_fit::GravityFitArgs {
                dirs,
                fit,
                fit_joints,
                out_dir,
                repo_urdf,
                method: match method {
                    FitMethod::Auto => gravity_fit::Method::Auto,
                    FitMethod::Wave => gravity_fit::Method::Wave,
                    FitMethod::Static => gravity_fit::Method::Static,
                },
            })
        }
        Commands::Candump { action } => run_candump(action),
        Commands::FirmwareTiming { json, captures } => run_firmware_timing(json, &captures),
        Commands::RecoverKnownV2 {
            source,
            backup,
            output,
        } => run_recovery(source, backup, output),
        Commands::Store(command) => run_store_command(root, db, command),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_firmware_timing(json: bool, captures: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    let timing = firmware_timing::analyze(captures)?;
    let mut stdout = std::io::stdout().lock();
    if json {
        serde_json::to_writer_pretty(&mut stdout, &timing)?;
        writeln!(stdout)?;
    } else {
        write!(stdout, "{}", firmware_timing::format_text(&timing))?;
    }
    Ok(())
}

/// Exit 0 = patch proposed, 2 = refused (fit verdict or out-of-limit input), 1 = error.
fn run_gravity_fit(args: &gravity_fit::GravityFitArgs) -> ExitCode {
    match gravity_fit::run(args) {
        Ok(gravity_fit::Outcome::Proposed) => ExitCode::SUCCESS,
        Ok(gravity_fit::Outcome::Refused) => ExitCode::from(2),
        Err(err) if err.is_refusal() => {
            eprintln!("refused: {err}");
            ExitCode::from(2)
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_store_command(
    root: Option<PathBuf>,
    db: Option<PathBuf>,
    command: StoreCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root.unwrap_or_else(resolve_marengo_root);
    let db = db.unwrap_or_else(resolve_db_path);
    let store = Store::open(db, root)?;
    match command {
        StoreCommand::Session { action } => match action {
            SessionAction::Register {
                id,
                label,
                bench,
                candump,
                trace,
                started_ms,
            } => {
                let started = started_ms.unwrap_or_else(marengo_store::now_ms);
                store.register_session(
                    &id,
                    label.as_deref(),
                    started,
                    bench.as_deref(),
                    candump.as_deref(),
                    trace.as_deref(),
                )?;
                println!("registered session {id}");
            }
            SessionAction::Finalize { id } => {
                store.finalize_session(&id, marengo_store::now_ms())?;
                println!("finalized session {id}");
            }
            SessionAction::ClearArtifact { id, artifact } => {
                if store.clear_session_artifact(&id, artifact.into())? {
                    println!("cleared {artifact:?} reference for session {id}; file preserved");
                } else {
                    println!("no {artifact:?} reference to clear for session {id}");
                }
            }
        },
        StoreCommand::Archive { keep } => {
            let n = store.archive_hot_sessions(keep)?;
            println!("archived {n} hot files (keep {keep})");
        }
        StoreCommand::Purge => {
            let report = store.enforce_retention()?;
            println!(
                "purged {} log rows, {} sessions older than the archive window, {} sessions over the disk budget (usage {} of {} bytes)",
                report.log_rows,
                report.aged_sessions,
                report.budget_sessions,
                report.usage_bytes,
                report.budget_bytes
            );
        }
        StoreCommand::ImportLegacy { keep } => {
            let report = store.import_legacy_hot_report(keep)?;
            println!(
                "imported {} legacy sessions, {} artifacts",
                report.sessions, report.artifacts
            );
        }
        StoreCommand::JournalImport => {
            let n = import_journal(&store, JOURNAL_UNITS)?;
            println!("imported {n} journal lines");
        }
    }
    Ok(())
}
