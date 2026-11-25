//! Post-bump consistency check between `Cargo.toml` and `Cargo.lock`.
//!
//! Bumping the manifest version leaves `Cargo.lock` stale until Cargo next refreshes it. This
//! module only detects that situation and prints a note; it never modifies either file.
use std::{io::Write, path::Path};

use anyhow::{Context, Result, anyhow, bail};
use toml::Value;

use crate::cargo::{load_manifest, read_if_present};

/// Cargo lockfile name, resolved relative to the project root.
const CARGO_LOCK_FILE: &str = "Cargo.lock";

/// Return the manifest package name and explicit version.
///
/// A virtual workspace has no `[package]` table and is skipped.
///
/// A package that inherits `version` from `[workspace.package]` is also skipped, as the effective
/// package version is not declared directly in this manifest.
fn manifest_package(manifest: &Value) -> Result<Option<(&str, &str)>> {
    let Some(package) = manifest.get("package") else {
        return Ok(None);
    };
    let package = package
        .as_table()
        .ok_or_else(|| anyhow!("Cargo.toml [package] must be a table"))?;

    let name = package
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Cargo.toml [package].name must be a string"))?;
    let version = match package.get("version") {
        Some(Value::String(version)) => version.as_str(),
        Some(Value::Table(version))
            if version.get("workspace").and_then(Value::as_bool) == Some(true) =>
        {
            return Ok(None);
        }
        _ => {
            bail!("Cargo.toml [package].version must be a string or set 'version.workspace = true'")
        }
    };

    Ok(Some((name, version)))
}

/// Return versions for package entries whose `name` exactly matches `name`.
///
/// Name normalization is deliberately not performed; if Cargo.lock contains `my_package` while
/// Cargo.toml declares `my-package`, that is not treated as a match.
fn locked_versions<'a>(lock: &'a Value, name: &str) -> Vec<&'a str> {
    let Some(packages) = lock.get("package").and_then(Value::as_array) else {
        return Vec::new();
    };

    packages
        .iter()
        .filter_map(|package| {
            let package = package.as_table()?;
            if package.get("name").and_then(Value::as_str) != Some(name) {
                return None;
            }
            package.get("version").and_then(Value::as_str)
        })
        .collect()
}

/// Check whether Cargo.lock contains the version declared in Cargo.toml.
///
/// This check never changes either file. It prints a non-fatal note when the lockfile does not
/// contain an exact-name package entry at the manifest version.
///
/// The check is silently skipped if either file is missing, if the manifest is a virtual
/// workspace, or if the package inherits its version from the workspace.
///
/// # Errors
///
/// Returns an error if either file exists but cannot be read or parsed, if the manifest's
/// `[package]` table is malformed, or if `out` cannot be written to.
pub(crate) fn check(root: &Path, out: &mut impl Write) -> Result<()> {
    let Some(manifest) = load_manifest(root)? else {
        return Ok(());
    };
    let Some((name, manifest_version)) = manifest_package(&manifest)? else {
        return Ok(());
    };

    let lock_path = root.join(CARGO_LOCK_FILE);
    let Some(lock_contents) = read_if_present(&lock_path)? else {
        return Ok(());
    };
    let lock: Value = toml::from_str(&lock_contents)
        .with_context(|| format!("Could not parse '{}'", lock_path.display()))?;

    let versions = locked_versions(&lock, name);
    if versions.contains(&manifest_version) {
        return Ok(());
    }

    if versions.is_empty() {
        writeln!(
            out,
            "NOTE: Cargo.lock has no package entry named '{name}'; \
            run 'cargo update --workspace' to refresh Cargo.lock."
        )?;
        return Ok(());
    }

    if versions.len() == 1 {
        writeln!(
            out,
            "NOTE: Cargo.lock version for package '{name}' is '{}', \
            but Cargo.toml has '{manifest_version}'; \
            run 'cargo update --workspace' to refresh Cargo.lock.",
            versions[0]
        )?;
        return Ok(());
    }

    let locked_versions = versions
        .iter()
        .map(|version| format!("'{version}'"))
        .collect::<Vec<_>>()
        .join(", ");
    writeln!(
        out,
        "NOTE: Cargo.lock versions for package '{name}' are {locked_versions}, \
        but Cargo.toml has '{manifest_version}'; \
        run 'cargo update --workspace' to refresh Cargo.lock."
    )?;

    Ok(())
}

#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
