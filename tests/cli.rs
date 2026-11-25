use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn repo_with(files: &[(&str, &str)]) -> TempDir {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    for (name, contents) in files {
        fs::write(temp_dir.path().join(name), contents).expect("could not write test file");
    }

    temp_dir
}

fn bumper_in(root: &TempDir) -> Command {
    let mut command = Command::cargo_bin("bumper").expect("could not locate bumper binary");

    command.current_dir(root.path());
    command
}

const CARGO_0_1_0: &str = r#"[package]
name = "example"
version = "0.1.0"
edition = "2024"
"#;

const LOCK_0_1_0: &str = r#"version = 4

[[package]]
name = "example"
version = "0.1.0"
"#;

const SEMVER_CONFIG: &str = r#"[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"

[[tool.bumper.files]]
file = "./README.md"
search = "release v{current_version}"
"#;

fn read(root: &TempDir, name: &str) -> String {
    fs::read_to_string(root.path().join(name)).expect("could not read test file")
}

#[test]
fn bump_fails_when_configuration_is_missing() {
    let repo = TempDir::new().expect("could not create temporary directory");

    bumper_in(&repo)
        .args(["bump", "patch"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Configuration file '.bumper.toml' could not be located.",
        ));
}

#[test]
fn bump_rejects_an_invalid_versioning_and_bump_combination() {
    let repo = repo_with(&[(
        ".bumper.toml",
        r#"[tool.bumper]
current_version = "2026.11.0"
versioning_type = "calver"
files = []
"#,
    )]);

    bumper_in(&repo)
        .args(["bump", "patch"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Calver projects must bump by date.",
        ));
}

