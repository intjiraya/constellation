<div align="center">

<br>

# constellation

#### every claude code chat, in one place - fast, local, single binary

<br>

</div>

> `claude --resume` only shows chats from the directory you're in.
> **constellation** reads every chat from every project, in one place,
> with a real PTY-resume right in the browser.

<br>

## install

```sh
git clone https://github.com/intjiraya/constellation
cd constellation
cargo install --path .
```

<details>
<summary><b>future install paths</b> - pacman, brew, scoop, apt</summary>

```sh
# arch (AUR) - coming soon
yay -S constellation

# macos / linux (homebrew) - coming soon
brew install intjiraya/tap/constellation

# windows (scoop) - coming soon
scoop install constellation

# debian / ubuntu (.deb) - coming soon
curl -fsSL https://github.com/intjiraya/constellation/releases/latest/download/constellation_amd64.deb \
  | sudo dpkg -i
```

</details>

<br>

## use

```sh
cchats                        # http://127.0.0.1:6767 + opens browser
cchats --port 9090
cchats --no-open
cchats --root /custom/path    # override ~/.claude/projects
```

<br>

## features

| feature                           | what it does                                                                              |
| :-------------------------------- | :---------------------------------------------------------------------------------------- |
| `every chat in one place`         | reads every `~/.claude/projects/<sanitized>/*.jsonl`, groups by project, sorts by recency |
| `full-text search across chats`   | server-side inverted index + suffix array for substring lookup, snippet highlighting      |
| `smart client search`             | multi-term AND, operators (`project:`, `model:`, `has:`, `before:`, `after:`), quotes     |
| `live resume in the browser`      | click, spawns `claude --resume <id>` inside a PTY, bridged through WebSocket to xterm.js  |
| `fork without scarring`           | one-click `--fork-session` from any chat, original untouched                              |
| `new chat from the rail`          | start a fresh `claude` session in any indexed project's cwd                               |
| `token accounting`                | input / cache-create / cache-read / output buckets, per chat, per project, all-up         |
| `single binary`                   | rust, no runtime, no node, no python, `rust-embed` ships every asset inside               |
| `DNS-rebinding hardened`          | binds 127.0.0.1, rejects non-loopback `Host` and `Origin`, strict CSP, vendored scripts   |

<br>

## search

Type in the search bar. Supports a small DSL:

```
auth bug                            multi-term AND across title / content
"merge conflict"                    quoted phrase
project:web                         filter by project (matches display path)
model:opus has:tool                 model contains "opus" AND has tool calls
has:cache before:2026-04-01         has cached tokens, last activity before date
auth project:api after:2026-01-01   combine freely
```

Plain terms hit the server's inverted index and search **inside the message
bodies** — user text, assistant text, thinking blocks, tool inputs and tool
outputs. Operators are evaluated client-side against session metadata.

<br>

## blazing fast

Split rebuild publishes projects/sessions metadata immediately and indexes
search bodies in a parallel background phase.

| metric                   |        value |
| :----------------------- | -----------: |
| cold start (server up)   |       5.6 ms |
| metadata ready (152 ses) |        ~3 ms |
| search ready (152 ses)   |     ~150 ms  |
| RSS idle                 |     18.6 MiB |
| `/api/stats` p50         |      0.08 ms |
| `/api/projects` p50      |      0.14 ms |
| big session parse        |        27 ms |
| reindex 234 MiB          |       430 ms |

### search benchmark

Synthetic JSONL bodies (~4 KiB each, 30-word vocab repeated, 1 turn per
session), single-threaded x86_64. Real Claude sessions tend to have lower term
density so latency drops accordingly.

| sessions | rebuild() total | RSS Δ (KiB) | search "auth" p50 | search "auth tool" p50 |
| -------: | --------------: | ----------: | ----------------: | ---------------------: |
|      100 |          ~8 ms  |       ~2.5 K|           ~0.9 ms |                ~1.4 ms |
|     1000 |         ~58 ms  |        ~17 K|           ~8.9 ms |                ~15 ms  |
|     5000 |        ~282 ms  |        ~76 K|           ~46 ms  |                ~76 ms  |

Reproduce locally: `cargo run --release --example bench_search`.

Single binary, no runtime, no warmup. It just starts.

<br>

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

<br>
<div align="center">

`built in rust, vendored to the bone, loopback-only by default`

</div>
