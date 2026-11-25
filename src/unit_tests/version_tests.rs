use semver::Version;

use super::{BumpType, YearMonth, bump_calver, next_version, parse_version};
use crate::config::VersioningType;

const TODAY: YearMonth = ym(2026, 11);

const OVERFLOW_ERROR: &str = "Version component is already at its maximum value";

const fn ym(year: u64, month: u64) -> YearMonth {
    YearMonth { year, month }
}

fn version(input: &str) -> Version {
    Version::parse(input).expect("test version should be valid SemVer")
}

#[test]
fn parse_version_rejects_unsupported_versions_and_names_the_input() {
    let cases = [
        ("1.0", "Invalid current_version '1.0'"),
        (
            "1.2.3-rc.1",
            "Invalid current_version '1.2.3-rc.1': prerelease versions are not supported",
        ),
        (
            "1.2.3+build.5",
            "Invalid current_version '1.2.3+build.5': build metadata is not supported",
        ),
    ];

    for (input, expected) in cases {
        let err = parse_version(input).expect_err("version should be rejected");

        assert!(
            err.to_string().contains(expected),
            "case: {input}; error: {err}"
        );
    }
}

#[test]
fn next_version_bumps_semver_components_and_resets_lower_ones() {
    let cases = [
        (BumpType::Major, "2.0.0"),
        (BumpType::Minor, "1.3.0"),
        (BumpType::Patch, "1.2.4"),
    ];

    for (bump, expected) in cases {
        let got = next_version(&version("1.2.3"), VersioningType::Semver, bump)
            .expect("SemVer bump should succeed");

        assert_eq!(got, version(expected), "case: {bump:?}");
    }
}

#[test]
fn next_version_rejects_bumping_a_semver_component_at_its_maximum() {
    let cases = [
        (Version::new(u64::MAX, 0, 0), BumpType::Major),
        (Version::new(0, u64::MAX, 0), BumpType::Minor),
        (Version::new(0, 0, u64::MAX), BumpType::Patch),
    ];

    for (current, bump) in cases {
        let err = next_version(&current, VersioningType::Semver, bump)
            .expect_err("overflowing bump should be rejected");

        assert!(
            err.to_string().contains(OVERFLOW_ERROR),
            "case: {current} {bump:?}; error: {err}"
        );
    }
}

#[test]
fn next_version_resets_lower_semver_components_at_their_maximum() {
    // Lower components are reset rather than incremented, so being at the maximum is not an error
    let cases = [
        (
            Version::new(1, u64::MAX, u64::MAX),
            BumpType::Major,
            Version::new(2, 0, 0),
        ),
        (
            Version::new(1, 1, u64::MAX),
            BumpType::Minor,
            Version::new(1, 2, 0),
        ),
    ];

    for (current, bump, expected) in cases {
        let got = next_version(&current, VersioningType::Semver, bump)
            .expect("SemVer bump should succeed");

        assert_eq!(got, expected, "case: {current} {bump:?}");
    }
}

#[test]
fn next_version_rejects_invalid_versioning_and_bump_combinations() {
    let cases = [
        (
            VersioningType::Semver,
            BumpType::Date,
            "SemVer projects must bump by major, minor, or patch",
        ),
        (
            VersioningType::Calver,
            BumpType::Patch,
            "CalVer projects must bump by date",
        ),
    ];

    for (versioning, bump, expected) in cases {
        let err = next_version(&version("1.2.3"), versioning, bump)
            .expect_err("mismatched bump should be rejected");

        assert!(
            err.to_string().contains(expected),
            "case: {versioning:?} {bump:?}; error: {err}"
        );
    }
}

#[test]
fn next_version_uses_the_current_utc_month_for_calver() {
    // A valid date that is always earlier than the clock, so the bump always rolls over to the
    // current month
    let current = Version::new(2000, 1, 0);

    let before = YearMonth::now_utc();
    let got = next_version(&current, VersioningType::Calver, BumpType::Date)
        .expect("CalVer date bump should succeed");
    let after = YearMonth::now_utc();

    // The month can change between the two clock reads, so either is acceptable
    let expected_before = Version::new(before.year, before.month, 0);
    let expected_after = Version::new(after.year, after.month, 0);
    assert!(
        got == expected_before || got == expected_after,
        "got {got}; expected {expected_before} or {expected_after}"
    );
}

#[test]
fn bump_calver_increments_within_the_month_and_rolls_over_otherwise() {
    let cases = [
        (
            "same month increments the micro",
            "2026.11.4",
            TODAY,
            "2026.11.5",
        ),
        (
            "new month resets the micro",
            "2026.10.3",
            TODAY,
            "2026.11.0",
        ),
        ("new year rolls over", "2025.12.3", ym(2026, 1), "2026.1.0"),
        (
            "same month of an earlier year rolls over",
            "2025.11.5",
            TODAY,
            "2026.11.0",
        ),
        // A non-CalVer version, such as one seeded by `init` from Cargo.toml, is still a valid
        // (if ancient) date, so switching a project to CalVer keeps working
        ("non-CalVer version rolls over", "1.2.3", TODAY, "2026.11.0"),
    ];

    for (case, current, today, expected) in cases {
        let got = bump_calver(&version(current), today).expect("CalVer bump should succeed");

        assert_eq!(got, version(expected), "case: {case}");
    }
}

#[test]
fn bump_calver_rejects_a_future_dated_version() {
    let err = bump_calver(&version("2027.3.0"), TODAY)
        .expect_err("future-dated CalVer version should be rejected");

    assert!(
        err.to_string().contains("later than the current UTC month"),
        "error: {err}"
    );
}

#[test]
fn bump_calver_rejects_versions_that_do_not_form_a_valid_date() {
    let cases = [
        ("month above 12", "2025.13.0"),
        ("month zero", "2026.0.4"),
        ("year beyond 9999", "10000.1.0"),
        ("year beyond i16", "70000.1.0"),
        // Validity is checked before the future-date check, so the more specific error wins
        ("invalid and future dated", "2099.13.0"),
    ];

    for (case, input) in cases {
        let err = bump_calver(&version(input), TODAY)
            .expect_err("a version that is not a valid date should be rejected");
        let message = err.to_string();

        assert!(
            message.contains(&format!("'{input}'")) && message.contains("does not form a valid"),
            "case: {case}; error: {message}"
        );
    }
}

#[test]
fn bump_calver_rejects_a_micro_at_its_maximum_in_the_current_month() {
    let err = bump_calver(&Version::new(2026, 11, u64::MAX), TODAY)
        .expect_err("overflowing micro bump should be rejected");

    assert!(err.to_string().contains(OVERFLOW_ERROR), "error: {err}");
}

#[test]
fn bump_calver_rolls_over_a_micro_at_its_maximum_from_an_earlier_month() {
    // Rolling over resets the micro component rather than incrementing it
    let got =
        bump_calver(&Version::new(2026, 10, u64::MAX), TODAY).expect("CalVer bump should succeed");

    assert_eq!(got, Version::new(2026, 11, 0));
}
