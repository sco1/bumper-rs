//! Reading Cargo's files from the project root.
use std::{fs, io::ErrorKind, path::Path};

use anyhow::{Context, Result};
use toml::Value;

/// Cargo manifest file name, resolved relative to the project root.
pub(crate) const CARGO_MANIFEST_FILE: &str = "Cargo.toml";

/// Dotted key of a package's own version.
pub(crate) const PACKAGE_VERSION_KEY: &str = "package.version";

/// Dotted key of the version that workspace members can inherit.
pub(crate) const WORKSPACE_VERSION_KEY: &str = "workspace.package.version";

/// Read `path` to a string, returning `Ok(None)` if the file does not exist.
pub(crate) fn read_if_present(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("Could not read '{}'", path.display())),
    }
}

/// Parse `Cargo.toml` in `root`, returning `Ok(None)` if there is none.
///
/// # Errors
///
/// Returns an error if the manifest exists but cannot be read or is not valid TOML.
pub(crate) fn load_manifest(root: &Path) -> Result<Option<Value>> {
    let path = root.join(CARGO_MANIFEST_FILE);
    let Some(contents) = read_if_present(&path)? else {
        return Ok(None);
    };

    let manifest = toml::from_str(&contents)
        .with_context(|| format!("Could not parse '{}'", path.display()))?;
    Ok(Some(manifest))
}

/// Find the version declared in `manifest`, along with the dotted key that holds it.
///
/// `[package].version` is preferred. `[workspace.package].version` is used for a virtual
/// workspace, or for a package that inherits its version from a workspace in the same manifest.
/// Returns `None` if neither is a string. The version is returned as written, without validation.
pub(crate) fn version_field(manifest: &Value) -> Option<(&'static str, &str)> {
    [PACKAGE_VERSION_KEY, WORKSPACE_VERSION_KEY]
        .into_iter()
        .find_map(|key| {
            let version = key
                .split('.')
                .try_fold(manifest, |item, part| item.get(part))?
                .as_str()?;
            Some((key, version))
        })
}

#[cfg(test)]
#[path = "cargo_tests.rs"]
mod tests;
