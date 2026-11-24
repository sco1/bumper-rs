//! Automatically increment the project's version number.
//!
//! The binary (`bumper`) is a thin wrapper around this library.
//!
//! The library is organized as a small pipeline:
//!
//! 1. [`config`] loads the `[tool.bumper]` table from `.bumper.toml`.
//! 2. [`version`] computes the next version from the current one, the configured
//!    [`VersioningType`](config::VersioningType), and the requested
//!    [`BumpType`](version::BumpType).
//! 3. [`bump`] plans the per-file text replacements and then either applies them or previews them
//!    as a diff.
//!
//! [`run`] ties these steps together for each [`cli::Command`].
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

/// Dotted TOML key of the version field inside `.bumper.toml` itself.
///
/// A rule targeting this key is always appended to the configured rules so that the configuration
/// file is bumped alongside the project files.
const CONFIG_VERSION_KEY: &str = "tool.bumper.current_version";

/// Execute a parsed CLI [`Command`] against the project whsoe base directory is `root`.
///
/// User-facing progress messages are written to `out`.
///
/// # Errors
///
/// Returns an error if the subcommand fails, e.g. the configuration cannot be loaded or parsed, the
/// requested bump is incompatible with the versioning scheme, or a target file cannot be read or
/// written.
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

/// Handle the `bump` subcommand.
///
/// Loads the configuration, computes the next version, and plans replacements for every configured
/// rule plus the `.bumper.toml` version itself. All files are read and planned before any are
/// written, so a planning failure leaves the working tree untouched.
///
/// With `dry_run`, the planned changes are previewed as a diff and nothing is written. Otherwise
/// the changes are applied and, if `check_lock` is set, `Cargo.lock` is checked against the bumped
/// `Cargo.toml` version.
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
