# Contributing to constellation

Thanks for considering a contribution. This is a small personal project
maintained in spare time, so please read through this short guide before
you spend your time.

## branch model

```
   feature/foo, fix/bar, chore/baz
                  │
                  ▼  (PR)
              develop ────────────── ongoing integration
                  │
                  ▼  (PR, when ready to release)
                main ────────────── stable, release-plz lives here
                  │
                  ▼
              v0.1.x tag → GitHub Release → AUR
```

| branch | rules |
| :----- | :---- |
| `main` | stable. **Direct push prohibited.** Only release-plz auto-PRs land here. Each merge bumps a tag and triggers the release pipeline. |
| `develop` | integration. **Default target for contributions.** All feature/fix PRs merge here. |
| `feature/<topic>`, `fix/<topic>`, `chore/<topic>` | short-lived. Open from `develop`, merge back into `develop`. Auto-deleted on merge. |

Releases happen by opening a PR `develop -> main`. release-plz then opens
its bump PR on `main`, you merge it, the rest is automatic.

## quick triage

| if you want to...                          | do this                                                                                          |
| :----------------------------------------- | :----------------------------------------------------------------------------------------------- |
| report a bug                               | open an issue using the bug template                                                             |
| propose a feature                          | open an issue using the feature template, **before** writing code                                |
| report a security issue                    | follow [SECURITY.md](SECURITY.md), do not open a public issue                                    |
| fix a typo / clean a small thing           | open a PR directly **against `develop`**                                                         |
| change behaviour, add a feature, refactor  | open an issue first to align on the approach, then PR against `develop`                          |

## local setup

```sh
git clone https://github.com/intjiraya/constellation
cd constellation
git checkout develop                            # work off develop
git switch -c fix/short-description             # short-lived branch

cargo test                                      # 56 unit + 15 integration tests
cargo clippy --all-targets -- -D warnings
cargo run -- --no-open
```

Toolchain is pinned in `rust-toolchain.toml` (stable). MSRV is `1.85`,
declared in `Cargo.toml`. The CI matrix verifies MSRV builds on every PR.

## conventional commits

Commits and PR titles use [Conventional Commits](https://www.conventionalcommits.org).
`release-plz` reads them to bump versions and assemble `CHANGELOG.md`.

| prefix      | when to use                                  | release-plz |
| :---------- | :------------------------------------------- | :---------- |
| `feat:`     | a new user-facing capability                 | minor bump  |
| `fix:`      | a bug fix                                    | patch bump  |
| `perf:`     | a performance improvement, no behaviour change | patch bump |
| `refactor:` | a code change with no behaviour change       | patch bump  |
| `docs:`     | docs-only change                             | shown in changelog |
| `chore:`    | non-code change (deps, repo hygiene)         | skipped     |
| `ci:`       | CI/CD only                                   | skipped     |
| `test:`     | test-only changes                            | skipped     |
| `build:`    | build system changes                         | shown       |

Breaking changes: append `!` (e.g. `feat!: drop --legacy-flag`) and add a
`BREAKING CHANGE:` footer in the body. release-plz then bumps the major.

## what every PR must pass

`ci.yml` runs on every push and PR (against both `main` and `develop`).
A PR cannot merge until:

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

## release flow (maintainer cheat-sheet)

1. Work accumulates on `develop` via PRs.
2. When ready, open a PR `develop -> main`. Title:
   `chore: promote develop`. Squash-merge the PR (one stable commit on main).
3. release-plz observes the new main commit, opens a `chore: release v0.x.y` PR.
4. Merge that PR. release-plz tags the commit.
5. `release.yml` cross-compiles bin's for 5 targets, publishes the GitHub Release.
6. `aur.yml` listens to the release event, pushes a refreshed `constellation-bin`
   to AUR (sha256 recomputed from the release tarballs).

You touch nothing manually after step 2.

## review

This is a one-maintainer project for now. Expect a few days of latency
on PRs. Tag PRs with the relevant area in the title
(`feat(parser):`, `fix(pty):`, ...) to help triage.

## code of conduct

By participating you agree to abide by [the Code of Conduct](CODE_OF_CONDUCT.md).
