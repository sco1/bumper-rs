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

## Installation
`bumper` requires a Rust 1.85+ toolchain with Cargo.

Install the [latest version](https://github.com/sco1/bumper-rs/releases/latest) from the GitHub repository:

```text
$ cargo install --git https://github.com/sco1/bumper-rs --tag v1.0.0 --locked
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
$ cargo install --git https://github.com/sco1/bumper-rs --tag v1.0.0 --locked --root .tools/bumper
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

#### `tool.bumper.files`
* `file` - Path to target file, relative to the repository root

Exactly one replacement type:
  * `search` - Replacement string to search for in the target file; must contain a `{current_version}` tag
  * `key` - A dotted TOML key path, such as `package.version`, whose string value is replaced if it matches the current version
    * Key paths are split on `.`, so quoted keys containing dots are not supported
    * Only table keys can be traversed; values inside arrays or arrays of tables (e.g. `[[bin]]`) cannot be targeted

**NOTE:** `key` rules are recommended for TOML files when possible because they update one specific value rather than every matching text fragment.

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
      --no-check-lock  Do not check Cargo.lock after applying changes
  -h, --help           Print help
```
<!-- [[[end]]] (sum: bNco2f5pn9) -->

### `bumper init`
A small helper to initialize a starter `.bumper.toml` file with a default rule that updates `[package].version` in `Cargo.toml`.

**NOTE:** Be sure to update the sample version with your current version number before bumping with bumper.

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
