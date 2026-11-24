use std::{
    fs,
    io::{ErrorKind, Write},
    path::Path,
};

use anyhow::{Context, Result, anyhow, bail};
use toml::Value;

const CARGO_MANIFEST_FILE: &str = "Cargo.toml";
const CARGO_LOCK_FILE: &str = "Cargo.lock";

fn read_if_present(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err)
            .with_context(|| format!("Could not read '{}'", path.display())),
    }
}

/// Return the manifest package name and explicit version.
///
/// A virtual workspace has no `[package]` table and is skipped.
///
/// A package that inherits `version` from `[workspace.package]` is also skipped, as the effective
/// package version is not declared directly in this manifest.
fn manifest_package(manifest: &Value) -> Result<Option<(&str, &str)>> {
    let Some(package) = manifest.get("package") else { return Ok(None); };
    let package = package.as_table().ok_or_else(|| anyhow!("Cargo.toml [package] must be a table"))?;
    
    let name = package.get("name").and_then(Value::as_str).ok_or_else(|| anyhow!("Cargo.toml [package].name must be a string"))?;
    let version = match package.get("version") {
        Some(Value::String(version)) => version.as_str(),
        Some(Value::Table(version)) if version.get("workspace").and_then(Value::as_bool) == Some(true) => { return Ok(None); },
        _ => {bail!("Cargo.toml [package].version must be a string or set 'version.workspace = true'")},
    };

    Ok(Some((name, version)))
}

/// Return versions for package entries whose `name` exactly matches `name`.
///
/// Name normalization is deliberately not performed; if Cargo.lock contains `my_package` while
/// Cargo.toml declares `my-package`, that is not treated as a match.
fn locked_versions<'a>(lock: &'a Value, name: &str) -> Vec<&'a str> {
    let Some(packages) = lock.get("package").and_then(Value::as_array) else { return Vec::new(); };

    packages
        .iter()
        .filter_map(|package| {
            let package = package.as_table()?;
            if package.get("name").and_then(Value::as_str) != Some(name) { return None; }
            package.get("version").and_then(Value::as_str)
        })
        .collect()
}

/// Check whether Cargo.lock contains the version declared in Cargo.toml.
///
/// This check never changes either file. It prints a non-fatal note when the lockfile does not
/// contain an exact-name package entry at the manifest version.
pub(crate) fn check(root: &Path, out: &mut impl Write) -> Result<()> {
    let manifest_path = root.join(CARGO_MANIFEST_FILE);
    let Some(manifest_contents) = read_if_present(&manifest_path)? else { return Ok(()); };
    let manifest: Value = toml::from_str(&manifest_contents).with_context(|| format!("Could not parse '{}'", manifest_path.display()))?;
    let Some((name, manifest_version)) = manifest_package(&manifest)? else { return Ok(()); };

    let lock_path = root.join(CARGO_LOCK_FILE);
    let Some(lock_contents) = read_if_present(&lock_path)? else { return Ok(()); };
    let lock: Value = toml::from_str(&lock_contents).with_context(|| format!("Could not parse '{}'", lock_path.display()))?;

    let versions = locked_versions(&lock, name);
    if versions.iter().any(|version| *version == manifest_version) { return Ok(()); }

    if versions.is_empty() {
        writeln!(out, "NOTE: Cargo.lock has no package entry named '{name}'; run 'cargo update --workspace' to refresh Cargo.lock.")?;
        return Ok(());
    }

    if versions.len() == 1 {
        writeln!(out, "NOTE: Cargo.lock version for package '{name}' is '{}', but Cargo.toml has '{manifest_version}'; run 'cargo update --workspace' to refresh Cargo.lock.", versions[0])?;
        return Ok(());
    }

    let locked_versions = versions.iter().map(|version| format!("'{version}'")).collect::<Vec<_>>().join(", ");
    writeln!(out, "NOTE: Cargo.lock versions for package '{name}' are {locked_versions}, but Cargo.toml has '{manifest_version}'; run 'cargo update --workspace' to refresh Cargo.lock.")?;

    Ok(())
}


#[cfg(test)]
#[path = "lock_tests.rs"]
mod tests;
