use std::{fs, path::Path};

use semver::Version;
use tempfile::TempDir;

use super::{Start, create, render};
use crate::{
    config::{BUMPER_CONFIG_FILE, BumpRule, Config, RuleKind, VersioningType, load},
    version::YearMonth,
};

const TODAY: YearMonth = YearMonth {
    year: 2026,
    month: 11,
};

fn read_config(root: &Path) -> String {
    fs::read_to_string(root.join(BUMPER_CONFIG_FILE))
        .expect("could not read generated configuration")
}

/// The configuration `create` is expected to produce from `Start::default_for`.
fn default_config(current_version: Version, versioning_type: VersioningType) -> Config {
    Config {
        current_version,
        versioning_type,
        files: vec![BumpRule {
            file: "./Cargo.toml".to_owned(),
            kind: RuleKind::Key("package.version".to_owned()),
        }],
    }
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
fn create_writes_a_loadable_default_configuration() {
    let cases = [
        (VersioningType::Semver, Version::new(0, 1, 0)),
        (VersioningType::Calver, Version::new(2026, 11, 0)),
    ];

    for (versioning_type, expected_version) in cases {
        let temp_dir = TempDir::new().expect("could not create temporary directory");
        let start = Start::default_for(versioning_type, TODAY);

        create(temp_dir.path(), versioning_type, &start, false)
            .expect("create should write configuration");

        let config = load(temp_dir.path()).expect("generated configuration should load");
        assert_eq!(
            config,
            default_config(expected_version, versioning_type),
            "case: {versioning_type:?}"
        );
    }
}

#[test]
fn create_preserves_an_existing_configuration_without_override() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");
    let original = "[tool.bumper]\ncurrent_version = \"9.9.9\"\n";
    fs::write(temp_dir.path().join(BUMPER_CONFIG_FILE), original)
        .expect("could not write test configuration");

    let start = Start::default_for(VersioningType::Semver, TODAY);
    let err = create(temp_dir.path(), VersioningType::Semver, &start, false)
        .expect_err("create should reject an existing configuration");

    assert_eq!(
        err.to_string(),
        "Configuration file already exists: '.bumper.toml'"
    );
    assert_eq!(read_config(temp_dir.path()), original);
}

#[test]
fn create_overwrites_an_existing_configuration_when_requested() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");
    // Longer than the generated configuration, so the file must be truncated rather than
    // partially overwritten
    let existing = "# existing configuration\n".repeat(50);
    fs::write(temp_dir.path().join(BUMPER_CONFIG_FILE), existing)
        .expect("could not write test configuration");

    let start = Start::default_for(VersioningType::Semver, TODAY);
    create(temp_dir.path(), VersioningType::Semver, &start, true)
        .expect("create should overwrite configuration when requested");

    assert_eq!(
        read_config(temp_dir.path()),
        render(VersioningType::Semver, &start)
    );
}
