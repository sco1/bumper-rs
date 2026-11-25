# Changelog
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html) (`<major>`.`<minor>`.`<patch>`)

## v1.1.0
### Changed
* `search` patterns must now contain `{current_version}`; patterns without the token are rejected when the configuration is loaded
* `bump` now warns about a rule that matches nothing, along with the reason (e.g. a key whose value differs from `current_version`)
* `init` now starts from the version in `Cargo.toml` (`[package].version`, or `[workspace.package].version` for workspaces), targets the matching key, and reports where the starting version came from
* `tool.bumper.files` is now optional; without it, only `.bumper.toml` is bumped
* Raise the minimum supported Rust version to 1.95

### Added
* Add `bump --strict`, which fails without modifying files if any rule matches nothing

### Fixed
* CalVer bumps now reject a `current_version` whose year and month do not form a valid date (e.g. `2026.13.0`)
* Bumping a version component that is already at its maximum value now fails instead of panicking or wrapping around to `0`

## v1.0.0
Initial release
