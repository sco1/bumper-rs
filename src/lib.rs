//! Automatically increment the project's version number.
//!
//! This crate is intended to be used via its `bumper` binary, which is a thin wrapper around
//! [`cli_main`]. No library API is provided; all other items are internal and are not covered by
//! SemVer guarantees.

// The library is organized as a small pipeline:
//
// 1. `config` loads the `[tool.bumper]` table from `.bumper.toml`.
// 2. `version` computes the next version from the current one, the configured `VersioningType`, and
//    the requested `BumpType`.
// 3. `bump` plans the per-file text replacements and then either applies them or previews them as a
//    diff.
// 4. After applying, `lock` notes if `Cargo.lock` no longer matches the bumped `Cargo.toml`
//    version. This is skipped for previews and with `--no-check-lock`.
//
// `run` dispatches each `cli::Command`: the `bump` subcommand runs the steps above, while the
// `init` subcommand uses `cargo` to find the version declared in `Cargo.toml` and `init` to write a
// starter `.bumper.toml` from it.
//
// `config`, `cargo`, and `lock` read files that may be absent through the shared helpers in
// `files`.
use std::{
    env,
    io::{self, Write},
    path::Path,
    process::ExitCode,
};

use anyhow::{Context, Result, bail};
use clap::Parser;

mod bump;
mod cargo;
mod cli;
mod config;
mod diff;
mod files;
mod init;
mod lock;
mod version;

use bump::{FileChange, apply, plan, preview};
use cargo::CARGO_MANIFEST_FILE;
use cli::{Cli, Command};
use config::{BUMPER_CONFIG_FILE, VersioningType};
use version::{YearMonth, parse_version};

/// Run the `bumper` CLI using the process arguments and the current working directory as the
/// project root.
///
/// Any error chain is printed to stderr and mapped to [`ExitCode::FAILURE`].
pub fn cli_main() -> ExitCode {
    match try_cli_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Fallible body of [`cli_main`], separated so errors can be propagated with `?`.
fn try_cli_main() -> Result<()> {
    let cli = Cli::parse();

    let root = env::current_dir().context("Could not determine the current working directory")?;
    let stdout = io::stdout();
    let mut out = stdout.lock();

    run(cli.command, &root, &mut out)
}

/// Execute a parsed CLI [`Command`] against the project whose base directory is `root`.
///
/// User-facing progress messages are written to `out`.
///
/// # Errors
///
/// Returns an error if the subcommand fails, e.g. the configuration cannot be loaded or parsed, the
/// requested bump is incompatible with the versioning scheme, or a target file cannot be read or
/// written.
fn run(command: Command, root: &Path, out: &mut impl Write) -> Result<()> {
    match command {
        Command::Bump {
            bump,
            dry_run,
            strict,
            check_lock,
        } => run_bump(root, bump, dry_run, strict, check_lock, out),
        Command::Init {
            versioning_type,
            ignore_existing,
        } => run_init(root, versioning_type, ignore_existing, out),
    }
}

/// Handle the `bump` subcommand.
///
/// Loads the configuration, computes the next version, and plans replacements for every configured
/// rule plus the `.bumper.toml` version itself. All files are read and planned before any are
/// written, so a planning failure leaves the working tree untouched.
///
/// Rules that match nothing are reported as warnings. With `strict`, they are an error instead,
/// raised before anything is previewed or written.
///
/// With `dry_run`, the planned changes are previewed as a diff and nothing is written. Otherwise
/// the changes are applied and, if `check_lock` is set, `Cargo.lock` is checked against the bumped
/// `Cargo.toml` version.
fn run_bump(
    root: &Path,
    bump_type: version::BumpType,
    dry_run: bool,
    strict: bool,
    check_lock: bool,
    out: &mut impl Write,
) -> Result<()> {
    let config = config::load(root)?;
    let next = version::next_version(&config.current_version, config.versioning_type, bump_type)?;

    let mut rules = config.files.clone();
    rules.push(config::self_rule());

    let changes = plan(root, &rules, &config.current_version, &next)?;
    let misses: Vec<String> = changes.iter().flat_map(FileChange::misses).collect();

    if strict && !misses.is_empty() {
        bail!(
            "{} rule(s) matched nothing; no files were changed:\n  {}",
            misses.len(),
            misses.join("\n  ")
        );
    }

    writeln!(
        out,
        "Bumper bumping: v{} -> v{}",
        config.current_version, next
    )?;
    for miss in &misses {
        writeln!(out, "WARNING: {miss}")?;
    }

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

/// Handle the `init` subcommand.
///
/// The generated configuration starts from the version declared in `Cargo.toml`, if any, so the
/// first bump updates it; otherwise it starts from the default for `versioning_type`. The starting
/// version, and where it came from, is reported to `out`.
fn run_init(
    root: &Path,
    versioning_type: VersioningType,
    ignore_existing: bool,
    out: &mut impl Write,
) -> Result<()> {
    let manifest = cargo::load_manifest(root)?;

    let (start, origin) = match manifest.as_ref().and_then(cargo::version_field) {
        Some((key, version)) => {
            let current_version = parse_version(version).with_context(|| {
                format!("Could not use {CARGO_MANIFEST_FILE} `{key}` as the starting version")
            })?;
            let origin = format!("from {CARGO_MANIFEST_FILE} `{key}`");
            (
                init::Start {
                    current_version,
                    key,
                },
                origin,
            )
        }
        None => {
            let start = init::Start::default_for(versioning_type, YearMonth::now_utc());
            let origin = "default; update it to match your project before bumping".to_owned();
            (start, origin)
        }
    };

    init::create(root, versioning_type, &start, ignore_existing)?;
    writeln!(
        out,
        "Created {BUMPER_CONFIG_FILE} with current_version {} ({origin})",
        start.current_version
    )?;

    Ok(())
}
