use semver::Version;

use super::{BumpType, YearMonth, bump_calver, next_version, parse_version};
use crate::config::VersioningType;

fn version(input: &str) -> Version {
    Version::parse(input).expect("test version should be valid SemVer")
}

#[test]
fn parse_version_accepts_valid_release_versions() {
    let cases = ["0.1.0", "2025.1.0"];

    for input in cases {
        let got = parse_version(input).expect("version should parse");

        assert_eq!(got.to_string(), input, "case: {input}");
    }
}

#[test]
fn parse_version_rejects_invalid_or_unsupported_versions() {
    let cases = [
        "1.0",
        "v1.2.3",
        "01.2.3",
        "2025.01.0",
        "1.2.3-rc.1",
        "1.2.3+build.5",
        "",
    ];

    for input in cases {
        let err = parse_version(input).expect_err("version should be rejected");

        assert!(
            err.to_string().contains("current_version"),
            "case: {input}; error: {err}",
        );
    }
}

#[test]
fn parse_version_reports_the_invalid_input() {
    let err = parse_version("1.2.3-rc.1").expect_err("prerelease version should be rejected");

    assert!(err.to_string().contains("1.2.3-rc.1"), "error: {err}",);
    assert!(err.to_string().contains("prerelease"), "error: {err}",);
}

#[test]
fn next_version_bumps_semver_versions() {
    let cases = [
        ("0.0.1", BumpType::Major, "1.0.0"),
        ("0.0.1", BumpType::Minor, "0.1.0"),
        ("0.0.1", BumpType::Patch, "0.0.2"),
        ("0.1.0", BumpType::Major, "1.0.0"),
        ("0.1.0", BumpType::Minor, "0.2.0"),
        ("0.1.0", BumpType::Patch, "0.1.1"),
        ("1.0.0", BumpType::Major, "2.0.0"),
        ("1.0.0", BumpType::Minor, "1.1.0"),
        ("1.0.0", BumpType::Patch, "1.0.1"),
    ];

    for (input, bump, expected) in cases {
        let current = version(input);

        let got = next_version(&current, VersioningType::Semver, bump)
            .expect("SemVer bump should succeed");

        assert_eq!(got, version(expected), "case: {input} {bump:?}",);
    }
}

#[test]
fn bump_calver_increments_or_rolls_over_correctly() {
    let cases = [
        (
            "2025.1.0",
            YearMonth {
                year: 2025,
                month: 1,
            },
            "2025.1.1",
        ),
        (
            "2025.10.0",
            YearMonth {
                year: 2025,
                month: 10,
            },
            "2025.10.1",
        ),
        (
            "2025.1.0",
            YearMonth {
                year: 2025,
                month: 2,
            },
            "2025.2.0",
        ),
        (
            "2025.1.1",
            YearMonth {
                year: 2025,
                month: 2,
            },
            "2025.2.0",
        ),
        (
            "2025.10.0",
            YearMonth {
                year: 2025,
                month: 11,
            },
            "2025.11.0",
        ),
        (
            "2025.10.1",
            YearMonth {
                year: 2025,
                month: 11,
            },
            "2025.11.0",
        ),
        (
            "2025.12.3",
            YearMonth {
                year: 2026,
                month: 1,
            },
            "2026.1.0",
        ),
        (
            "2024.1.5",
            YearMonth {
                year: 2025,
                month: 1,
            },
            "2025.1.0",
        ),
    ];

    for (input, today, expected) in cases {
        let current = version(input);

        let got = bump_calver(&current, today).expect("CalVer bump should succeed");

        assert_eq!(
            got,
            version(expected),
            "case: {input} @ {}-{}",
            today.year,
            today.month,
        );
    }
}

#[test]
fn bump_calver_rejects_a_future_dated_version() {
    let current = version("2027.3.0");
    let today = YearMonth {
        year: 2026,
        month: 11,
    };

    let err =
        bump_calver(&current, today).expect_err("future-dated CalVer version should be rejected");

    assert!(
        err.to_string().contains("later than the current UTC month"),
        "error: {err}",
    );
    assert!(err.to_string().contains("review"), "error: {err}",);
}

#[test]
fn next_version_rejects_invalid_versioning_and_bump_combinations() {
    let current = version("1.2.3");

    let semver_date_err = next_version(&current, VersioningType::Semver, BumpType::Date)
        .expect_err("SemVer date bump should be rejected");

    assert!(
        semver_date_err
            .to_string()
            .contains("must bump by major, minor, or patch"),
        "error: {semver_date_err}",
    );

    let calver_patch_err = next_version(&current, VersioningType::Calver, BumpType::Patch)
        .expect_err("CalVer patch bump should be rejected");

    assert!(
        calver_patch_err.to_string().contains("must bump by date"),
        "error: {calver_patch_err}",
    );
}

#[test]
fn next_version_uses_the_current_utc_month_for_calver() {
    // A real UTC month is always in 1..=12, so this cannot be interpreted
    // as the same CalVer month as the current clock value.
    let current = Version::new(0, 0, 0);

    let before = YearMonth::now_utc();

    let got = next_version(&current, VersioningType::Calver, BumpType::Date)
        .expect("CalVer date bump should succeed");

    let after = YearMonth::now_utc();

    let expected_before = Version::new(before.year, before.month, 0);
    let expected_after = Version::new(after.year, after.month, 0);

    assert!(
        got == expected_before || got == expected_after,
        "got {got}; expected {expected_before} or {expected_after}",
    );
}