#[test]
fn bump_updates_targets_and_the_configuration_file() {
    let repo = repo_with(&[
        (".bumper.toml", SEMVER_CONFIG),
        ("Cargo.toml", CARGO_0_1_0),
        ("README.md", "# Example\n\nCurrent release v0.1.0\n"),
        ("Cargo.lock", LOCK_0_1_0),
    ]);

    bumper_in(&repo)
        .args(["bump", "minor", "--no-check-lock"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Bumper bumping: v0.1.0 -> v0.2.0\n")
                .and(predicate::str::contains("Bumped ./Cargo.toml\n"))
                .and(predicate::str::contains("Bumped ./README.md\n"))
                .and(predicate::str::contains("Bumped .bumper.toml\n")),
        );

    assert!(
        read(&repo, "Cargo.toml").contains(r#"version = "0.2.0""#),
        "Cargo.toml was not updated"
    );
    assert!(
        read(&repo, "README.md").contains("release v0.2.0"),
        "README.md was not updated"
    );
    assert!(
        read(&repo, ".bumper.toml").contains(r#"current_version = "0.2.0""#),
        ".bumper.toml was not updated"
    );
}

#[test]
fn dry_run_prints_diffs_and_does_not_modify_files() {
    let readme = "# Example\n\nCurrent release v0.1.0\n";

    let repo = repo_with(&[
        (".bumper.toml", SEMVER_CONFIG),
        ("Cargo.toml", CARGO_0_1_0),
        ("README.md", readme),
        // Deliberately stale: dry-run must not invoke the lock check.
        ("Cargo.lock", LOCK_0_1_0),
    ]);

    bumper_in(&repo)
        .args(["bump", "patch", "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Bumper bumping: v0.1.0 -> v0.1.1\n")
                .and(predicate::str::contains("--- ./Cargo.toml\n"))
                .and(predicate::str::contains("+++ ./Cargo.toml\n"))
                .and(predicate::str::contains("--- ./README.md\n"))
                .and(predicate::str::contains("+++ ./README.md\n"))
                .and(predicate::str::contains("--- .bumper.toml\n"))
                .and(predicate::str::contains("+++ .bumper.toml\n"))
                .and(predicate::str::contains("Bumped").not())
                .and(predicate::str::contains("NOTE: Cargo.lock").not()),
        );

    assert_eq!(read(&repo, "Cargo.toml"), CARGO_0_1_0);
    assert_eq!(read(&repo, "README.md"), readme);
    assert_eq!(read(&repo, ".bumper.toml"), SEMVER_CONFIG);
    assert_eq!(read(&repo, "Cargo.lock"), LOCK_0_1_0);
}

#[test]
fn bump_reports_a_stale_cargo_lock_by_default() {
    let repo = repo_with(&[
        (
            ".bumper.toml",
            r#"[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"
"#,
        ),
        ("Cargo.toml", CARGO_0_1_0),
        ("Cargo.lock", LOCK_0_1_0),
    ]);

    bumper_in(&repo)
        .args(["bump", "patch"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "NOTE: Cargo.lock version for package 'example' is '0.1.0', \
             but Cargo.toml has '0.1.1'; \
             run 'cargo update --workspace' to refresh Cargo.lock.",
        ));

    assert!(
        read(&repo, "Cargo.toml").contains(r#"version = "0.1.1""#),
        "Cargo.toml should be bumped before lockfile validation"
    );
    assert!(
        read(&repo, ".bumper.toml").contains(r#"current_version = "0.1.1""#),
        ".bumper.toml should be bumped before lockfile validation"
    );
    assert_eq!(
        read(&repo, "Cargo.lock"),
        LOCK_0_1_0,
        "lock checking is advisory and must not modify Cargo.lock"
    );
}

#[test]
fn no_check_lock_suppresses_the_stale_lock_advisory() {
    let repo = repo_with(&[
        (
            ".bumper.toml",
            r#"[tool.bumper]
current_version = "0.1.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"
"#,
        ),
        ("Cargo.toml", CARGO_0_1_0),
        ("Cargo.lock", LOCK_0_1_0),
    ]);

    bumper_in(&repo)
        .args(["bump", "patch", "--no-check-lock"])
        .assert()
        .success()
        .stdout(predicate::str::contains("NOTE: Cargo.lock").not());

    assert!(
        read(&repo, "Cargo.toml").contains(r#"version = "0.1.1""#),
        "Cargo.toml should still be bumped"
    );
    assert_eq!(read(&repo, "Cargo.lock"), LOCK_0_1_0);
}

/// Configuration whose `Cargo.toml` key rule will not match [`CARGO_0_1_0`].
const OUT_OF_SYNC_CONFIG: &str = r#"[tool.bumper]
current_version = "0.0.9"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"
"#;

const OUT_OF_SYNC_WARNING: &str =
    "./Cargo.toml: key 'package.version' has value '0.1.0', not the current version";

#[test]
fn bump_warns_about_unmatched_rules_and_still_bumps() {
    let repo = repo_with(&[
        (".bumper.toml", OUT_OF_SYNC_CONFIG),
        ("Cargo.toml", CARGO_0_1_0),
    ]);

    bumper_in(&repo)
        .args(["bump", "patch", "--no-check-lock"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains(format!("WARNING: {OUT_OF_SYNC_WARNING}\n"))
                .and(predicate::str::contains("./Cargo.toml - No changes.\n"))
                .and(predicate::str::contains("Bumped .bumper.toml\n")),
        );

    assert_eq!(read(&repo, "Cargo.toml"), CARGO_0_1_0);
    assert!(
        read(&repo, ".bumper.toml").contains(r#"current_version = "0.0.10""#),
        ".bumper.toml should still be bumped without --strict"
    );
}

#[test]
fn dry_run_warns_about_unmatched_rules() {
    let repo = repo_with(&[
        (".bumper.toml", OUT_OF_SYNC_CONFIG),
        ("Cargo.toml", CARGO_0_1_0),
    ]);

    bumper_in(&repo)
        .args(["bump", "patch", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "WARNING: {OUT_OF_SYNC_WARNING}\n"
        )));

    assert_eq!(read(&repo, ".bumper.toml"), OUT_OF_SYNC_CONFIG);
}

#[test]
fn strict_fails_on_unmatched_rules_without_writing_anything() {
    for args in [
        &["bump", "patch", "--strict"][..],
        &["bump", "patch", "--strict", "--dry-run"][..],
    ] {
        let repo = repo_with(&[
            (".bumper.toml", OUT_OF_SYNC_CONFIG),
            ("Cargo.toml", CARGO_0_1_0),
        ]);

        bumper_in(&repo)
            .args(args)
            .assert()
            .failure()
            .stdout(predicate::str::is_empty())
            .stderr(
                predicate::str::contains("1 rule(s) matched nothing; no files were changed:")
                    .and(predicate::str::contains(format!("  {OUT_OF_SYNC_WARNING}"))),
            );

        assert_eq!(read(&repo, "Cargo.toml"), CARGO_0_1_0, "args: {args:?}");
        assert_eq!(
            read(&repo, ".bumper.toml"),
            OUT_OF_SYNC_CONFIG,
            "args: {args:?}"
        );
    }
}

#[test]
fn strict_bumps_when_every_rule_matches() {
    let repo = repo_with(&[
        (".bumper.toml", SEMVER_CONFIG),
        ("Cargo.toml", CARGO_0_1_0),
        ("README.md", "# Example\n\nCurrent release v0.1.0\n"),
    ]);

    bumper_in(&repo)
        .args(["bump", "patch", "--strict", "--no-check-lock"])
        .assert()
        .success()
        .stdout(predicate::str::contains("WARNING").not());

    assert!(
        read(&repo, "Cargo.toml").contains(r#"version = "0.1.1""#),
        "Cargo.toml was not updated"
    );
}

#[test]
fn init_creates_a_usable_configuration_and_bump_can_use_it() {
    let repo = repo_with(&[("Cargo.toml", CARGO_0_1_0)]);

    bumper_in(&repo).args(["init"]).assert().success();

    let generated = read(&repo, ".bumper.toml");

    assert!(generated.contains(r#"current_version = "0.1.0""#));
    assert!(generated.contains(r#"versioning_type = "semver""#));
    assert!(generated.contains(r#"file = "./Cargo.toml""#));
    assert!(generated.contains(r#"key = "package.version""#));

    bumper_in(&repo)
        .args(["bump", "patch", "--no-check-lock"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Bumper bumping: v0.1.0 -> v0.1.1"));

    assert!(
        read(&repo, "Cargo.toml").contains(r#"version = "0.1.1""#),
        "generated configuration did not update Cargo.toml"
    );
    assert!(
        read(&repo, ".bumper.toml").contains(r#"current_version = "0.1.1""#),
        "generated configuration did not update itself"
    );
}
