use similar::TextDiff;

/// Render a zero-context unified diff for a changed file.
///
/// CRLF is normalized to LF for display only. The caller retains the original
/// file content and is responsible for preserving its original line endings
/// when applying changes.
pub(crate) fn render(file: &str, original: &str, updated: &str) -> String {
    let original = original.replace("\r\n", "\n");
    let updated = updated.replace("\r\n", "\n");

    TextDiff::from_lines(&original, &updated)
        .unified_diff()
        .context_radius(0)
        .header(file, file)
        .missing_newline_hint(false)
        .to_string()
}

#[cfg(test)]
#[path = "diff_tests.rs"]
mod tests;
