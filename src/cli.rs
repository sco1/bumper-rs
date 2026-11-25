//! Command-line interface definition.
use clap::{ArgAction, Parser, Subcommand};

use crate::{config::VersioningType, version::BumpType};

#[derive(Debug, Parser)]
#[command(
    name = "bumper",
    bin_name = "bumper",
    about = "Automatically increment the project's version number."
)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Bump the configured project version.
    Bump {
        /// Version component to bump.
        bump: BumpType,

        /// Preview the planned changes without modifying files.
        #[arg(long)]
        dry_run: bool,

        /// Fail without modifying files if any rule matches nothing.
        #[arg(long)]
        strict: bool,

        /// Do not check Cargo.lock after applying changes.
        #[arg(long = "no-check-lock", action = ArgAction::SetFalse)]
        check_lock: bool,
    },

    /// Generate a starter .bumper.toml configuration file.
    Init {
        /// Versioning scheme for the generated configuration.
        #[arg(long, default_value = "semver")]
        versioning_type: VersioningType,

        /// Overwrite an existing .bumper.toml file.
        #[arg(long)]
        ignore_existing: bool,
    },
}
