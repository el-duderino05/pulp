# Contributing to Pulp

Thank you for helping with Pulp. Please also read our [Code of Conduct](CODE_OF_CONDUCT.md).

## Licence

Pulp is released under the [Mozilla Public License 2.0](LICENSE). By contributing, you agree that your contribution is released under the same licence.

## Sign off every commit

Every commit needs a Developer Certificate of Origin (DCO) sign-off. Add it with:

```
git commit -s
```

The sign-off certifies that you wrote the contribution, or otherwise have the right to submit it under the project's licence. Read the full text at <https://developercertificate.org>.

## Before you open a pull request

- Tests must pass: `cargo test -p pulp-core`
- Formatting must be clean: `cargo fmt`
- Lints must be clean: `cargo clippy -- -D warnings`
- New source files must start with the licence line: `// SPDX-License-Identifier: MPL-2.0`

## How Pulp should behave

The specs in [`docs/specs/`](docs/specs/) are the source of truth for behaviour. If the code and a spec disagree, raise it in an issue or pull request.
