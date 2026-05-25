<!--
Thanks for the PR!

Target branch:
  - `develop`  for feature / fix / chore PRs (default for contributors)
  - `main`     only for `develop -> main` release promotions

If you targeted the wrong branch, change it in the PR settings before review.
-->

## what & why

<!-- short summary of the change and its motivation -->

## checklist

- [ ] PR targets `develop` (or, if releasing, `main`)
- [ ] commit title follows Conventional Commits (`feat:`, `fix:`, `chore:`...)
- [ ] `cargo fmt --all --check` is clean
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo test` is green locally
- [ ] new behaviour has tests (`parser`/`scanner` -> unit, HTTP -> integration)
- [ ] docs updated (`README.md`, doc-comments) if the public API changes
- [ ] no secrets / no `.env` files committed

## linked issues

<!-- closes #123, refs #456 -->

