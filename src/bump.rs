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

const VERSION_TOKEN: &str = "{current_version}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    path: PathBuf,
    file: String,
    original: String,
    updated: String,
}

impl FileChange {
    fn is_changed(&self) -> bool {
        self.original != self.updated
    }
}

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

fn replace_search(content: &str, search: &str, current: &str, next: &str) -> String {
    content.replace(
        &search.replace(VERSION_TOKEN, current),
        &search.replace(VERSION_TOKEN, next),
    )
}

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

fn apply_rules(content: &str, kinds: &[&RuleKind], current: &str, next: &str) -> Result<String> {
    kinds
        .iter()
        .try_fold(content.to_owned(), |running, kind| match kind {
            RuleKind::Search(search) => Ok(replace_search(&running, search, current, next)),
            RuleKind::Key(key) => replace_key(&running, key, current, next),
        })
}

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

pub fn preview(changes: &[FileChange], out: &mut impl Write) -> Result<()> {
    for fc in changes {
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
