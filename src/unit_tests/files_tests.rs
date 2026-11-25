use std::fs;

use tempfile::TempDir;
use toml::Value;

use super::load_toml;

#[test]
fn load_toml_returns_none_for_a_missing_file() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");

    let got = load_toml::<Value>(&temp_dir.path().join("absent.toml"))
        .expect("a missing file is not an error");

    assert!(got.is_none());
}

#[test]
fn load_toml_parses_an_existing_file() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");
    let path = temp_dir.path().join("present.toml");
    fs::write(&path, "[package]\nversion = \"1.2.3\"\n").expect("could not write test file");

    let got = load_toml::<Value>(&path)
        .expect("file should load")
        .expect("file should be present");

    assert_eq!(got["package"]["version"].as_str(), Some("1.2.3"));
}

#[test]
fn load_toml_names_the_file_when_it_is_not_valid_toml() {
    let temp_dir = TempDir::new().expect("could not create temporary directory");
    let path = temp_dir.path().join("broken.toml");
    fs::write(&path, "[package\n").expect("could not write test file");

    let err = load_toml::<Value>(&path).expect_err("invalid TOML should be rejected");

    assert!(
        err.to_string()
            .contains(&format!("Could not parse '{}'", path.display())),
        "error: {err}"
    );
}
