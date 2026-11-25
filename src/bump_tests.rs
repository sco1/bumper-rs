use std::{fs, path::Path};

use anyhow::Result;
use semver::Version;
use tempfile::TempDir;

use super::{apply, apply_rules, group_rules, plan, preview, replace_key, replace_search};
use crate::config::{BumpRule, RuleKind, SearchPattern};

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

/// Plan and preview the provided rules, returning the reported output.
fn preview_bump(root: &Path, rules: &[BumpRule], current: &str, next: &str) -> Result<String> {
    let changes = plan(root, rules, &version(current), &version(next))?;

    let mut out = Vec::new();
    preview(&changes, &mut out)?;

    Ok(String::from_utf8(out).expect("output should be valid UTF-8"))
}

#[test]
fn group_rules_merges_by_file_in_first_appearance_order() {
    let rules = [
        search_rule("./Cargo.toml", r#"version = "{current_version}""#),
        search_rule("./README.md", "sco1-bumper/{current_version}"),
        search_rule("./README.md", "rev: v{current_version}"),
    ];

    let expected = vec![
        ("./Cargo.toml", vec![&rules[0].kind]),
        ("./README.md", vec![&rules[1].kind, &rules[2].kind]),
    ];

    assert_eq!(group_rules(&rules), expected);
}

#[test]
fn group_rules_uses_the_exact_configured_spelling() {
    // Different spellings of the same file are distinct groups; plan's duplicate guard catches them
    let rules = [
        search_rule("./README.md", "a{current_version}"),
        search_rule("README.md", "b{current_version}"),
    ];

    let groups = group_rules(&rules);

    assert_eq!(groups.len(), 2);
}

#[test]
fn replace_search_substitutes_the_version_template() {
    let cases = [
        (
            "single match",
            "version = \"0.1.0\"\n",
            r#"version = "{current_version}""#,
            "version = \"0.2.0\"\n",
        ),
        (
            "every occurrence",
            "v0.1.0 and v0.1.0",
            "v{current_version}",
            "v0.2.0 and v0.2.0",
        ),
        (
            "surrounding text limits the match",
            "version = \"0.1.0\"\nother = \"0.1.0\"\n",
            r#"version = "{current_version}""#,
            "version = \"0.2.0\"\nother = \"0.1.0\"\n",
        ),
        (
            "no match changes nothing",
            "nothing to see here\n",
            "v{current_version}",
            "nothing to see here\n",
        ),
    ];

    for (case, content, search, expected) in cases {
        let got = replace_search(content, &pattern(search), "0.1.0", "0.2.0");

        assert_eq!(got, expected, "case: {case}");
    }
}

#[test]
fn replace_key_updates_only_the_targeted_value() {
    let cases = [
        (
            "package version",
            SAMPLE_CARGO,
            "package.version",
            SAMPLE_CARGO.replace(r#"version = "0.1.0""#, r#"version = "0.2.0""#),
        ),
        (
            "workspace package version",
            "[workspace.package]\nversion = \"0.1.0\"\n",
            "workspace.package.version",
            "[workspace.package]\nversion = \"0.2.0\"\n".to_owned(),
        ),
        (
            "dependency with the same version is untouched",
            "[package]\nversion = \"0.1.0\"\n\n[dependencies]\nhelper = { path = \"../helper\", version = \"0.1.0\" }\n",
            "package.version",
            "[package]\nversion = \"0.2.0\"\n\n[dependencies]\nhelper = { path = \"../helper\", version = \"0.1.0\" }\n"
                .to_owned(),
        ),
        (
            "single quotes, spacing and comments are kept",
            "[package]\nversion   =   '0.1.0'  # keep me\n",
            "package.version",
            "[package]\nversion   =   '0.2.0'  # keep me\n".to_owned(),
        ),
        (
            "inline table",
            "package = { name = 'x', version = '0.1.0' }\n",
            "package.version",
            "package = { name = 'x', version = '0.2.0' }\n".to_owned(),
        ),
        (
            "dotted key",
            "package.version = \"0.1.0\"\n",
            "package.version",
            "package.version = \"0.2.0\"\n".to_owned(),
        ),
        (
            "crlf and mixed line endings are kept",
            "# top\r\n[package]\r\nversion = \"0.1.0\"\n\r\n[tool]\r\n",
            "package.version",
            "# top\r\n[package]\r\nversion = \"0.2.0\"\n\r\n[tool]\r\n".to_owned(),
        ),
    ];

    for (case, content, key, expected) in cases {
        let got =
            replace_key(content, key, "0.1.0", "0.2.0").expect("key replacement should succeed");

        assert_eq!(got, expected, "case: {case}");
    }
}

#[test]
fn replace_key_leaves_content_unchanged_when_nothing_matches() {
    let content = "[package]\nversion = \"0.1.0\"\n";
    let cases = [
        ("missing key", "package.nope", "0.1.0"),
        ("missing table", "workspace.package.version", "0.1.0"),
        (
            "path continues past a value",
            "package.version.extra",
            "0.1.0",
        ),
        (
            "value differs from the current version",
            "package.version",
            "0.3.0",
        ),
    ];

    for (case, key, current) in cases {
        let got = replace_key(content, key, current, "1.0.0").expect("should not error");

        assert_eq!(got, content, "case: {case}");
    }
}

#[test]
fn replace_key_rejects_unusable_targets() {
    let cases = [
        (
            "key is a table",
            "[package]\nversion = \"0.1.0\"\n",
            "package",
            "Key 'package' is not a value",
        ),
        (
            "value is not a string",
            "[package]\nversion = 1\n",
            "package.version",
            "Key 'package.version' is not a string",
        ),
        (
            "invalid TOML",
            "[package\n",
            "package.version",
            "Could not parse file as TOML",
        ),
    ];

    for (case, content, key, expected) in cases {
        let err = replace_key(content, key, "0.1.0", "0.2.0").expect_err("should be rejected");
        let message = format!("{err:#}");

        assert!(message.contains(expected), "case: {case}; error: {message}");
    }
}

#[test]
fn apply_rules_applies_mixed_rules_cumulatively_in_order() {
    let content = "[package]\nversion = \"0.1.0\"\n# pkg/0.1.0\n";
    let key = RuleKind::Key("package.version".to_owned());
    let search = RuleKind::Search(pattern("pkg/{current_version}"));

    let got = apply_rules(content, &[&key, &search], "0.1.0", "0.2.0").expect("rules should apply");

    assert_eq!(got, "[package]\nversion = \"0.2.0\"\n# pkg/0.2.0\n");
}

#[test]
fn apply_rules_stops_at_the_first_error() {
    let content = "[package]\nversion = 1\n";
    let key = RuleKind::Key("package.version".to_owned());

    let err = apply_rules(content, &[&key], "0.1.0", "0.2.0").expect_err("should be rejected");

    assert!(
        format!("{err:#}").contains("is not a string"),
        "error: {err:#}"
    );
}

#[test]
fn plan_does_not_modify_files() {
    let repo = repo_with(&[("Cargo.toml", SAMPLE_CARGO.as_bytes())]);
    let rules = [search_rule(
        "Cargo.toml",
        r#"version = "{current_version}""#,
    )];

    let changes = plan(repo.path(), &rules, &version("0.1.0"), &version("0.2.0"))
        .expect("plan should succeed");

    assert_eq!(changes.len(), 1);
    assert!(changes[0].is_changed());
    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
}

#[test]
fn bump_single_replacement() {
    let repo = repo_with(&[("Cargo.toml", SAMPLE_CARGO.as_bytes())]);
    let rules = [search_rule(
        "./Cargo.toml",
        r#"version = "{current_version}""#,
    )];

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
        search_rule("./Cargo.toml", r#"version = "{current_version}""#),
        search_rule("./README.md", "sco1-bumper/{current_version}"),
        search_rule("./README.md", "rev: v{current_version}"),
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
        search_rule("./Cargo.toml", r#"version = "{current_version}""#),
        search_rule("./README.md", "no-such-text/{current_version}"),
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
        search_rule("Cargo.toml", r#"version = "{current_version}""#),
        search_rule("README.md", "sco1-bumper/{current_version}"),
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
        let rules = [search_rule(
            "Cargo.toml",
            r#"version = "{current_version}""#,
        )];

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
    let rules = [search_rule(
        "Cargo.toml",
        r#"version = "{current_version}""#,
    )];

    bump(repo.path(), &rules, "0.1.0", "0.1.1").expect("bump should succeed");

    let expected = src.replace(r#"version = "0.1.0""#, r#"version = "0.1.1""#);
    assert_eq!(read(repo.path(), "Cargo.toml"), expected);
}

#[test]
fn bump_matches_a_non_ascii_search() {
    let repo = repo_with(&[("README.md", "Café release: v0.1.0\n".as_bytes())]);
    let rules = [search_rule("README.md", "Café release: v{current_version}")];

    bump(repo.path(), &rules, "0.1.0", "0.1.1").expect("bump should succeed");

    assert_eq!(read(repo.path(), "README.md"), "Café release: v0.1.1\n");
}

#[test]
fn plan_rejects_two_spellings_of_the_same_file() {
    let repo = repo_with(&[("README.md", SAMPLE_README.as_bytes())]);
    let rules = [
        search_rule("./README.md", "sco1-bumper/{current_version}"),
        search_rule("README.md", "rev: v{current_version}"),
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
        search_rule("Cargo.toml", r#"version = "{current_version}""#),
        search_rule("missing.md", "v{current_version}"),
    ];

    let err = bump(repo.path(), &rules, "0.1.0", "0.2.0").expect_err("missing file should fail");
    let message = format!("{err:#}");

    assert!(
        message.contains("Could not locate bump target 'missing.md'"),
        "error: {message}"
    );
    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
}

#[test]
fn bump_with_a_key_rule_leaves_matching_dependencies_alone() {
    let src = "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nhelper = { path = \"../helper\", version = \"0.1.0\" }\n";
    let repo = repo_with(&[("Cargo.toml", src.as_bytes())]);
    let rules = [key_rule("./Cargo.toml", "package.version")];

    let output = bump(repo.path(), &rules, "0.1.0", "0.2.0").expect("bump should succeed");

    assert_eq!(output, "Bumped ./Cargo.toml\n");
    assert_eq!(
        read(repo.path(), "Cargo.toml"),
        src.replacen("version = \"0.1.0\"", "version = \"0.2.0\"", 1)
    );
}

#[test]
fn bump_reports_no_changes_for_a_missing_or_mismatched_key() {
    let repo = repo_with(&[
        ("Cargo.toml", SAMPLE_CARGO.as_bytes()),
        ("other.toml", b"[package]\nversion = \"9.9.9\"\n"),
    ]);
    let rules = [
        key_rule("Cargo.toml", "package.missing"),
        key_rule("other.toml", "package.version"),
    ];

    let output = bump(repo.path(), &rules, "0.1.0", "0.2.0").expect("bump should succeed");

    assert_eq!(
        output,
        "Cargo.toml - No changes.\nother.toml - No changes.\n"
    );
    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
}

#[test]
fn plan_names_the_file_when_a_key_rule_fails_and_writes_nothing() {
    let repo = repo_with(&[
        ("Cargo.toml", SAMPLE_CARGO.as_bytes()),
        ("bad.toml", b"[package]\nversion = 1\n"),
    ]);
    let rules = [
        key_rule("Cargo.toml", "package.version"),
        key_rule("bad.toml", "package.version"),
    ];

    let err = bump(repo.path(), &rules, "0.1.0", "0.2.0").expect_err("bad key should fail");
    let message = format!("{err:#}");

    assert!(
        message.contains("Could not bump 'bad.toml'"),
        "error: {message}"
    );
    assert!(message.contains("is not a string"), "error: {message}");
    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
}

#[test]
fn preview_renders_diffs_and_does_not_modify_files() {
    let repo = repo_with(&[
        ("Cargo.toml", SAMPLE_CARGO.as_bytes()),
        ("README.md", SAMPLE_README.as_bytes()),
    ]);
    let rules = [
        search_rule("./Cargo.toml", r#"version = "{current_version}""#),
        search_rule("./README.md", "does-not-appear/{current_version}"),
    ];

    let output =
        preview_bump(repo.path(), &rules, "0.1.0", "0.2.0").expect("preview should succeed");

    assert_eq!(
        output,
        concat!(
            "--- ./Cargo.toml\n",
            "+++ ./Cargo.toml\n",
            "@@ -3 +3 @@\n",
            "-version = \"0.1.0\"\n",
            "+version = \"0.2.0\"\n",
            "\n",
            "./README.md - No changes.\n",
        ),
    );

    assert_eq!(read(repo.path(), "Cargo.toml"), SAMPLE_CARGO);
    assert_eq!(read(repo.path(), "README.md"), SAMPLE_README);
}
