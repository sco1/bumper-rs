use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use jiff::{Timestamp, tz::TimeZone};
use semver::Version;

use crate::config::VersioningType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BumpType {
    Major,
    Minor,
    Patch,
    Date,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YearMonth {
    pub year: u64,
    pub month: u64,
}

impl YearMonth {
    pub fn now_utc() -> Self {
        let now_dt = Timestamp::now().to_zoned_datetime(TimeZone::UTC);

        let year =
            u64::try_from(now_dt.year()).expect("Could not retrieve year from UTC timestamp");
        let month =
            u64::try_from(now_dt.month()).expect("Could not retrieve month from UTC timestamp");

        Self { year, month }
    }
}

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

fn bump_semver(current: &Version, bump: BumpType) -> Version {
    match bump {
        BumpType::Major => Version::new(current.major + 1, 0, 0),
        BumpType::Minor => Version::new(current.major, current.minor + 1, 0),
        BumpType::Patch => Version::new(current.major, current.minor, current.patch + 1),
        BumpType::Date => unreachable!("bump_semver called with BumpType::Date"),
    }
}

fn bump_calver(current: &Version, today: YearMonth) -> Result<Version> {
    if (current.major, current.minor) > (today.year, today.month) {
        bail!(
            "Current CalVer version '{current}' is later than the current UTC month '{}.{}'; review the configured version before bumping.",
            today.year,
            today.month
        );
    }

    if (current.major, current.minor) == (today.year, today.month) {
        Ok(Version::new(
            current.major,
            current.minor,
            current.patch + 1,
        ))
    } else {
        Ok(Version::new(today.year, today.month, 0))
    }
}

pub fn next_version(
    current: &Version,
    versioning: VersioningType,
    bump: BumpType,
) -> Result<Version> {
    match (versioning, bump) {
        (VersioningType::Semver, BumpType::Major | BumpType::Minor | BumpType::Patch) => {
            Ok(bump_semver(current, bump))
        }
        (VersioningType::Calver, BumpType::Date) => {
            let today = YearMonth::now_utc();
            bump_calver(current, today)
        }
        (VersioningType::Semver, BumpType::Date) => {
            bail!("Semver projects must bump by major, minor, or patch.");
        }
        (VersioningType::Calver, _) => {
            bail!("Calver projects must bump by date.");
        }
    }
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
