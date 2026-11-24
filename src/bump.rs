use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use semver::Version;

use crate::config::{BUMPER_CONFIG_FILE, BumpRule};

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

fn group_rules(rules: &[BumpRule]) -> Vec<(&str, Vec<&str>)> {
    // For this application we'll generally only have a handful of replacement rules, so searching
    // through Vec should be sufficient vs. using HashMap or indexmap
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for rule in rules {
        match groups.iter_mut().find(|(file, _)| *file == rule.file) {
            Some((_, searches)) => searches.push(&rule.search),
            None => groups.push((rule.file.as_str(), vec![rule.search.as_str()])),
        }
    }

    groups
}

fn replace_all(content: &str, searches: &[&str], current: &str, next: &str) -> String {
    searches.iter().fold(content.to_owned(), |running, search| {
        running.replace(
            &search.replace(VERSION_TOKEN, current), // Old; replace template with current ver
            &search.replace(VERSION_TOKEN, next),    // New; replace template with next ver
        )
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

    for (file, searches) in group_rules(rules) {
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
        let updated = replace_all(&original, &searches, &current, &next);
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

#[cfg(test)]
#[path = "bump_tests.rs"]
mod tests;
