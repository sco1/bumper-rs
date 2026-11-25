use std::fs;

use tempfile::TempDir;
use toml::Value;

use super::{load_manifest, version_field};

fn manifest(contents: &str) -> Value {
    toml::from_str(contents).expect("test manifest should be valid TOML")
}

#[test]
fn load_manifest_returns_none_without_a_cargo_toml() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    let got = load_manifest(temp_dir.path()).expect("a missing manifest is not an error");

    assert!(got.is_none());
}

#[test]
fn load_manifest_parses_an_existing_cargo_toml() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"example\"\nversion = \"1.2.3\"\n",
    )
    .expect("could not write test manifest");

    let got = load_manifest(temp_dir.path())
        .expect("manifest should load")
        .expect("manifest should be present");

    assert_eq!(version_field(&got), Some(("package.version", "1.2.3")));
}

#[test]
fn load_manifest_rejects_invalid_toml() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");
    fs::write(temp_dir.path().join("Cargo.toml"), "[package\n").expect("could not write manifest");

    let err = load_manifest(temp_dir.path()).expect_err("invalid TOML should be rejected");

    assert!(
        format!("{err:#}").contains("Could not parse"),
        "error: {err:#}"
    );
}

#[test]
fn version_field_finds_the_declared_version_and_its_key() {
    let cases = [
        (
            "package version",
            "[package]\nname = \"x\"\nversion = \"1.2.3\"\n",
            Some(("package.version", "1.2.3")),
        ),
        (
            "virtual workspace",
            "[workspace]\nmembers = [\"x\"]\n\n[workspace.package]\nversion = \"0.4.0\"\n",
            Some(("workspace.package.version", "0.4.0")),
        ),
        (
            "package inherits from the workspace in the same manifest",
            concat!(
                "[workspace.package]\nversion = \"0.4.0\"\n\n",
                "[package]\nname = \"x\"\nversion.workspace = true\n",
            ),
            Some(("workspace.package.version", "0.4.0")),
        ),
        (
            "package version wins over the workspace version",
            concat!(
                "[workspace.package]\nversion = \"0.4.0\"\n\n",
                "[package]\nname = \"x\"\nversion = \"1.2.3\"\n",
            ),
            Some(("package.version", "1.2.3")),
        ),
        (
            "package inherits from a workspace in another manifest",
            "[package]\nname = \"x\"\nversion.workspace = true\n",
            None,
        ),
        (
            "package without a version",
            "[package]\nname = \"x\"\n",
            None,
        ),
        (
            "version is not a string",
            "[package]\nname = \"x\"\nversion = 1\n",
            None,
        ),
        ("no package or workspace", "[dependencies]\n", None),
    ];

    for (case, contents, expected) in cases {
        let manifest = manifest(contents);

        assert_eq!(version_field(&manifest), expected, "case: {case}");
    }
}

#[test]
fn version_field_returns_unsupported_versions_unvalidated() {
    // Validation is left to the caller, so it can report the offending key
    let manifest = manifest("[package]\nname = \"x\"\nversion = \"1.0.0-rc.1\"\n");

    assert_eq!(
        version_field(&manifest),
        Some(("package.version", "1.0.0-rc.1"))
    );
}
