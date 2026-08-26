use std::time::Duration;

use clap::Parser;

use args::Args;
use eyre::Result;
use file_ops::process_directory;
mod args;
mod file_ops;
mod platform;

const ANSI_PROGRESS_CHARS: &str = "█▇▆▅▄▃▂▁";
const PROGRESS_CHARS: &str = "=> ";
const TICK_DURATION: Duration = Duration::from_millis(100);
const TICK_CHARS: &str = r#"-\|/"#;

fn main() -> Result<()> {
    let args = Args::parse();
    if let Err(e) = process_directory(&args.operation) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    Ok(())
}
