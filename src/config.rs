use std::{fs, io::ErrorKind, path::Path};

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::{Deserialize, Deserializer, de};

use crate::version::parse_version;

const BUMPER_CONFIG_FILE: &str = ".bumper.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersioningType {
    Semver,
    Calver,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BumpRule {
    pub file: String,
    pub search: String,
}

// Unknown keys in [tool.bumper] are intentionally ignored
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    #[serde(deserialize_with = "deserialize_release_version")]
    pub current_version: Version,
    pub versioning_type: VersioningType,
    pub files: Vec<BumpRule>,
}

// Wrappers for the [tool.bumper] nesting; other [tool.*] tables are ignored
#[derive(Deserialize)]
struct ConfigFile {
    tool: Tool,
}

#[derive(Deserialize)]
struct Tool {
    bumper: Config,
}

/// Deserialize a version string using the same strict release-only rules as `parse_version`.
fn deserialize_release_version<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;

    // Use the alternate format so the full anyhow context chain is retained in the serde error
    parse_version(&raw).map_err(|e| de::Error::custom(format!("{e:#}")))
}

pub fn load(root: &Path) -> Result<Config> {
    let cfg_file = root.join(BUMPER_CONFIG_FILE);

    let contents = match fs::read_to_string(&cfg_file) {
        Ok(contents) => contents,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            bail!("Configuration file '{BUMPER_CONFIG_FILE}' could not be located.");
        }
        Err(e) => {
            return Err(e).with_context(|| {
                format!("Could not read configuration file '{}'", cfg_file.display())
            });
        }
    };

    let parsed: ConfigFile = toml::from_str(&contents)
        .with_context(|| format!("Could not parse configuration file '{BUMPER_CONFIG_FILE}'"))?;

    Ok(parsed.tool.bumper)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
