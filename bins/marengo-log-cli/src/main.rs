//! Archive bench sessions, maintain SQLite log store on Pi.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
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

fn open_store(cli: &Cli) -> Result<Store, Box<dyn std::error::Error>> {
    let root = cli.root.clone().unwrap_or_else(resolve_marengo_root);
    let db = cli.db.clone().unwrap_or_else(resolve_db_path);
    Ok(Store::open(db, root)?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let cli = Cli::parse();
    let store = open_store(&cli)?;

    match cli.command {
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
    }
    Ok(())
}
