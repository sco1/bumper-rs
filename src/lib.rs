//! Automatically increment the project's version number.
//!
//! The binary (`bumper`) is a thin wrapper around this library.
use std::{io::Write, path::Path};

use anyhow::Result;

mod diff;
mod lock;

pub mod bump;
pub mod cli;
pub mod config;
pub mod version;

use bump::{apply, plan, preview};
use cli::Command;
use config::{BUMPER_CONFIG_FILE, BumpRule, RuleKind};
use version::YearMonth;

const CONFIG_VERSION_KEY: &str = "tool.bumper.current_version";

pub fn run(command: Command, root: &Path, out: &mut impl Write) -> Result<()> {
    match command {
        Command::Bump {
            bump,
            dry_run,
            check_lock,
        } => run_bump(root, bump, dry_run, check_lock, out),
        Command::Init {
            versioning_type,
            ignore_existing,
        } => config::init(root, versioning_type, ignore_existing, YearMonth::now_utc()),
    }
}

fn run_bump(
    root: &Path,
    bump_type: version::BumpType,
    dry_run: bool,
    check_lock: bool,
    out: &mut impl Write,
) -> Result<()> {
    let config = config::load(root)?;
    let next = version::next_version(&config.current_version, config.versioning_type, bump_type)?;

    let mut rules = config.files.clone();
    rules.push(BumpRule {
        file: BUMPER_CONFIG_FILE.to_owned(),
        kind: RuleKind::Key(CONFIG_VERSION_KEY.to_owned()),
    });

    let changes = plan(root, &rules, &config.current_version, &next)?;

    writeln!(
        out,
        "Bumper bumping: v{} -> v{}",
        config.current_version, next
    )?;

    if dry_run {
        preview(&changes, out)?;
    } else {
        apply(&changes, out)?;

        if check_lock {
            lock::check(root, out)?;
        }
    }

    Ok(())
}
