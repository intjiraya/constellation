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
| `live resume in the browser`      | click, spawns `claude --resume <id>` inside a PTY, bridged through WebSocket to xterm.js  |
| `fork without scarring`           | one-click `--fork-session` from any chat, original untouched                              |
| `new chat from the rail`          | start a fresh `claude` session in any indexed project's cwd                               |
| `token accounting`                | input / cache-create / cache-read / output buckets, per chat, per project, all-up         |
| `single 3.2 MiB binary`           | rust, no runtime, no node, no python, `rust-embed` ships every asset inside               |
| `loopback-only, origin-checked`   | binds 127.0.0.1, rejects non-loopback `Origin`, strict CSP, vendored CDN scripts          |

<br>

## blazing fast

| metric              |        value |
| :------------------ | -----------: |
| cold start          |       5.6 ms |
| index ready (152)   |       447 ms |
| RSS idle            |     18.6 MiB |
| `/api/stats` p50    |      0.08 ms |
| `/api/projects` p50 |      0.14 ms |
| big session parse   |        27 ms |
| reindex 234 MiB     |       430 ms |

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
