# NetPeek

Network diagnostics for your own machine — honest tests, local-only results.

## Features

| Check | Mechanism | External contact? |
|---|---|---|
| Local hostname & IPs | UDP-connect trick + hostname | none |
| Network interfaces | `if-addrs` crate, loopbacks excluded | none |
| Ping | System `ping` binary (parsed) | only the host you type |
| DNS resolution | System resolver (`getaddrinfo`) | your configured resolvers |
| DNS resolvers | `/etc/resolv.conf` (Linux/macOS) / `ipconfig /all` (Windows) | none |
| Connectivity | TCP connect to `1.1.1.1:443` | Cloudflare, only on button click |
| Public IP | `ifconfig.me` | only on button click |
| Download speed | `speed.cloudflare.com/__down?bytes=8000000` (~8 MB) | only on button click |
| Latency history | Saved to app-data dir (`latency-history.jsonl`) | none |
| Diagnostic report | Markdown export to your Downloads folder | none |

Un-run checks are reported as `_not run_` — NetPeek never fabricates results.

## What NetPeek does NOT do

- No port scanning, no ARP scanning, no packet capture, no "who else is on my network"
- No telemetry, analytics or accounts
- No background network activity (all checks are user-initiated)

## Install

Prebuilt packages land in `src-tauri/target/release/bundle/` after a build, or build from source:

```bash
pnpm install
pnpm tauri build
```

Linux system packages needed to build: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf` (plus Rust ≥ 1.77 and Node ≥ 20).

## Development

```bash
pnpm dev        # vite dev server
pnpm tauri dev  # app with hot reload
pnpm test       # vitest (report builder)
cargo test      # in src-tauri — parser unit tests
pnpm build      # frontend production build
pnpm tauri build# native bundles
```

## Project layout

```
src/App.svelte          — UI (single window)
src/lib/report.ts       — report builder + formatters (unit-tested)
src-tauri/src/lib.rs    — Tauri commands
src-tauri/src/netutil.rs— host validation + ping/resolv.conf parsers (unit-tested)
```

## Platform notes

- **Linux**: ping uses the system binary; DNS resolvers read from `/etc/resolv.conf`
  (systemd-resolved users: this shows the stub `127.0.0.53`, which is accurate).
- **Windows**: ping via `ping.exe`; resolvers via `ipconfig /all` parsing.
- **macOS**: ping via BSD ping; resolvers read from `/etc/resolv.conf`.
- Public IP can differ from what DNS shows you (CGNAT vs direct); both are shown side by side honestly.
