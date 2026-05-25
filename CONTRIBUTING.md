# Contributing to constellation

Thanks for considering a contribution. This is a small personal project
maintained in spare time, so please read through this short guide before
you spend your time.

## quick triage

| if you want to...                          | do this                                                                                          |
| :----------------------------------------- | :----------------------------------------------------------------------------------------------- |
| report a bug                               | open an issue using the bug template                                                             |
| propose a feature                          | open an issue using the feature template, **before** writing code                                |
| report a security issue                    | follow [SECURITY.md](SECURITY.md), do not open a public issue                                    |
| fix a typo / clean a small thing           | open a PR directly                                                                               |
| change behaviour, add a feature, refactor  | open an issue first to align on the approach                                                     |

## local setup

```sh
git clone https://github.com/intjiraya/constellation
cd constellation
cargo test            # 56 unit + 15 integration tests
cargo clippy --all-targets -- -D warnings
cargo run -- --no-open
```

Toolchain is pinned in `rust-toolchain.toml` (stable). MSRV is `1.85`,
declared in `Cargo.toml`. The CI matrix verifies MSRV builds on every PR.

## conventional commits

Commits and PR titles use [Conventional Commits](https://www.conventionalcommits.org).
`release-plz` reads them to bump versions and assemble `CHANGELOG.md`.

| prefix      | when to use                                  |
| :---------- | :------------------------------------------- |
| `feat:`     | a new user-facing capability                 |
| `fix:`      | a bug fix                                    |
| `perf:`     | a performance improvement, no behaviour change |
| `refactor:` | a code change with no behaviour change       |
| `docs:`     | docs-only change                             |
| `chore:`    | non-code change (deps, repo hygiene)         |
| `ci:`       | CI/CD only                                   |
| `test:`     | test-only changes                            |

Breaking changes: append `!` (e.g. `feat!: drop --legacy-flag`) and add a
`BREAKING CHANGE:` footer in the body.

## what every PR must pass

`ci.yml` runs on every push and PR. A PR cannot merge until:

- `cargo fmt --all --check` is clean
- `cargo clippy --all-targets --all-features -- -D warnings` is clean
- `cargo test` is green on Linux, macOS, Windows, and the MSRV row
- `cargo doc --no-deps --all-features` builds with `RUSTDOCFLAGS=-D warnings`
- `cargo audit` reports no advisories
- `cargo deny check` passes (`deny.toml`)

If a check fails, the PR comment will tell you which one. Run it locally
first to keep the loop short.

## test discipline

Parser and scanner follow TDD. New paths there require a fixture under
`tests/fixtures/sessions/` and a unit test pinning the expected behaviour.

HTTP changes need an integration test in `tests/http_integration.rs`,
using the `tower::ServiceExt::oneshot` pattern already in place.

The frontend has no JS test infra yet; if you touch `static/app.js`,
load `cchats` locally and manually verify the affected flow.

## review

This is a one-maintainer project for now. Expect a few days of latency
on PRs. Tag PRs with the relevant area in the title
(`feat(parser):`, `fix(pty):`, ...) to help triage.

## code of conduct

By participating you agree to abide by [the Code of Conduct](CODE_OF_CONDUCT.md).
