//! Archive bench sessions, maintain SQLite log store on Pi.
//! Candump inspection uses `marengo-candump` directly (no DB required).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use marengo_candump::{
    format_inspection_text, Candump, FramePage, InspectRequest, Inspection, TimestampMode,
};
use marengo_store::{
    import_journal, resolve_db_path, resolve_marengo_root, Store, DEFAULT_ARCHIVE_DAYS,
    DEFAULT_HOT_KEEP, JOURNAL_UNITS,
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

#[derive(Subcommand)]
enum Commands {
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
    /// Purge log rows and sessions older than N days.
    Purge {
        #[arg(long, default_value_t = DEFAULT_ARCHIVE_DAYS)]
        days: u32,
    },
    /// One-time import of existing hot log files.
    ImportLegacy {
        #[arg(long, default_value_t = DEFAULT_HOT_KEEP)]
        keep: usize,
    },
    /// Report log disk usage bytes (stdout).
    DiskUsage,
    /// Import systemd journal into log_events (marengo-* units).
    JournalImport,
    /// Inspect a candump capture (plain or gzip).
    Candump {
        #[command(subcommand)]
        action: CandumpAction,
    },
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

fn main() -> ExitCode {
    init_tracing();
    let cli = Cli::parse();
    let Cli { command, root, db } = cli;

    let result = match command {
        Commands::Candump { action } => run_candump(action),
        command => run_store_command(root, db, command),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_store_command(
    root: Option<PathBuf>,
    db: Option<PathBuf>,
    command: Commands,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = root.unwrap_or_else(resolve_marengo_root);
    let db = db.unwrap_or_else(resolve_db_path);
    let store = Store::open(db, root)?;
    match command {
        Commands::Session { action } => match action {
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
        },
        Commands::Archive { keep } => {
            let n = store.archive_hot_sessions(keep)?;
            println!("archived {n} hot files (keep {keep})");
        }
        Commands::Purge { days } => {
            let (logs, sessions) = store.purge_older_than_days(days)?;
            println!("purged {logs} log rows, {sessions} sessions (>{days} days)");
        }
        Commands::ImportLegacy { keep } => {
            let n = store.import_legacy_hot(keep)?;
            println!("imported {n} legacy sessions");
        }
        Commands::DiskUsage => {
            let bytes = store.log_disk_usage_bytes()?;
            println!("{bytes}");
        }
        Commands::JournalImport => {
            let n = import_journal(&store, JOURNAL_UNITS)?;
            println!("imported {n} journal lines");
        }
        Commands::Candump { .. } => unreachable!("candump handled before store open"),
    }
    Ok(())
}
