//! Reading files that may be absent, such as `Cargo.toml` and `Cargo.lock`.
use std::{fs, io::ErrorKind, path::Path};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;

/// Read `path` to a string, returning `Ok(None)` if the file does not exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read.
pub(crate) fn read_if_present(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("Could not read '{}'", path.display())),
    }
}

/// Parse the TOML file at `path` into `T`, returning `Ok(None)` if the file does not exist.
///
/// # Errors
///
/// Returns an error if the file exists but cannot be read, or cannot be parsed as `T`.
pub(crate) fn load_toml<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let Some(contents) = read_if_present(path)? else {
        return Ok(None);
    };

    let value = toml::from_str(&contents)
        .with_context(|| format!("Could not parse '{}'", path.display()))?;
    Ok(Some(value))
}

#[cfg(test)]
#[path = "unit_tests/files_tests.rs"]
mod tests;
