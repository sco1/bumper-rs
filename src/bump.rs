//! Planning, applying, and previewing version replacements.
//!
//! To keep a failure of any individual rule from leaving the project partially bumped, bumping is
//! split into two phases:
//!     1. [`plan`] reads every target file and computes its updated contents in memory
//!     2. [`apply`] writes the results (or [`preview`] renders them as a diff during a dry run)
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use semver::Version;
use toml_edit::Document;

use crate::{
    config::{BUMPER_CONFIG_FILE, BumpRule, RuleKind},
    diff,
};

/// Placeholder for [`RuleKind::Search`] patterns that is substituted with a version string.
const VERSION_TOKEN: &str = "{current_version}";

/// The planned before/after contents of a single target file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Resolved path used for writing (`root` joined with `file`).
    path: PathBuf,
    /// File path as spelled in the configuration, used in user-facing output.
    file: String,
    /// File contents as read from disk.
    original: String,
    /// File contents after all of the file's rules have been applied.
    updated: String,
}

impl FileChange {
    /// Return `true` if applying the rules altered the file contents.
    fn is_changed(&self) -> bool {
        self.original != self.updated
    }
}

/// Group rules by target file, preserving first-seen file order and per-file rule order.
///
/// Files are grouped by their configured spelling; [`plan`] separately rejects distinct spellings
/// that resolve to the same file.
fn group_rules(rules: &[BumpRule]) -> Vec<(&str, Vec<&RuleKind>)> {
    // For this application we'll generally only have a handful of replacement rules, so searching
    // through Vec should be sufficient vs. using HashMap or indexmap
    let mut groups: Vec<(&str, Vec<&RuleKind>)> = Vec::new();
    for rule in rules {
        match groups.iter_mut().find(|(file, _)| *file == rule.file) {
            Some((_, kinds)) => kinds.push(&rule.kind),
            None => groups.push((rule.file.as_str(), vec![&rule.kind])),
        }
    }

    groups
}

/// Replace every occurrence of `search` (with [`VERSION_TOKEN`] set to `current`) by `search`
/// with the token set to `next`.
///
/// A pattern without [`VERSION_TOKEN`] matches its own replacement, so the content is unchanged.
fn replace_search(content: &str, search: &str, current: &str, next: &str) -> String {
    content.replace(
        &search.replace(VERSION_TOKEN, current),
        &search.replace(VERSION_TOKEN, next),
    )
}

/// Replace the string value at the dotted TOML `key` if it equals `current`.
///
/// The edit is made by splicing the value's source span, so all other formatting, comments, and
/// quoting in the file are preserved. A missing key, or a value that does not equal `current`,
/// leaves the content unchanged.
///
/// # Errors
///
/// Returns an error if `content` is not valid TOML, or if `key` resolves to a table or a non-string
/// value.
fn replace_key(content: &str, key: &str, current: &str, next: &str) -> Result<String> {
    let doc = Document::parse(content).context("Could not parse file as TOML")?;

    let mut item = doc.as_item();
    for part in key.split('.') {
        match item.get(part) {
            Some(child) if !child.is_none() => item = child,
            _ => return Ok(content.to_owned()), // missing key: report as no change
        }
    }

    let value = item
        .as_value()
        .with_context(|| format!("Key '{key}' is not a value"))?;
    let found = value
        .as_str()
        .with_context(|| format!("Key '{key}' is not a string"))?;
    if found != current {
        return Ok(content.to_owned());
    }

    let span = value
        .span()
        .with_context(|| format!("Key '{key}' has no source location"))?;
    let replaced = content[span.clone()].replacen(current, next, 1);
    Ok(format!(
        "{}{}{}",
        &content[..span.start],
        replaced,
        &content[span.end..]
    ))
}

/// Apply each rule to `content` in order, feeding each rule's output into the next.
fn apply_rules(content: &str, kinds: &[&RuleKind], current: &str, next: &str) -> Result<String> {
    kinds
        .iter()
        .try_fold(content.to_owned(), |running, kind| match kind {
            RuleKind::Search(search) => Ok(replace_search(&running, search, current, next)),
            RuleKind::Key(key) => replace_key(&running, key, current, next),
        })
}

/// Compute the [`FileChange`] for every file targeted by `rules`; no writing is done by this step.
///
/// Rule file paths are resolved relative to `root`. One `FileChange` is returned per distinct file,
/// in the order each file first appears in `rules`.
///
/// # Errors
///
/// Returns an error if a target file does not exist or cannot be read, if two configured paths
/// resolve to the same file, if a `key` rule targets a file that is not valid TOML, or a key has a
/// value that is not a string.
pub fn plan(
    root: &Path,
    rules: &[BumpRule],
    current: &Version,
    next: &Version,
) -> Result<Vec<FileChange>> {
    let mut seen: HashMap<PathBuf, &str> = HashMap::new();
    let mut changes: Vec<FileChange> = Vec::new();
    let (current, next) = (current.to_string(), next.to_string());

    for (file, kinds) in group_rules(rules) {
        let path = root.join(file);
        let canonical = fs::canonicalize(&path)
            .with_context(|| format!("Could not locate bump target '{file}'"))?;

        // Bail & notify user if two configured spellings resolve to the same file
        if let Some(previous) = seen.insert(canonical, file) {
            bail!(
                "'{previous}' and '{file}' refer to the same file; \
                use a single spelling in {BUMPER_CONFIG_FILE}"
            );
        }

        let original = fs::read_to_string(&path)
            .with_context(|| format!("Could not read bump target '{file}'"))?;
        let updated = apply_rules(&original, &kinds, &current, &next)
            .with_context(|| format!("Could not bump '{file}'"))?;
        changes.push(FileChange {
            path,
            file: file.to_owned(),
            original,
            updated,
        });
    }

    Ok(changes)
}

/// Write each changed file to disk, reporting the outcome for every file to `out`.
///
/// Unchanged files are reported as such and left untouched.
///
/// # Errors
///
/// Returns an error if a file cannot be written or `out` cannot be written to. Files are written in
/// order, so a write failure part way through leaves earlier files already bumped.
pub fn apply(changes: &[FileChange], out: &mut impl Write) -> Result<()> {
    for fc in changes {
        if !fc.is_changed() {
            writeln!(out, "{} - No changes.", fc.file)?;
            continue;
        }

        fs::write(&fc.path, &fc.updated)
            .with_context(|| format!("Could not write bump target '{}'", fc.file))?;
        writeln!(out, "Bumped {}", fc.file)?;
    }

    Ok(())
}

/// Write a zero-context unified diff of each planned change to `out`, without modifying any files.
///
/// Output for consecutive files is separated by a blank line.
///
/// # Errors
///
/// Returns an error if `out` cannot be written to.
pub fn preview(changes: &[FileChange], out: &mut impl Write) -> Result<()> {
    for (index, fc) in changes.iter().enumerate() {
        // Add a delimiter in between multi-file output
        if index > 0 {
            writeln!(out)?;
        }

        if !fc.is_changed() {
            writeln!(out, "{} - No changes.", fc.file)?;
            continue;
        }

        write!(out, "{}", diff::render(&fc.file, &fc.original, &fc.updated))?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "bump_tests.rs"]
mod tests;
