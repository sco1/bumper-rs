use std::fs;

use semver::Version;
use tempfile::TempDir;

use super::{BUMPER_CONFIG_FILE, BumpRule, Config, RuleKind, SearchPattern, VersioningType, load};

fn pattern(search: &str) -> SearchPattern {
    SearchPattern::try_from(search.to_owned()).expect("test pattern should be valid")
}

fn search_rule(file: &str, search: &str) -> BumpRule {
    BumpRule {
        file: file.to_owned(),
        kind: RuleKind::Search(pattern(search)),
    }
}

fn key_rule(file: &str, key: &str) -> BumpRule {
    BumpRule {
        file: file.to_owned(),
        kind: RuleKind::Key(key.to_owned()),
    }
}

fn temp_dir_with_config(contents: &str) -> TempDir {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    fs::write(temp_dir.path().join(BUMPER_CONFIG_FILE), contents)
        .expect("could not write test configuration");

    temp_dir
}

fn load_str(contents: &str) -> anyhow::Result<Config> {
    load(temp_dir_with_config(contents).path())
}

/// Load the provided configuration, expecting failure, and return the full error chain.
///
/// NOTE: `err.to_string()` only renders the outermost context; the alternate format (`{:#}`) is
/// needed to see the underlying serde/TOML reason.
fn load_error(contents: &str) -> String {
    let err = load_str(contents).expect_err("configuration should be rejected");

    format!("{err:#}")
}

#[test]
fn load_rejects_a_missing_configuration_file() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    let err = load(temp_dir.path()).expect_err("missing configuration should fail");

    assert_eq!(
        err.to_string(),
        "Configuration file '.bumper.toml' could not be located."
    );
}

