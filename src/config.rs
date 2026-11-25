//! Loading and generating the `.bumper.toml` configuration file.
//!
//! The configuration lives under a `[tool.bumper]` table, with one `[[tool.bumper.files]]` entry
//! per replacement rule. See the README for the full format.
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

/// Configuration file name, resolved relative to the project root.
pub(crate) const BUMPER_CONFIG_FILE: &str = ".bumper.toml";

/// Placeholder in a `search` pattern that is substituted with a version string.
const VERSION_TOKEN: &str = "{current_version}";

/// Versioning scheme used by the project.
///
/// Deserialized from the lowercase strings `"semver"` and `"calver"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum VersioningType {
    Semver,
    Calver,
}

/// A `search` pattern containing at least one `{current_version}` token.
///
/// Constructed only via [`TryFrom<String>`], so every instance is guaranteed to reference the
/// version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPattern(String);

impl SearchPattern {
    /// Return the pattern with every `{current_version}` token replaced by `version`.
    pub fn render(&self, version: &str) -> String {
        self.0.replace(VERSION_TOKEN, version)
    }
}

impl TryFrom<String> for SearchPattern {
    type Error = String;

    /// Validate that `pattern` contains the `{current_version}` token.
    ///
    /// # Errors
    ///
    /// Returns an error if `pattern` does not contain `{current_version}`.
    fn try_from(pattern: String) -> Result<Self, Self::Error> {
        if pattern.contains(VERSION_TOKEN) {
            Ok(Self(pattern))
        } else {
            Err(format!(
                "`search` pattern '{pattern}' must contain `{VERSION_TOKEN}`"
            ))
        }
    }
}

/// How a [`BumpRule`] locates the version to replace within its target file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleKind {
    /// Plain-text search pattern containing a `{current_version}` token.
    ///
    /// Every occurrence of the pattern, with the token substituted by the current version, is
    /// replaced by the same pattern with the token substituted by the next version.
    Search(SearchPattern),
    /// Dotted TOML key path, e.g. `package.version`.
    ///
    /// Only the string value at that key is updated, and only if it equals the current version.
    Key(String),
}

/// Raw `[[tool.bumper.files]]` entry, validated into a [`BumpRule`] on deserialization.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBumpRule {
    file: String,
    search: Option<String>,
    key: Option<String>,
}

/// A single replacement rule targeting one file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawBumpRule")]
pub struct BumpRule {
    /// Path to the target file, relative to the project root.
    pub file: String,
    /// How the version is located within `file`.
    pub kind: RuleKind,
}

/// Require exactly one of `search` or `key` to be set.
impl TryFrom<RawBumpRule> for BumpRule {
    type Error = String; // anything that implements Display

    fn try_from(raw: RawBumpRule) -> Result<Self, Self::Error> {
        let kind = match (raw.search, raw.key) {
            (Some(search), None) => RuleKind::Search(
                SearchPattern::try_from(search)
                    .map_err(|e| format!("rule for '{}': {e}", raw.file))?,
            ),
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

/// Parsed contents of the `[tool.bumper]` table; unknown keys in [tool.bumper] are intentionally
/// ignored
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    /// The project's current release version.
    #[serde(deserialize_with = "deserialize_release_version")]
    pub current_version: Version,
    /// Versioning scheme used to compute the next version.
    pub versioning_type: VersioningType,
    /// Replacement rules, in the order they appear in the configuration file.
    pub files: Vec<BumpRule>,
}

// Wrappers for the [tool.bumper] nesting; other [tool.*] tables are ignored
/// Top level of the configuration file.
#[derive(Deserialize)]
struct ConfigFile {
    tool: Tool,
}

/// The `[tool]` table of the configuration file.
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

/// Load the [`Config`] from `.bumper.toml` in `root`.
///
/// # Errors
///
/// Returns an error if the file is missing or unreadable, is not valid TOML, lacks a
/// `[tool.bumper]` table, has an invalid `current_version`, or contains a malformed rule.
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

/// Render the starter configuration used by [`init`].
///
/// The starting version is `0.1.0` for SemVer, or `<year>.<month>.0` from `today` for CalVer. The
/// template includes a single rule that bumps `package.version` in `./Cargo.toml`.
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

/// Write a starter `.bumper.toml` into `root`.
///
/// `today` sets the starting version for CalVer projects. An existing configuration file is only
/// replaced when `ignore_existing` is set; otherwise an existing file is never clobbered.
///
/// # Errors
///
/// Returns an error if the configuration file already exists (and `ignore_existing` is not set), or
/// if it cannot be written.
pub fn init(
    root: &Path,
    versioning_type: VersioningType,
    ignore_existing: bool,
    today: YearMonth,
) -> Result<()> {
    let cfg_file = root.join(BUMPER_CONFIG_FILE);
    let contents = init_template(versioning_type, today);

    if ignore_existing {
        fs::write(&cfg_file, contents).with_context(|| {
            format!(
                "Could not write configuration file '{}'",
                cfg_file.display()
            )
        })?;
        return Ok(());
    }

    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&cfg_file)
    {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            bail!("Configuration file already exists: '{BUMPER_CONFIG_FILE}'")
        }
        Err(e) => {
            return Err(e).with_context(|| {
                format!(
                    "Could not write configuration file '{}'",
                    cfg_file.display()
                )
            });
        }
    };

    file.write_all(contents.as_bytes()).with_context(|| {
        format!(
            "Could not write configuration file '{}'",
            cfg_file.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
