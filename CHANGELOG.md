# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.1.1](https://github.com/intjiraya/constellation/compare/v0.1.0...v0.1.1) - 2026-05-25

### Documentation
- redesign README - by @intjiraya
- fix README table alignment - by @intjiraya
- trim README - by @intjiraya
- tweak README ascii art - by @intjiraya

### Fixed
- windows path separator in display_path + add CoC 3.0 + apply cargo fmt - by @intjiraya

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

- WebSocket `Origin` enforced loopback-only (rejects same-machine cross-origin
  attaches).
- Child environment restricted to a 28-key allowlist (`TERM`, `PATH`, `HOME`,
  ...). No ambient credentials forwarded to spawned `claude`.
- Strict `Content-Security-Policy`: `default-src 'self'`, `frame-ancestors
  'none'`, all third-party scripts vendored inside the binary.
- DOMPurify mandatory, with a text-only fallback if it fails to load.
- Spawned cwd must canonicalise under `$HOME`.

[Unreleased]: https://github.com/intjiraya/constellation/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/intjiraya/constellation/releases/tag/v0.1.0
