//! Disposable writer stand-in: records argv and exercises process boundaries.
//! Never imports robot code or modifies repository configuration.
use std::env;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> io::Result<()> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(env::var("WRITER_FIXTURE_LOG").expect("fixture log required"))?;
    writeln!(log, "{{\"pid\":{},\"args\":{:?}}}", std::process::id(), arguments)?;
    drop(log);
    let joint = arguments.windows(2)
        .find(|pair| pair[0] == "--joint")
        .map(|pair| pair[1].as_str())
        .unwrap_or("");
    match joint {
        "right_worker_stall" => thread::sleep(Duration::from_secs(10)),
        "right_worker_output" => io::stdout().write_all(&vec![b'x'; 128 * 1024])?,
        "right_worker_failure" => std::process::exit(17),
        _ => eprintln!("fixture accepted"),
    }
    Ok(())
}
