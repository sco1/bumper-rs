use std::fs;

use semver::Version;
use tempfile::TempDir;

use super::{Start, create, render};
use crate::{
    config::{BUMPER_CONFIG_FILE, BumpRule, RuleKind, VersioningType, load},
    version::YearMonth,
};

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

fn semver_default() -> Start {
    Start::default_for(VersioningType::Semver, FIXED_TODAY)
}

fn calver_default() -> Start {
    Start::default_for(VersioningType::Calver, FIXED_TODAY)
}

fn read_config(root: &std::path::Path) -> String {
    fs::read_to_string(root.join(BUMPER_CONFIG_FILE))
        .expect("could not read generated configuration")
}

#[test]
fn start_defaults_to_the_versioning_scheme_starting_version() {
    let cases = [
        (VersioningType::Semver, Version::new(0, 1, 0)),
        (VersioningType::Calver, Version::new(2026, 11, 0)),
    ];

    for (versioning_type, expected) in cases {
        let start = Start::default_for(versioning_type, FIXED_TODAY);

        assert_eq!(
            start,
            Start {
                current_version: expected,
                key: "package.version",
            },
            "case: {versioning_type:?}"
        );
    }
}

#[test]
fn render_generates_the_expected_semver_configuration() {
    let got = render(VersioningType::Semver, &semver_default());

    assert_eq!(got, SEMVER_INIT_CONFIG);
}

#[test]
fn render_generates_the_expected_calver_configuration() {
    let got = render(VersioningType::Calver, &calver_default());

    assert_eq!(got, CALVER_INIT_CONFIG);
}

#[test]
fn render_uses_the_provided_version_and_key() {
    let start = Start {
        current_version: Version::new(1, 2, 3),
        key: "workspace.package.version",
    };

    let got = render(VersioningType::Semver, &start);

    assert_eq!(
        got,
        concat!(
            "[tool.bumper]\n",
            "current_version = \"1.2.3\"\n",
            "versioning_type = \"semver\"\n",
            "\n",
            "[[tool.bumper.files]]\n",
            "file = \"./Cargo.toml\"\n",
            "key = \"workspace.package.version\"\n",
        )
    );
}

#[test]
fn create_writes_a_semver_configuration_when_none_exists() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    create(
        temp_dir.path(),
        VersioningType::Semver,
        &semver_default(),
        false,
    )
    .expect("create should create configuration");

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
fn create_preserves_an_existing_configuration_without_override() {
    let original = "[tool.bumper]\ncurrent_version = \"9.9.9\"\n";
    let temp_dir = temp_dir_with_config(original);

    let err = create(
        temp_dir.path(),
        VersioningType::Semver,
        &semver_default(),
        false,
    )
    .expect_err("create should reject an existing configuration");

    assert_eq!(
        err.to_string(),
        "Configuration file already exists: '.bumper.toml'"
    );
    assert_eq!(read_config(temp_dir.path()), original);
}

#[test]
fn create_overwrites_an_existing_configuration_when_requested() {
    let temp_dir = temp_dir_with_config("existing configuration\n");

    create(
        temp_dir.path(),
        VersioningType::Calver,
        &calver_default(),
        true,
    )
    .expect("create should overwrite configuration when requested");

    assert_eq!(read_config(temp_dir.path()), CALVER_INIT_CONFIG);

    let config = load(temp_dir.path()).expect("generated configuration should load");

    assert_eq!(config.current_version, Version::new(2026, 11, 0));
    assert_eq!(config.versioning_type, VersioningType::Calver);
    assert_eq!(
        config.files,
        vec![key_rule("./Cargo.toml", "package.version")]
    );
}
