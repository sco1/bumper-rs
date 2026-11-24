use std::{env, io, process::ExitCode};

use anyhow::{Context, Result};
use clap::Parser;

use sco1_bumper::{cli::Cli, run};

fn main() -> ExitCode {
    match try_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

fn try_main() -> Result<()> {
    let cli = Cli::parse();

    let root = env::current_dir().context("Could not determine the current working directory")?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    run(cli.command, &root, &mut out)
}
