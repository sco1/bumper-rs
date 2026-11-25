# Changelog
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html) (`<major>`.`<minor>`.`<patch>`)

## v1.1.0
* `search` patterns must now contain `{current_version}`; patterns without the token are rejected when the configuration is loaded
* `bump` now warns about a rule that matches nothing, along with the reason (e.g. a key whose value differs from `current_version`)
* Add `bump --strict`, which fails without modifying files if any rule matches nothing

## v1.0.0
Initial release
