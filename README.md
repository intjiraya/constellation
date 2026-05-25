# constellation

A local web UI to browse and resume every Claude Code chat across all your projects.

Claude Code stores conversation history per working directory in
`~/.claude/projects/<sanitized-cwd>/<session-uuid>.jsonl`, which means
`claude --resume` only shows sessions from the directory you happen to be in.
Constellation reads all of them, groups them by project, and lets you preview,
fork or resume any chat from one place — with an embedded terminal so resume
happens right in the browser tab.

## install

From source (requires Rust 1.85+):

```sh
git clone https://github.com/intjiraya/constellation
cd constellation
cargo install --path .
```

## use

```sh
cchats                 # serves on http://127.0.0.1:6767 and opens the browser
cchats --port 9090
cchats --no-open
cchats --root /path/to/claude/projects
```

## what it does

- **scans** `~/.claude/projects/*/` for `.jsonl` session files
- **parses** each session into a normalized model (turns, blocks, tool calls,
  timestamps, token usage)
- **serves** a local web UI showing every chat across every project with
  search, previews, token statistics and prompt-cache hit ratios
- **resumes** any chat by spawning `claude --resume <id>` inside a PTY,
  bridged into the browser through an `xterm.js` terminal

Nothing leaves your machine, nothing is uploaded.

## architecture

```mermaid
flowchart LR
    disk[("~/.claude/projects/<br/>*.jsonl")]

    subgraph backend["cchats (single binary)"]
        direction LR
        scan["scanner + parser<br/>(JSONL → typed model)"]
        idx["index<br/>(Arc&lt;RwLock&gt;)"]
        api["axum router"]
        pty["portable-pty bridge"]
        assets[["embedded static<br/>(rust-embed)"]]
    end

    browser[/"browser<br/>vanilla JS + xterm.js"/]
    claude>"claude --resume &lt;id&gt;"]

    disk --> scan --> idx --> api
    assets --> api
    api -- "REST /api/*" --> browser
    api -- "WS /api/sessions/{id}/pty" --> pty
    api -- "WS /api/projects/{name}/new-chat" --> pty
    pty -- "spawn + bytes" --> claude
    pty <--> browser
```

Static assets (HTML / CSS / JS / SVG logo) are embedded into the release binary
via `rust-embed`, so distribution is a single ~2.7 MiB stripped binary with no
runtime dependencies beyond `libc`.

### resume flow

When you click **▶ resume** on a chat, the browser opens a WebSocket to the
binary, which forks a real `claude` process inside a PTY and proxies bytes both
ways. xterm.js renders the terminal in-page.

```mermaid
sequenceDiagram
    autonumber
    participant B as browser (xterm.js)
    participant A as axum
    participant P as portable-pty
    participant C as claude

    B->>A: WS upgrade  /api/sessions/{id}/pty
    A->>P: openpty + spawn(claude --resume {id})
    P->>C: fork + exec inside the pty slave
    Note over P,C: master fd ↔ slave tty<br/>both directions live
    loop while session is open
        C-->>P: stdout bytes (pty master)
        P-->>A: blocking read → tokio mpsc
        A-->>B: binary frame
        B->>A: text frame {"type":"input", "data": "…"}
        A->>P: write to master fd (spawn_blocking)
        P->>C: stdin
        B->>A: text frame {"type":"resize", cols, rows}
        A->>P: master.resize(cols, rows)
    end
    B-)A: close socket
    A->>C: kill + wait
```

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
