//! Version parsing and next-version calculation.
//!
//! Both supported schemes are stored as a [`semver::Version`]; CalVer versions map
//! `<YYYY>.<MM>.<MICRO>` onto `major.minor.patch`.
use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use jiff::{Timestamp, civil::Date, tz::TimeZone};
use semver::Version;

use crate::config::VersioningType;

/// Version component to increment.
///
/// `Major`, `Minor`, and `Patch` apply to SemVer projects; `Date` applies to CalVer projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BumpType {
    Major,
    Minor,
    Patch,
    Date,
}

/// A calendar year and month, used as the reference date for CalVer bumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearMonth {
    /// Calendar year, e.g. `2026`.
    pub year: u64,
    /// Calendar month, `1` through `12`.
    pub month: u64,
}

impl YearMonth {
    /// Return the current year and month in UTC.
    ///
    /// # Panics
    ///
    /// Panics if the year or month cannot be converted to `u64`, which is not expected for any
    /// timestamp produced by the system clock.
    pub fn now_utc() -> Self {
        let now_dt = Timestamp::now().to_zoned_datetime(TimeZone::UTC);

        let year =
            u64::try_from(now_dt.year()).expect("Could not retrieve year from UTC timestamp");
        let month =
            u64::try_from(now_dt.month()).expect("Could not retrieve month from UTC timestamp");

        Self { year, month }
    }
}

/// Parse a release version string.
///
/// # Errors
///
/// Returns an error if `s` is not a valid SemVer string, or if it carries prerelease
/// (e.g. `1.0.0-rc.1`) or build metadata (e.g. `1.0.0+abc`), neither of which is supported.
pub fn parse_version(s: &str) -> Result<Version> {
    let ver = Version::parse(s).with_context(|| format!("Invalid current_version '{s}'"))?;

    ensure!(
        ver.pre.is_empty(),
        "Invalid current_version '{s}': prerelease versions are not supported"
    );
    ensure!(
        ver.build.is_empty(),
        "Invalid current_version '{s}': build metadata is not supported"
    );

    Ok(ver)
}

/// Return an error unless the year and month of a CalVer `version` form a valid calendar date.
///
/// The year must be at most 9999 and the month must be from 1 to 12. The micro component is not
/// constrained.
fn ensure_valid_calver(version: &Version) -> Result<()> {
    let year = i16::try_from(version.major);
    let month = i8::try_from(version.minor);
    let is_valid = matches!(
        (year, month),
        (Ok(year), Ok(month)) if Date::new(year, month, 1).is_ok()
    );

    ensure!(
        is_valid,
        "Current CalVer version '{version}' does not form a valid <YYYY>.<MM>.<MICRO> date; \
        the year must be at most 9999 and the month from 1 to 12."
    );
    Ok(())
}

/// Return `component + 1`, or an error if that would overflow.
fn increment(component: u64) -> Result<u64> {
    component
        .checked_add(1)
        .context("Version component is already at its maximum value")
}

/// Increment the requested SemVer component, resetting all lower components to zero.
///
/// # Errors
///
/// Returns an error if the bumped component is already at its maximum value.
///
/// # Panics
///
/// Panics if called with [`BumpType::Date`]; callers must route CalVer bumps to [`bump_calver`].
fn bump_semver(current: &Version, bump: BumpType) -> Result<Version> {
    let next = match bump {
        BumpType::Major => Version::new(increment(current.major)?, 0, 0),
        BumpType::Minor => Version::new(current.major, increment(current.minor)?, 0),
        BumpType::Patch => Version::new(current.major, current.minor, increment(current.patch)?),
        BumpType::Date => unreachable!("bump_semver called with BumpType::Date"),
    };
    Ok(next)
}

/// Compute the next CalVer version relative to `today`.
///
/// If `current` is already in `today`'s year and month, the micro (patch) component is
/// incremented. Otherwise the version rolls over to `<year>.<month>.0`.
///
/// # Errors
///
/// Returns an error if `current` does not form a valid date, or is dated later than `today`; either
/// usually indicates a misconfigured `current_version`. Also returns an error if `current` is in
/// `today`'s month and its micro component is already at its maximum value.
fn bump_calver(current: &Version, today: YearMonth) -> Result<Version> {
    ensure_valid_calver(current)?;
    if (current.major, current.minor) > (today.year, today.month) {
        bail!(
            "Current CalVer version '{current}' is later than the current UTC month '{}.{}'; \
            review the configured version before bumping.",
            today.year,
            today.month
        );
    }

    if (current.major, current.minor) == (today.year, today.month) {
        Ok(Version::new(
            current.major,
            current.minor,
            increment(current.patch)?,
        ))
    } else {
        Ok(Version::new(today.year, today.month, 0))
    }
}

/// Compute the version that follows `current` under the given versioning scheme.
///
/// CalVer bumps use the current UTC year and month as the reference date.
///
/// # Errors
///
/// Returns an error if `bump` is incompatible with `versioning` (SemVer projects require `major`,
/// `minor`, or `patch`; CalVer projects require `date`), if a CalVer `current` version does not
/// form a valid date or is dated in the future, or if the bumped component is already at its
/// maximum value.
pub fn next_version(
    current: &Version,
    versioning: VersioningType,
    bump: BumpType,
) -> Result<Version> {
    match (versioning, bump) {
        (VersioningType::Semver, BumpType::Major | BumpType::Minor | BumpType::Patch) => {
            bump_semver(current, bump)
        }
        (VersioningType::Calver, BumpType::Date) => {
            let today = YearMonth::now_utc();
            bump_calver(current, today)
        }
        (VersioningType::Semver, BumpType::Date) => {
            bail!("SemVer projects must bump by major, minor, or patch.");
        }
        (VersioningType::Calver, _) => {
            bail!("CalVer projects must bump by date.");
        }
    }
}

#[cfg(test)]
#[path = "unit_tests/version_tests.rs"]
mod tests;
