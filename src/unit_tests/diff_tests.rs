use super::render;

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
fn render_normalizes_crlf_and_mixed_line_endings_for_display() {
    let original = "heading\r\nversion = \"0.1.0\"\r\ntrailer\n";
    let updated = "heading\r\nversion = \"0.1.1\"\r\ntrailer\n";

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
}

#[test]
fn render_omits_the_missing_newline_hint() {
    let got = render("./README.md", "release: 0.1.0", "release: 0.2.0");

    assert_eq!(
        got,
        concat!(
            "--- ./README.md\n",
            "+++ ./README.md\n",
            "@@ -1 +1 @@\n",
            "-release: 0.1.0\n",
            "+release: 0.2.0\n",
        ),
    );
}
