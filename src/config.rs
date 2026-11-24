use std::{
    fs,
    io::{ErrorKind, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use semver::Version;
use serde::{Deserialize, Deserializer, de};

use crate::version::{YearMonth, parse_version};

pub(crate) const BUMPER_CONFIG_FILE: &str = ".bumper.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum VersioningType {
    Semver,
    Calver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleKind {
    Search(String),
    Key(String),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBumpRule {
    file: String,
    search: Option<String>,
    key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawBumpRule")]
pub struct BumpRule {
    pub file: String,
    pub kind: RuleKind,
}

impl TryFrom<RawBumpRule> for BumpRule {
    type Error = String; // anything that implements Display

    fn try_from(raw: RawBumpRule) -> Result<Self, Self::Error> {
        let kind = match (raw.search, raw.key) {
            (Some(search), None) => RuleKind::Search(search),
            (None, Some(key)) => RuleKind::Key(key),
            (Some(_), Some(_)) => {
                return Err(format!(
                    "rule for '{}' sets both `search` and `key`; use only one",
                    raw.file
                ));
            }
            (None, None) => {
                return Err(format!(
                    "rule for '{}' needs either `search` or `key`",
                    raw.file
                ));
            }
        };
        Ok(BumpRule {
            file: raw.file,
            kind,
        })
    }
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

fn init_template(versioning_type: VersioningType, today: YearMonth) -> String {
    let current_version = match versioning_type {
        VersioningType::Semver => "0.1.0".to_owned(),
        VersioningType::Calver => format!("{}.{}.0", today.year, today.month),
    };

    let versioning_type = match versioning_type {
        VersioningType::Semver => "semver",
        VersioningType::Calver => "calver",
    };

    format!(
        r#"[tool.bumper]
current_version = "{current_version}"
versioning_type = "{versioning_type}"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"
"#
    )
}

pub fn init(root: &Path, versioning_type: VersioningType, ignore_existing: bool, today: YearMonth) -> Result<()> {
    let cfg_file = root.join(BUMPER_CONFIG_FILE);
    let contents = init_template(versioning_type, today);

    if ignore_existing {
        fs::write(&cfg_file, contents).with_context(|| format!("Could not write configuration file '{}'", cfg_file.display()))?;
        return Ok(());
    }

    let mut file = match fs::OpenOptions::new().write(true).create_new(true).open(&cfg_file) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {bail!("Configuration file already exists: '{BUMPER_CONFIG_FILE}'")},
        Err(e) => {return Err(e).with_context(|| format!("Could not write configuration file '{}'", cfg_file.display()));},
    };

    file.write_all(contents.as_bytes()).with_context(|| format!("Could not write configuration file '{}'", cfg_file.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
