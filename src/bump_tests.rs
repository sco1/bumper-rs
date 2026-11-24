use std::{fs, path::Path};

use anyhow::Result;
use semver::Version;
use tempfile::TempDir;

use super::{apply, group_rules, plan, replace_all};
use crate::config::BumpRule;

const SAMPLE_CARGO: &str = r#"[package]
name = "sco1-bumper"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1.0.104"
"#;

const SAMPLE_README: &str = r#"# bumper
[![crates.io](https://some.url/sco1-bumper/0.1.0?logo=rust)]

Automatically increment the project's version number.

```yaml
repos:
-   repo: https://github.com/sco1/brie-commit
    rev: v0.1.0
    hooks:
    -   id: brie-commit
```
"#;

fn rule(file: &str, search: &str) -> BumpRule {
    BumpRule {
        file: file.to_owned(),
        search: search.to_owned(),
    }
}

fn version(input: &str) -> Version {
    Version::parse(input).expect("test version should be valid SemVer")
}

/// Create a temporary project root containing the provided `(relative path, contents)` files.
fn repo_with(files: &[(&str, &[u8])]) -> TempDir {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    for (name, contents) in files {
        fs::write(temp_dir.path().join(name), contents).expect("could not write test file");
    }

    temp_dir
}

/// Plan & apply the provided rules, returning the reported output.
fn bump(root: &Path, rules: &[BumpRule], current: &str, next: &str) -> Result<String> {
    let changes = plan(root, rules, &version(current), &version(next))?;

    let mut out = Vec::new();
    apply(&changes, &mut out)?;

    Ok(String::from_utf8(out).expect("output should be valid UTF-8"))
}

fn read(root: &Path, name: &str) -> String {
    fs::read_to_string(root.join(name)).expect("could not read test file")
}

#[test]
fn group_rules_merges_by_file_in_first_appearance_order() {
    let rules = [
        rule("./Cargo.toml", r#"version = "{current_version}""#),
        rule("./README.md", "sco1-bumper/{current_version}"),
        rule("./README.md", "rev: v{current_version}"),
    ];

    let expected = vec![
        ("./Cargo.toml", vec![r#"version = "{current_version}""#]),
        (
            "./README.md",
            vec!["sco1-bumper/{current_version}", "rev: v{current_version}"],
        ),
    ];

    assert_eq!(group_rules(&rules), expected);
}

#[test]
fn group_rules_uses_the_exact_configured_spelling() {
    // Different spellings of the same file are distinct groups; plan's duplicate guard catches them
    let rules = [rule("./README.md", "a"), rule("README.md", "b")];

    let groups = group_rules(&rules);

    assert_eq!(groups.len(), 2);
}

#[test]
fn replace_all_substitutes_the_version_template() {
    let cases = [
        (
            "single match",
            "version = \"0.1.0\"\n",
            vec![r#"version = "{current_version}""#],
            "version = \"0.2.0\"\n",
        ),
        (
            "every occurrence",
            "v0.1.0 and v0.1.0",
            vec!["v{current_version}"],
            "v0.2.0 and v0.2.0",
        ),
        (
            "rules apply cumulatively",
            "pkg/0.1.0\nrev: v0.1.0\n",
            vec!["pkg/{current_version}", "rev: v{current_version}"],
            "pkg/0.2.0\nrev: v0.2.0\n",
        ),
        (
            "surrounding text limits the match",
            "version = \"0.1.0\"\nother = \"0.1.0\"\n",
            vec![r#"version = "{current_version}""#],
            "version = \"0.2.0\"\nother = \"0.1.0\"\n",
        ),
        (
            "search without the token changes nothing",
            "version = \"0.1.0\"\n",
            vec!["version"],
            "version = \"0.1.0\"\n",
        ),
        (
            "no match changes nothing",
            "nothing to see here\n",
            vec!["v{current_version}"],
            "nothing to see here\n",
        ),
    ];

    for (case, content, searches, expected) in cases {
        let got = replace_all(content, &searches, "0.1.0", "0.2.0");

        assert_eq!(got, expected, "case: {case}");
    }
}

#[test]
fn plan_does_not_modify_files() {
    let repo = repo_with(&[("Cargo.toml", SAMPLE_CARGO.as_bytes())]);
    let rules = [rule("Cargo.toml", r#"version = "{current_version}""#)];

    let changes = plan(repo.path(), &rules, &version("0.1.0"), &version("0.2.0"))
        .expect("plan should succeed");

    assert_eq!(changes.len(), 1);
    assert!(changes[0].is_changed());
    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
}

#[test]
fn bump_single_replacement() {
    let repo = repo_with(&[("Cargo.toml", SAMPLE_CARGO.as_bytes())]);
    let rules = [rule("./Cargo.toml", r#"version = "{current_version}""#)];

    bump(repo.path(), &rules, "0.1.0", "0.2.0").expect("bump should succeed");

    let expected = SAMPLE_CARGO.replace(r#"version = "0.1.0""#, r#"version = "0.2.0""#);
    assert_eq!(read(repo.path(), "Cargo.toml"), expected);
}

#[test]
fn bump_multiple_replacements_across_files() {
    let repo = repo_with(&[
        ("Cargo.toml", SAMPLE_CARGO.as_bytes()),
        ("README.md", SAMPLE_README.as_bytes()),
    ]);
    let rules = [
        rule("./Cargo.toml", r#"version = "{current_version}""#),
        rule("./README.md", "sco1-bumper/{current_version}"),
        rule("./README.md", "rev: v{current_version}"),
    ];

    bump(repo.path(), &rules, "0.1.0", "0.2.0").expect("bump should succeed");

    let expected_cargo = SAMPLE_CARGO.replace(r#"version = "0.1.0""#, r#"version = "0.2.0""#);
    let expected_readme = SAMPLE_README
        .replace("sco1-bumper/0.1.0", "sco1-bumper/0.2.0")
        .replace("rev: v0.1.0", "rev: v0.2.0");

    assert_eq!(read(repo.path(), "Cargo.toml"), expected_cargo);
    assert_eq!(read(repo.path(), "README.md"), expected_readme);
}

#[test]
fn apply_reports_bumped_and_unchanged_files_in_order() {
    let repo = repo_with(&[
        ("Cargo.toml", SAMPLE_CARGO.as_bytes()),
        ("README.md", SAMPLE_README.as_bytes()),
    ]);
    let rules = [
        rule("./Cargo.toml", r#"version = "{current_version}""#),
        rule("./README.md", "no-such-text/{current_version}"),
    ];

    let output = bump(repo.path(), &rules, "0.1.0", "0.2.0").expect("bump should succeed");

    assert_eq!(output, "Bumped ./Cargo.toml\n./README.md - No changes.\n");
    assert_eq!(read(repo.path(), "README.md"), SAMPLE_README);
}

#[test]
fn apply_reports_no_changes_when_the_version_is_absent() {
    let repo = repo_with(&[
        ("Cargo.toml", SAMPLE_CARGO.as_bytes()),
        ("README.md", SAMPLE_README.as_bytes()),
    ]);
    let rules = [
        rule("Cargo.toml", r#"version = "{current_version}""#),
        rule("README.md", "sco1-bumper/{current_version}"),
    ];

    let output = bump(repo.path(), &rules, "100.200.300", "101.0.0").expect("bump should succeed");

    assert_eq!(
        output,
        "Cargo.toml - No changes.\nREADME.md - No changes.\n"
    );
    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
    assert_eq!(read(repo.path(), "README.md"), SAMPLE_README);
}

#[test]
fn bump_preserves_line_endings() {
    let cases: [(&str, &[u8]); 4] = [
        ("lf", b"[package]\nversion = \"0.1.0\"\n"),
        ("crlf", b"[package]\r\nversion = \"0.1.0\"\r\n"),
        ("mixed", b"[package]\r\nversion = \"0.1.0\"\n# trailing\r\n"),
        ("no trailing newline", b"[package]\nversion = \"0.1.0\""),
    ];

    for (case, src) in cases {
        let repo = repo_with(&[("Cargo.toml", src)]);
        let rules = [rule("Cargo.toml", r#"version = "{current_version}""#)];

        bump(repo.path(), &rules, "0.1.0", "0.1.1").expect("bump should succeed");

        let expected = String::from_utf8(src.to_vec())
            .expect("test input should be UTF-8")
            .replace(r#"version = "0.1.0""#, r#"version = "0.1.1""#)
            .into_bytes();
        let got = fs::read(repo.path().join("Cargo.toml")).expect("could not read test file");

        assert_eq!(got, expected, "case: {case}");
    }
}

#[test]
fn bump_round_trips_non_ascii_content() {
    let src = "[package]\nversion = \"0.1.0\"\nauthors = [\"Ágnes Café 🐍\"]\n";
    let repo = repo_with(&[("Cargo.toml", src.as_bytes())]);
    let rules = [rule("Cargo.toml", r#"version = "{current_version}""#)];

    bump(repo.path(), &rules, "0.1.0", "0.1.1").expect("bump should succeed");

    let expected = src.replace(r#"version = "0.1.0""#, r#"version = "0.1.1""#);
    assert_eq!(read(repo.path(), "Cargo.toml"), expected);
}

#[test]
fn bump_matches_a_non_ascii_search() {
    let repo = repo_with(&[("README.md", "Café release: v0.1.0\n".as_bytes())]);
    let rules = [rule("README.md", "Café release: v{current_version}")];

    bump(repo.path(), &rules, "0.1.0", "0.1.1").expect("bump should succeed");

    assert_eq!(read(repo.path(), "README.md"), "Café release: v0.1.1\n");
}

#[test]
fn plan_rejects_two_spellings_of_the_same_file() {
    let repo = repo_with(&[("README.md", SAMPLE_README.as_bytes())]);
    let rules = [
        rule("./README.md", "sco1-bumper/{current_version}"),
        rule("README.md", "rev: v{current_version}"),
    ];

    let err = bump(repo.path(), &rules, "0.1.0", "0.2.0").expect_err("duplicate should fail");
    let message = format!("{err:#}");

    assert!(
        message.contains("'./README.md' and 'README.md' refer to the same file"),
        "error: {message}"
    );
    assert_eq!(read(repo.path(), "README.md"), SAMPLE_README);
}

#[test]
fn plan_rejects_a_missing_target_before_writing_anything() {
    // The valid target comes first, so a write-as-you-go implementation would have modified it
    let repo = repo_with(&[("Cargo.toml", SAMPLE_CARGO.as_bytes())]);
    let rules = [
        rule("Cargo.toml", r#"version = "{current_version}""#),
        rule("missing.md", "v{current_version}"),
    ];

    let err = bump(repo.path(), &rules, "0.1.0", "0.2.0").expect_err("missing file should fail");
    let message = format!("{err:#}");

    assert!(
        message.contains("Could not locate bump target 'missing.md'"),
        "error: {message}"
    );
    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
}
