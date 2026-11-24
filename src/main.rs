//! Entry point for the `bumper` binary.
//!
//! Parses the command line and delegates to [`sco1_bumper::run`], using the current working
//! directory as the project root.
use std::{env, io, process::ExitCode};

use anyhow::{Context, Result};
use clap::Parser;
use sco1_bumper::{cli::Cli, run};

/// Run the CLI, printing any error chain to stderr and mapping the outcome to an exit code.
fn main() -> ExitCode {
    match try_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Fallible body of [`main`], separated so errors can be propagated with `?`.
fn try_main() -> Result<()> {
    let cli = Cli::parse();

    let root = env::current_dir().context("Could not determine the current working directory")?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    run(cli.command, &root, &mut out)
}
