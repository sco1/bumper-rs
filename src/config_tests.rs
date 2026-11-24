use std::fs;

use semver::Version;
use tempfile::TempDir;

use super::{
    BUMPER_CONFIG_FILE,
    BumpRule,
    Config,
    RuleKind,
    VersioningType,
    init,
    init_template,
    load,
};
use crate::version::YearMonth;

fn search_rule(file: &str, search: &str) -> BumpRule {
    BumpRule {
        file: file.to_owned(),
        kind: RuleKind::Search(search.to_owned()),
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

const FIXED_TODAY: YearMonth = YearMonth {
    year: 2026,
    month: 11,
};

const SEMVER_INIT_CONFIG: &str = concat!(
    "[tool.bumper]\n",
    "current_version = \"0.1.0\"\n",
    "versioning_type = \"semver\"\n",
    "\n",
    "[[tool.bumper.files]]\n",
    "file = \"./Cargo.toml\"\n",
    "key = \"package.version\"\n",
);

const CALVER_INIT_CONFIG: &str = concat!(
    "[tool.bumper]\n",
    "current_version = \"2026.11.0\"\n",
    "versioning_type = \"calver\"\n",
    "\n",
    "[[tool.bumper.files]]\n",
    "file = \"./Cargo.toml\"\n",
    "key = \"package.version\"\n",
);

fn read_config(root: &std::path::Path) -> String {
    fs::read_to_string(root.join(BUMPER_CONFIG_FILE))
        .expect("could not read generated configuration")
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
            search_rule("./pyproject.toml", r#"version = "{current_version}""#),
            search_rule("./README.md", "sco1-bumper/{current_version}"),
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
        search_rule("./Cargo.toml", r#"version = "{current_version}""#),
        search_rule("./README.md", "sco1-bumper/{current_version}"),
        search_rule("./README.md", "rev: v{current_version}"),
    ];

    assert_eq!(config.files, expected);
}

#[test]
fn load_accepts_key_rules_alongside_search_rules() {
    let temp_dir = temp_dir_with_config(
        r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"

[[tool.bumper.files]]
file = "./README.md"
search = "sco1-bumper/{current_version}"
"#,
    );

    let config = load(temp_dir.path()).expect("configuration should load");

    let expected = vec![
        key_rule("./Cargo.toml", "package.version"),
        search_rule("./README.md", "sco1-bumper/{current_version}"),
    ];

    assert_eq!(config.files, expected);
}

#[test]
fn load_requires_exactly_one_of_search_or_key() {
    let cases = [
        (
            "both search and key",
            r#"
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

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
[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
"#,
            "rule for './Cargo.toml' needs either `search` or `key`",
        ),
    ];

    for (case, contents, expected) in cases {
        let message = load_error(contents);

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

#[test]
fn init_template_generates_the_expected_semver_configuration() {
    let got = init_template(VersioningType::Semver, FIXED_TODAY);

    assert_eq!(got, SEMVER_INIT_CONFIG);
}

#[test]
fn init_template_generates_the_expected_calver_configuration() {
    let got = init_template(VersioningType::Calver, FIXED_TODAY);

    assert_eq!(got, CALVER_INIT_CONFIG);
}

#[test]
fn init_creates_a_semver_configuration_when_none_exists() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    init(
        temp_dir.path(),
        VersioningType::Semver,
        false,
        FIXED_TODAY,
    )
    .expect("init should create configuration");

    assert_eq!(read_config(temp_dir.path()), SEMVER_INIT_CONFIG);

    let config = load(temp_dir.path()).expect("generated configuration should load");

    assert_eq!(config.current_version, Version::new(0, 1, 0));
    assert_eq!(config.versioning_type, VersioningType::Semver);
    assert_eq!(
        config.files,
        vec![key_rule("./Cargo.toml", "package.version")]
    );
}

#[test]
fn init_preserves_an_existing_configuration_without_override() {
    let original = "[tool.bumper]\ncurrent_version = \"9.9.9\"\n";
    let temp_dir = temp_dir_with_config(original);

    let err = init(
        temp_dir.path(),
        VersioningType::Semver,
        false,
        FIXED_TODAY,
    )
    .expect_err("init should reject an existing configuration");

    assert_eq!(
        err.to_string(),
        "Configuration file already exists: '.bumper.toml'"
    );
    assert_eq!(read_config(temp_dir.path()), original);
}

#[test]
fn init_overwrites_an_existing_configuration_when_requested() {
    let temp_dir = temp_dir_with_config("existing configuration\n");

    init(
        temp_dir.path(),
        VersioningType::Calver,
        true,
        FIXED_TODAY,
    )
    .expect("init should overwrite configuration when requested");

    assert_eq!(read_config(temp_dir.path()), CALVER_INIT_CONFIG);

    let config = load(temp_dir.path()).expect("generated configuration should load");

    assert_eq!(config.current_version, Version::new(2026, 11, 0));
    assert_eq!(config.versioning_type, VersioningType::Calver);
    assert_eq!(
        config.files,
        vec![key_rule("./Cargo.toml", "package.version")]
    );
}