#[test]
fn load_keeps_search_and_key_rules_in_configured_order_and_spelling() {
    // Bump grouping and user-facing output rely on every rule being kept, in order, with its path
    // spelled exactly as configured
    let got = load_str(
        r#"
[tool.bumper]
current_version = "2.1.1"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"

[[tool.bumper.files]]
file = "./README.md"
search = "sco1-bumper/{current_version}"

[[tool.bumper.files]]
file = "README.md"
search = 'version = "{current_version}"'
"#,
    )
    .expect("configuration should load");

    let expected = Config {
        current_version: Version::new(2, 1, 1),
        versioning_type: VersioningType::Semver,
        files: vec![
            key_rule("./Cargo.toml", "package.version"),
            search_rule("./README.md", "sco1-bumper/{current_version}"),
            search_rule("README.md", r#"version = "{current_version}""#),
        ],
    };
    assert_eq!(got, expected);
}

#[test]
fn load_accepts_a_calver_configuration_without_files() {
    let got = load_str(
        r#"
[tool.bumper]
current_version = "2026.11.0"
versioning_type = "calver"
"#,
    )
    .expect("configuration without files should load");

    let expected = Config {
        current_version: Version::new(2026, 11, 0),
        versioning_type: VersioningType::Calver,
        files: vec![],
    };
    assert_eq!(got, expected);
}

#[test]
fn load_ignores_other_tool_tables_and_unknown_bumper_keys() {
    let got = load_str(
        r#"
[tool.black]
line-length = 100

[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"
future_option = "ignored"
"#,
    )
    .expect("configuration should load");

    let expected = Config {
        current_version: Version::new(0, 1, 0),
        versioning_type: VersioningType::Semver,
        files: vec![],
    };
    assert_eq!(got, expected);
}

#[test]
fn load_rejects_invalid_file_rules_with_their_location() {
    let cases = [
        (
            "both search and key",
            r#"
[[tool.bumper.files]]
file = "./Cargo.toml"
search = 'version = "{current_version}"'
key = "package.version"
"#,
            "rule for './Cargo.toml' sets both `search` and `key`",
        ),
        (
            "neither search nor key",
            r#"
[[tool.bumper.files]]
file = "./Cargo.toml"
"#,
            "rule for './Cargo.toml' needs either `search` or `key`",
        ),
        (
            "search without the version token",
            r#"
[[tool.bumper.files]]
file = "./README.md"
search = "v{current-version}"
"#,
            "rule for './README.md': `search` pattern 'v{current-version}' must contain",
        ),
        (
            "unknown rule key",
            r#"
[[tool.bumper.files]]
file = "./Cargo.toml"
search = 'version = "{current_version}"'
unexpected = "not allowed"
"#,
            "unknown field `unexpected`",
        ),
    ];

    for (case, rules, expected) in cases {
        let contents = format!(
            "[tool.bumper]\ncurrent_version = \"0.1.0\"\nversioning_type = \"semver\"\n{rules}"
        );

        let message = load_error(&contents);

        assert!(
            message.contains(expected),
            "case: {case}; expected {expected:?}; got: {message}"
        );
        assert!(
            message.contains("line "),
            "case: {case}; expected a location; got: {message}"
        );
    }
}

#[test]
fn load_rejects_missing_or_invalid_fields() {
    let cases = [
        ("missing tool table", "", "missing field `tool`"),
        (
            "missing bumper table",
            "[tool.black]\nline-length = 100\n",
            "missing field `bumper`",
        ),
        (
            "missing current_version",
            "[tool.bumper]\nversioning_type = \"semver\"\n",
            "missing field `current_version`",
        ),
        (
            "missing versioning_type",
            "[tool.bumper]\ncurrent_version = \"0.1.0\"\n",
            "missing field `versioning_type`",
        ),
        (
            "unknown versioning_type",
            "[tool.bumper]\ncurrent_version = \"0.1.0\"\nversioning_type = \"pep440\"\n",
            "unknown variant `pep440`",
        ),
        (
            "rule missing file",
            concat!(
                "[tool.bumper]\ncurrent_version = \"0.1.0\"\nversioning_type = \"semver\"\n",
                "[[tool.bumper.files]]\nkey = \"package.version\"\n",
            ),
            "missing field `file`",
        ),
    ];

    for (case, contents, expected) in cases {
        let message = load_error(contents);

        assert!(
            message.contains(expected),
            "case: {case}; expected {expected:?}; got: {message}"
        );
    }
}

#[test]
fn load_validates_current_version_as_a_release_version() {
    let message = load_error(
        "[tool.bumper]\ncurrent_version = \"1.2.3-rc.1\"\nversioning_type = \"semver\"\n",
    );

    assert!(
        message.contains("Invalid current_version '1.2.3-rc.1': prerelease versions"),
        "error: {message}"
    );
}

#[test]
fn load_reports_invalid_toml_with_configuration_context() {
    let err = load_str("[tool.bumper\ncurrent_version = \"0.1.0\"\n")
        .expect_err("invalid TOML should fail");

    assert!(
        err.to_string()
            .contains("Could not parse configuration file '.bumper.toml'"),
        "error: {err}"
    );
}

#[test]
fn search_pattern_renders_every_token() {
    let cases = [
        ("{current_version}", "1.2.3"),
        ("v{current_version}", "v1.2.3"),
        (
            "{current_version} and v{current_version}",
            "1.2.3 and v1.2.3",
        ),
    ];

    for (search, expected) in cases {
        assert_eq!(pattern(search).render("1.2.3"), expected, "case: {search}");
    }
}

#[test]
fn search_pattern_rejects_patterns_without_the_token() {
    let cases = [
        ("no token", "version"),
        ("empty", ""),
        ("misspelled token", "v{current-version}"),
        ("unbraced token", "v current_version"),
    ];

    for (case, search) in cases {
        let err = SearchPattern::try_from(search.to_owned()).expect_err("should be rejected");

        assert!(
            err.contains(&format!("'{search}'")) && err.contains("{current_version}"),
            "case: {case}; error: {err}"
        );
    }
}
