use std::fs;

use semver::Version;
use tempfile::TempDir;

use super::{BUMPER_CONFIG_FILE, BumpRule, Config, VersioningType, load};

fn temp_dir_with_config(contents: &str) -> TempDir {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    fs::write(temp_dir.path().join(BUMPER_CONFIG_FILE), contents)
        .expect("could not write test configuration");

    temp_dir
}

/// Load the provided configuration, expecting failure, and return the full error chain.
///
/// NOTE: `err.to_string()` only renders the outermost context; the alternate format (`{:#}`) is
/// needed to see the underlying serde/TOML reason.
fn load_error(contents: &str) -> String {
    let temp_dir = temp_dir_with_config(contents);

    let err = load(temp_dir.path()).expect_err("configuration should be rejected");

    format!("{err:#}")
}

fn assert_load_error_contains(contents: &str, needle: &str) {
    let message = load_error(contents);

    assert!(
        message.contains(needle),
        "expected {needle:?} in error; got: {message}"
    );
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
fn load_accepts_a_valid_semver_configuration() {
    let temp_dir = temp_dir_with_config(
        r#"
[tool.bumper]
current_version = "2.1.1"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./pyproject.toml"
search = 'version = "{current_version}"'

[[tool.bumper.files]]
file = "./README.md"
search = "sco1-bumper/{current_version}"
"#,
    );

    let got = load(temp_dir.path()).expect("configuration should load");

    let expected = Config {
        current_version: Version::new(2, 1, 1),
        versioning_type: VersioningType::Semver,
        files: vec![
            BumpRule {
                file: "./pyproject.toml".to_owned(),
                search: r#"version = "{current_version}""#.to_owned(),
            },
            BumpRule {
                file: "./README.md".to_owned(),
                search: "sco1-bumper/{current_version}".to_owned(),
            },
        ],
    };

    assert_eq!(got, expected);
}

#[test]
fn load_accepts_a_valid_calver_configuration_with_no_explicit_rules() {
    let temp_dir = temp_dir_with_config(
        r#"
[tool.bumper]
current_version = "2026.11.0"
versioning_type = "calver"
files = []
"#,
    );

    let got = load(temp_dir.path()).expect("configuration should load");

    let expected = Config {
        current_version: Version::new(2026, 11, 0),
        versioning_type: VersioningType::Calver,
        files: vec![],
    };

    assert_eq!(got, expected);
}

#[test]
fn load_preserves_the_configured_file_path_spelling() {
    let temp_dir = temp_dir_with_config(
        r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
search = 'version = "{current_version}"'
"#,
    );

    let config = load(temp_dir.path()).expect("configuration should load");

    assert_eq!(config.files.len(), 1);
    assert_eq!(config.files[0].file, "./Cargo.toml");
}

#[test]
fn load_keeps_repeated_targets_in_source_order() {
    // Bump grouping relies on every rule being retained, in configured order, even when several
    // rules target the same file
    let temp_dir = temp_dir_with_config(
        r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
search = 'version = "{current_version}"'

[[tool.bumper.files]]
file = "./README.md"
search = "sco1-bumper/{current_version}"

[[tool.bumper.files]]
file = "./README.md"
search = "rev: v{current_version}"
"#,
    );

    let config = load(temp_dir.path()).expect("configuration should load");

    let expected = vec![
        BumpRule {
            file: "./Cargo.toml".to_owned(),
            search: r#"version = "{current_version}""#.to_owned(),
        },
        BumpRule {
            file: "./README.md".to_owned(),
            search: "sco1-bumper/{current_version}".to_owned(),
        },
        BumpRule {
            file: "./README.md".to_owned(),
            search: "rev: v{current_version}".to_owned(),
        },
    ];

    assert_eq!(config.files, expected);
}

#[test]
fn load_ignores_other_tool_tables_and_unknown_bumper_keys() {
    let temp_dir = temp_dir_with_config(
        r#"
[tool.black]
line-length = 100

[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"
files = []
future_option = "ignored"
"#,
    );

    let config = load(temp_dir.path()).expect("configuration should load");

    assert_eq!(config.current_version, Version::new(0, 1, 0));
    assert_eq!(config.versioning_type, VersioningType::Semver);
    assert!(config.files.is_empty());
}

#[test]
fn load_rejects_missing_tables_and_fields() {
    let cases = [
        ("missing tool table", "", "missing field `tool`"),
        (
            "missing bumper table",
            r#"
[tool.black]
line-length = 100
"#,
            "missing field `bumper`",
        ),
        (
            "missing current_version",
            r#"
[tool.bumper]
versioning_type = "semver"
files = []
"#,
            "missing field `current_version`",
        ),
        (
            "missing versioning_type",
            r#"
[tool.bumper]
current_version = "0.1.0"
files = []
"#,
            "missing field `versioning_type`",
        ),
        (
            "missing files",
            r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"
"#,
            "missing field `files`",
        ),
        (
            "file rule missing file",
            r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
search = 'version = "{current_version}"'
"#,
            "missing field `file`",
        ),
        (
            "file rule missing search",
            r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
"#,
            "missing field `search`",
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
fn load_rejects_unknown_file_rule_keys() {
    let message = load_error(
        r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
search = 'version = "{current_version}"'
unexpected = "not allowed"
"#,
    );

    assert!(
        message.contains("Could not parse configuration file '.bumper.toml'"),
        "expected configuration-file context; got: {message}"
    );
    assert!(
        message.contains("unknown field `unexpected`"),
        "expected an unknown-field error naming the key; got: {message}"
    );
}

#[test]
fn load_rejects_an_unknown_versioning_type() {
    assert_load_error_contains(
        r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "pep440"
files = []
"#,
        "unknown variant `pep440`",
    );
}

#[test]
fn load_rejects_a_mistyped_field() {
    assert_load_error_contains(
        r#"
[tool.bumper]
current_version = 1
versioning_type = "semver"
files = []
"#,
        "invalid type: integer `1`",
    );
}

#[test]
fn load_rejects_invalid_current_versions() {
    let cases = [
        "1.0",
        "v1.2.3",
        "01.2.3",
        "2025.01.0",
        "1.2.3-rc.1",
        "1.2.3+build.5",
    ];

    for current_version in cases {
        let contents = format!(
            r#"
[tool.bumper]
current_version = "{current_version}"
versioning_type = "semver"
files = []
"#
        );

        let message = load_error(&contents);

        assert!(
            message.contains(&format!("Invalid current_version '{current_version}'")),
            "case: {current_version}; error: {message}"
        );
    }
}

#[test]
fn load_errors_report_their_location() {
    // Leading newline puts [tool.bumper] on line 2 and versioning_type on line 4
    let message = load_error(
        r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "pep440"
files = []
"#,
    );

    assert!(message.contains("line 4"), "error: {message}");
}

#[test]
fn load_reports_invalid_toml_with_configuration_context() {
    let temp_dir = temp_dir_with_config(
        r#"
[tool.bumper
current_version = "0.1.0"
"#,
    );

    let err = load(temp_dir.path()).expect_err("invalid TOML should fail");

    assert!(
        err.to_string()
            .contains("Could not parse configuration file '.bumper.toml'"),
        "error: {err}",
    );
}
