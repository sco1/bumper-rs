use super::render;

#[test]
fn render_returns_an_empty_string_for_identical_content() {
    let got = render(
        "./Cargo.toml",
        "[package]\nversion = \"0.1.0\"\n",
        "[package]\nversion = \"0.1.0\"\n",
    );

    assert_eq!(got, "");
}

#[test]
fn render_produces_an_exact_single_hunk_diff() {
    let original = "[package]\nname = \"example\"\nversion = \"0.1.0\"\n";
    let updated = "[package]\nname = \"example\"\nversion = \"0.2.0\"\n";

    let got = render("./Cargo.toml", original, updated);

    assert_eq!(
        got,
        concat!(
            "--- ./Cargo.toml\n",
            "+++ ./Cargo.toml\n",
            "@@ -3 +3 @@\n",
            "-version = \"0.1.0\"\n",
            "+version = \"0.2.0\"\n",
        ),
    );
}

#[test]
fn render_produces_separate_zero_context_hunks() {
    let original = "zero\nold-one\nmiddle\nstill-middle\nold-two\nlast\n";
    let updated = "zero\nnew-one\nmiddle\nstill-middle\nnew-two\nlast\n";

    let got = render("./README.md", original, updated);

    assert_eq!(
        got,
        concat!(
            "--- ./README.md\n",
            "+++ ./README.md\n",
            "@@ -2 +2 @@\n",
            "-old-one\n",
            "+new-one\n",
            "@@ -5 +5 @@\n",
            "-old-two\n",
            "+new-two\n",
        ),
    );
}

#[test]
fn render_normalizes_crlf_to_lf_for_display() {
    let original = "[package]\r\nversion = \"0.1.0\"\r\n";
    let updated = "[package]\r\nversion = \"0.1.1\"\r\n";

    let got = render("./Cargo.toml", original, updated);

    assert_eq!(
        got,
        concat!(
            "--- ./Cargo.toml\n",
            "+++ ./Cargo.toml\n",
            "@@ -2 +2 @@\n",
            "-version = \"0.1.0\"\n",
            "+version = \"0.1.1\"\n",
        ),
    );
    assert!(!got.contains('\r'));
}

#[test]
fn render_normalizes_mixed_line_endings_for_display() {
    let original = "heading\r\nversion = \"0.1.0\"\ntrailer\r\n";
    let updated = "heading\r\nversion = \"0.1.1\"\ntrailer\r\n";

    let got = render("./Cargo.toml", original, updated);

    assert_eq!(
        got,
        concat!(
            "--- ./Cargo.toml\n",
            "+++ ./Cargo.toml\n",
            "@@ -2 +2 @@\n",
            "-version = \"0.1.0\"\n",
            "+version = \"0.1.1\"\n",
        ),
    );
    assert!(!got.contains('\r'));
}

#[test]
fn render_omits_the_missing_newline_hint() {
    let got = render("./README.md", "release: 0.1.0", "release: 0.2.0");

    assert!(
        !got.contains("\\ No newline at end of file"),
        "unexpected missing-newline hint:\n{got}",
    );
}
