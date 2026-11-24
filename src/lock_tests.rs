use std::{fs, path::Path};

use anyhow::Result;
use tempfile::TempDir;

use super::check;

const MATCHING_MANIFEST: &str = r#"
[package]
name = "example"
version = "0.1.0"
edition = "2024"
"#;

const MATCHING_LOCK: &str = r#"
version = 4

[[package]]
name = "example"
version = "0.1.0"
"#;

fn repo_with(files: &[(&str, &str)]) -> TempDir {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    for (name, contents) in files {
        fs::write(temp_dir.path().join(name), contents)
            .expect("could not write test file");
    }

    temp_dir
}

fn run_check(root: &Path) -> Result<String> {
    let mut out = Vec::new();

    check(root, &mut out)?;

    Ok(String::from_utf8(out).expect("output should be valid UTF-8"))
}

#[test]
fn check_is_silent_when_manifest_and_lock_versions_match() {
    let repo = repo_with(&[
        ("Cargo.toml", MATCHING_MANIFEST),
        ("Cargo.lock", MATCHING_LOCK),
    ]);

    let output = run_check(repo.path()).expect("check should succeed");

    assert_eq!(output, "");
}

#[test]
fn check_reports_a_mismatched_locked_version() {
    let repo = repo_with(&[
        (
            "Cargo.toml",
            r#"
[package]
name = "example"
version = "0.2.0"
edition = "2024"
"#,
        ),
        ("Cargo.lock", MATCHING_LOCK),
    ]);

    let output = run_check(repo.path()).expect("check should succeed");

    assert_eq!(
        output,
        concat!(
            "NOTE: Cargo.lock version for package 'example' is '0.1.0', ",
            "but Cargo.toml has '0.2.0'; ",
            "run 'cargo update --workspace' to refresh Cargo.lock.\n",
        ),
    );
}

#[test]
fn check_reports_when_the_lockfile_has_no_exact_name_match() {
    let repo = repo_with(&[
        (
            "Cargo.toml",
            r#"
[package]
name = "my-package"
version = "0.1.0"
edition = "2024"
"#,
        ),
        (
            "Cargo.lock",
            r#"
version = 4

[[package]]
name = "my_package"
version = "0.1.0"
"#,
        ),
    ]);

    let output = run_check(repo.path()).expect("check should succeed");

    assert_eq!(
        output,
        concat!(
            "NOTE: Cargo.lock has no package entry named 'my-package'; ",
            "run 'cargo update --workspace' to refresh Cargo.lock.\n",
        ),
    );
}

#[test]
fn check_is_silent_when_cargo_toml_is_missing() {
    let repo = repo_with(&[("Cargo.lock", MATCHING_LOCK)]);

    let output = run_check(repo.path()).expect("check should succeed");

    assert_eq!(output, "");
}

#[test]
fn check_is_silent_when_cargo_lock_is_missing() {
    let repo = repo_with(&[("Cargo.toml", MATCHING_MANIFEST)]);

    let output = run_check(repo.path()).expect("check should succeed");

    assert_eq!(output, "");
}

#[test]
fn check_is_silent_for_a_virtual_workspace_manifest() {
    let repo = repo_with(&[
        (
            "Cargo.toml",
            r#"
[workspace]
members = ["member"]
resolver = "3"
"#,
        ),
        ("Cargo.lock", MATCHING_LOCK),
    ]);

    let output = run_check(repo.path()).expect("check should succeed");

    assert_eq!(output, "");
}

#[test]
fn check_is_silent_when_the_version_is_workspace_inherited() {
    let repo = repo_with(&[
        (
            "Cargo.toml",
            r#"
[workspace]
resolver = "3"

[workspace.package]
version = "0.2.0"

[package]
name = "example"
version.workspace = true
edition = "2024"
"#,
        ),
        ("Cargo.lock", MATCHING_LOCK),
    ]);

    let output = run_check(repo.path()).expect("check should succeed");

    assert_eq!(output, "");
}

#[test]
fn check_does_not_modify_either_file() {
    let manifest = r#"
[package]
name = "example"
version = "0.2.0"
edition = "2024"
"#;
    let lock = MATCHING_LOCK;

    let repo = repo_with(&[
        ("Cargo.toml", manifest),
        ("Cargo.lock", lock),
    ]);

    run_check(repo.path()).expect("check should succeed");

    assert_eq!(
        fs::read_to_string(repo.path().join("Cargo.toml"))
            .expect("could not read Cargo.toml"),
        manifest,
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("Cargo.lock"))
            .expect("could not read Cargo.lock"),
        lock,
    );
}
