# Security Policy

## Supported Versions

constellation is pre-`1.0`. Only the latest released version receives
security fixes. If you are on an older release, please update first.

| Version    | Status                |
| :--------- | :-------------------- |
| `0.1.x`    | actively supported    |
| `< 0.1.0`  | not supported         |

## Reporting a Vulnerability

**Do not open a public issue for security findings.**

Please use GitHub's private vulnerability reporting:

1. Go to https://github.com/intjiraya/constellation/security
2. Click **Report a vulnerability**
3. Fill in the form with what you found and how to reproduce it

If GitHub's form is unavailable, send a direct message on Discord to
`@nugget.waffle`.

### What to include

- A short description of the issue.
- Steps to reproduce, ideally a minimal proof-of-concept.
- The version (`cchats --version`), OS, and any relevant config
  (`--host`, `--root` overrides, custom env).
- The impact you observed and the impact you believe is possible.

### What to expect

- An acknowledgement within 7 days.
- A fix plan or a decision within 14 days.
- Public disclosure coordinated with you, typically alongside the patch
  release. Reporters are credited in the release notes unless they ask
  otherwise.

## Threat Model

constellation is a single-user local tool. The boundaries we defend are:

- The loopback HTTP / WebSocket surface (origin-checked, default-deny CSP,
  vendored scripts, no third-party network).
- Spawned `claude` child process (explicit env allowlist, cwd guard,
  bounded reap).

We do **not** defend against:

- An attacker with read access to `~/.claude/projects/`
  (they already have the chat content).
- An attacker with execute access to the user's account
  (they can run `claude` directly).
- Vulnerabilities in `claude` itself
  (please report those to Anthropic).
