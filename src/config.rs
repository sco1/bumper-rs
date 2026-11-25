//! Loading the `.bumper.toml` configuration file, and the rule that bumps its own version.
//!
//! The configuration lives under a `[tool.bumper]` table, with one `[[tool.bumper.files]]` entry
//! per replacement rule. See the README for the full format.
use std::{fmt, path::Path};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use semver::Version;
use serde::{Deserialize, Deserializer, de};

use crate::{files::read_if_present, version::parse_version};

/// Configuration file name, resolved relative to the project root.
pub(crate) const BUMPER_CONFIG_FILE: &str = ".bumper.toml";

/// Dotted TOML key of the version field inside `.bumper.toml` itself.
const CONFIG_VERSION_KEY: &str = "tool.bumper.current_version";

/// Placeholder in a `search` pattern that is substituted with a version string.
const VERSION_TOKEN: &str = "{current_version}";

/// Versioning scheme used by the project.
///
/// Written as `"semver"` or `"calver"`, both in the configuration file and on the command line;
/// [`Display`](fmt::Display) produces the same names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum VersioningType {
    Semver,
    Calver,
}

/// Display the scheme as it is written in the configuration and on the command line.
impl fmt::Display for VersioningType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self
            .to_possible_value()
            .expect("VersioningType has no skipped variants");
        f.write_str(value.get_name())
    }
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

/// Display the pattern as written in the configuration, with its token unrendered.
impl fmt::Display for SearchPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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

/// Describe the rule as it would be written in the configuration, e.g. `key 'package.version'`.
impl fmt::Display for RuleKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleKind::Search(search) => write!(f, "search '{search}'"),
            RuleKind::Key(key) => write!(f, "key '{key}'"),
        }
    }
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

/// Return the rule that bumps `current_version` in `.bumper.toml` itself.
///
/// This rule is always appended to the configured rules so that the configuration file is bumped
/// alongside the project files.
pub fn self_rule() -> BumpRule {
    BumpRule {
        file: BUMPER_CONFIG_FILE.to_owned(),
        kind: RuleKind::Key(CONFIG_VERSION_KEY.to_owned()),
    }
}

/// Require exactly one of `search` or `key` to be set, and a `search` to be a valid
/// [`SearchPattern`].
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
    /// Replacement rules, in the order they appear in the configuration file; empty if omitted.
    #[serde(default)]
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

    let Some(contents) = read_if_present(&cfg_file)? else {
        bail!("Configuration file '{BUMPER_CONFIG_FILE}' could not be located.");
    };

    let parsed: ConfigFile = toml::from_str(&contents)
        .with_context(|| format!("Could not parse configuration file '{BUMPER_CONFIG_FILE}'"))?;

    Ok(parsed.tool.bumper)
}

#[cfg(test)]
#[path = "unit_tests/config_tests.rs"]
mod tests;
