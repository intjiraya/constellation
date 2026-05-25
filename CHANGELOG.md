# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/intjiraya/constellation/compare/v0.1.1...v0.1.2) - 2026-05-25

### Added

- *(search)* full-text search, DSL, DNS-rebinding hardening, split rebuild

### Documentation

- *(packaging)* document automated AUR flow, drop manual-flow emphasis

## [0.1.1](https://github.com/intjiraya/constellation/compare/v0.1.0...v0.1.1) - 2026-05-25

Maintenance release. CI / release-pipeline plumbing only, no user-visible changes.

## [0.1.0] - 2026-05-25

### Added

- Local web UI for browsing every Claude Code chat across all projects in
  `~/.claude/projects/`.
- Real PTY resume in the browser via `xterm.js` and `portable-pty` over a
  loopback WebSocket.
- Fork an existing chat (`--fork-session`) without scarring the original.
- Start a fresh `claude` session in any indexed project's cwd from the rail.
- Token accounting with four buckets: input, cache-creation, cache-read,
  output. Aggregated per chat, per project, and across the whole index.
- Single 3.2 MiB stripped Rust binary, no runtime dependencies beyond `libc`.

### Security

- WebSocket `Origin` enforced loopback-only (rejects same-machine
  cross-origin attaches).
- Child environment restricted to a 28-key allowlist (`TERM`, `PATH`,
  `HOME`, ...). No ambient credentials forwarded to spawned `claude`.
- Strict `Content-Security-Policy`: `default-src 'self'`,
  `frame-ancestors 'none'`, all third-party scripts vendored inside
  the binary.
- DOMPurify mandatory, with a text-only fallback if it fails to load.
- Spawned cwd must canonicalise under `$HOME`.
