use clap::Parser;

use args::Args;
use eyre::Result;
use file_ops::process_directory;
mod args;
mod file_ops;
mod platform;

fn main() -> Result<()> {
    let args = Args::parse();
    process_directory(&args.operation)?;
    Ok(())
}
