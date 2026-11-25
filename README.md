# bumper-rs
[![GitHub Release](https://img.shields.io/github/v/release/sco1/bumper-rs)](https://github.com/sco1/bumper-rs/releases)
[![GitHub License](https://img.shields.io/github/license/sco1/bumper-rs)](https://github.com/sco1/bumper-rs/blob/main/LICENSE)

Automatically increment the project's version number

A Rust companion to my Python-based [`bumper`](https://github.com/sco1/bumper) utility.

## Supported Versioning Schemes
* [Semantic Versioning (SemVer)](https://semver.org/#semantic-versioning-200)
  * Assumes `<MAJOR>.<MINOR>.<PATCH>`
* [Calendar Versioning (CalVer)](https://calver.org/)
  * Assumes `<YYYY>.<MM>.<MICRO>`
  * The year and month must form a valid date, and the month is not zero-padded (e.g. `2026.1.0`)

## Installation
`bumper` requires a Rust 1.95+ toolchain with Cargo.

Install the [latest version](https://github.com/sco1/bumper-rs/releases/latest) from the GitHub repository:

```text
$ cargo install --git https://github.com/sco1/bumper-rs --tag v1.1.0 --locked
```

You can confirm proper installation via the `bumper` CLI:

<!-- [[[cog
import subprocess

help_text = subprocess.run(
    ["cargo", "run", "--quiet", "--locked", "--bin", "bumper", "--", "--help"],
    check=True,
    capture_output=True,
    text=True,
).stdout

cog.outl("```text")
cog.outl("$ bumper --help")
cog.out(help_text)
cog.outl("```")
]]] -->
```text
$ bumper --help
Automatically increment the project's version number.

Usage: bumper <COMMAND>

Commands:
  bump  Bump the configured project version
  init  Generate a starter .bumper.toml configuration file
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```
<!-- [[[end]]] (sum: 9oj8I8I7x0) -->

Project-local installations can be achieved using:

```text
$ cargo install --git https://github.com/sco1/bumper-rs --tag v1.1.0 --locked --root .tools/bumper
```

Which would then be invoked by:

```text
$ ./.tools/bumper/bin/bumper<.exe> <...>
```

## Configuration
`bumper` searches for its configuration options in a `.bumper.toml` file, located at the repository root.

### Required Fields
#### `tool.bumper`
* `current_version` - The current software version; this is automatically incremented when bumping.
* `versioning_type` - Versioning type to be used; accepted values are `"semver"` and `"calver"`

### Optional Fields
#### `tool.bumper.files`
Replacement rules for the files to bump. `.bumper.toml` itself is always bumped, so if no rules are given only its `current_version` is updated.

Each rule requires:
* `file` - Path to target file, relative to the repository root

Exactly one replacement type:
  * `search` - Replacement string to search for in the target file; must contain a `{current_version}` tag
  * `key` - A dotted TOML key path, such as `package.version`, whose string value is replaced if it matches the current version
    * Key paths are split on `.`, so quoted keys containing dots are not supported
    * Only table keys can be traversed; values inside arrays or arrays of tables (e.g. `[[bin]]`) cannot be targeted

**NOTE:** `key` rules are recommended for TOML files when possible because they update one specific value rather than every matching text fragment.

**NOTE:** `search` patterns are matched as plain text anywhere in the file, including inside longer strings; this allows a rule to target a version inside a larger string, such as a URL. It also means that a pattern that starts or ends with `{current_version}` can match a longer version containing the current one. For example, with `current_version = "1.0.1"`, the pattern `v{current_version}` also matches `v1.0.10` and `v1.0.1-rc.1`. Where possible, include surrounding text on both sides of the token (e.g. `"--tag v{current_version} "`).

### Example Configuration
The basic configuration looks something like the following:

#### SemVer
```toml
[tool.bumper]
current_version = "1.0.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"
```

#### CalVer
```toml
[tool.bumper]
current_version = "2026.11.0"
versioning_type = "calver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"
```

Multiple replacements within the same file can also be specified:

```toml
[tool.bumper]
current_version = "1.0.0"
versioning_type = "semver"

[[tool.bumper.files]]
file = "./Cargo.toml"
key = "package.version"

[[tool.bumper.files]]
file = "./README.md"
search = "--tag v{current_version}"

[[tool.bumper.files]]
file = "./README.md"
search = 'current_version = "{current_version}"'
```

## CLI
### `bumper bump`
Bump your project's version number using your defined configuration.

<!-- [[[cog
import subprocess

help_text = subprocess.run(
    ["cargo", "run", "--quiet", "--locked", "--bin", "bumper", "--", "bump", "--help"],
    check=True,
    capture_output=True,
    text=True,
).stdout

cog.outl("```text")
cog.outl("$ bumper bump --help")
cog.out(help_text)
cog.outl("```")
]]] -->
```text
$ bumper bump --help
Bump the configured project version

Usage: bumper bump [OPTIONS] <BUMP>

Arguments:
  <BUMP>  Version component to bump [possible values: major, minor, patch, date]

Options:
      --dry-run        Preview the planned changes without modifying files
      --strict         Fail without modifying files if any rule matches nothing
      --no-check-lock  Do not check Cargo.lock after applying changes
  -h, --help           Print help
```
<!-- [[[end]]] (sum: QQgLCkaWG0) -->

#### Unmatched Rules
A rule that matches nothing in its target file is reported with a warning that gives the reason, but the bump proceeds:

```text
$ bumper bump patch
Bumper bumping: v1.2.2 -> v1.2.3
WARNING: ./Cargo.toml: key 'package.version' has value '1.0.0', not the current version
./Cargo.toml - No changes.
Bumped .bumper.toml
```

This usually means `current_version` has drifted from a target file, or a `search` pattern no longer matches the file's text. Rules for the same file are applied in configuration order, so a rule may also find nothing because an earlier rule already replaced its match.

Use `--strict` to treat unmatched rules as an error instead: `bumper` lists every unmatched rule and exits without modifying any files. This also applies to `--dry-run`.

### `bumper init`
A small helper to initialize a starter `.bumper.toml` file with a rule that updates the version in `Cargo.toml`.

<!-- [[[cog
import subprocess

help_text = subprocess.run(
    ["cargo", "run", "--quiet", "--locked", "--bin", "bumper", "--", "init", "--help"],
    check=True,
    capture_output=True,
    text=True,
).stdout

cog.outl("```text")
cog.outl("$ bumper init --help")
cog.out(help_text)
cog.outl("```")
]]] -->
```text
$ bumper init --help
Generate a starter .bumper.toml configuration file

Usage: bumper init [OPTIONS]

Options:
      --versioning-type <VERSIONING_TYPE>
          Versioning scheme for the generated configuration [default: semver] [possible values: semver, calver]
      --ignore-existing
          Overwrite an existing .bumper.toml file
  -h, --help
          Print help
```
<!-- [[[end]]] (sum: bI+3y0w0UY) -->

The starting `current_version` and the rule's `key` are taken from `Cargo.toml`:
* `[package].version`, if set
* Otherwise `[workspace.package].version`, for a virtual workspace or a package that inherits its version from a workspace in the same `Cargo.toml`

```text
$ bumper init
Created .bumper.toml with current_version 2.3.4 (from Cargo.toml `package.version`)
```

If neither is found, `current_version` defaults to `0.1.0` (SemVer) or `<YYYY>.<MM>.0` (CalVer) and the rule targets `package.version`.

**NOTE:** When the default is used, be sure to update `current_version` to match your project before bumping.

`init` fails without writing anything if the version in `Cargo.toml` is not supported, e.g. a prerelease such as `1.0.0-rc.1`.
