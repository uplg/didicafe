# DidiCafe

![CI](https://github.com/uplg/didicafe/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)

Captive portal daemon for time-limited WiFi access. Built in Rust, designed for single-board computers running OpenWrt.

## Overview

DidiCafe turns a small router board into a complete paid WiFi hotspot system. Customers connect to the WiFi, enter a token they received after paying, and get internet access for the duration they purchased. When time expires, access stops automatically.

- **Single binary** — ~5 MB static musl binary, zero runtime dependencies
- **Captive portal** — RFC 8908/8910 compliant, works on all devices (iOS, Android, Windows, Mac, Linux)
- **Token-based auth** — Human-readable codes (e.g., `DIDI-A7F3-K9X2`), single-use, configurable duration
- **Admin panel** — Generate tokens, monitor sessions, track revenue, audit log
- **Session resilience** — Survives reboots; active sessions are restored automatically
- **Security** — Argon2id password hashing, CSRF protection, rate limiting, server-side sessions

## Architecture

```
[Starlink / ISP]
       |
    (Ethernet WAN)
       |
+------+------+
|  SBC (BPI-  |
|  R3 Mini)   |
|             |
|  didicafe   |  <-- Single Rust binary
|  hostapd    |  <-- WiFi access point
|  dnsmasq    |  <-- DHCP + DNS
|  nftables   |  <-- Firewall (allow/deny)
+-------------+
       |
  (WiFi 802.11ac/ax)
       |
  [Customer devices]
```

## Quick Start

See [`docs/INSTALL.md`](docs/INSTALL.md) for the full deployment guide on a Banana Pi BPI-R3 Mini with OpenWrt.

### Development

```sh
# Clone and build
cargo build

# Run in dev mode (mock firewall, combined HTTP listener)
cargo run -- --config config/didicafe.dev.toml

# Run with migrations only
cargo run -- --migrate

# Cross-compile for OpenWrt (aarch64 musl)
cargo zigbuild --release --target aarch64-unknown-linux-musl
```

### Prerequisites

- Rust 1.85+ (edition 2024)
- For cross-compilation: `zig` + `cargo-zigbuild`

## Project Structure

```
didicafe/
├── src/
│   ├── main.rs              # Entry point, dual-listener setup (HTTP portal + HTTPS admin)
│   ├── config.rs            # TOML configuration with validation
│   ├── db/                  # SQLite database (sqlx), models, queries, migrations
│   ├── firewall/            # nftables controller (trait-based, mock in dev)
│   ├── net/                 # Network utilities (ARP resolution, IP detection)
│   ├── services/            # Business logic (tokens, sessions, rate limiting, CSRF)
│   └── web/                 # Axum routes (portal, admin, API, CAPPORT, CPD)
├── templates/               # Askama HTML templates (server-rendered)
├── static/                  # CSS, JavaScript, assets
├── config/                  # Example configs, hostapd, dnsmasq, nftables, init script
├── migrations/              # SQLx migrations
├── scripts/                 # Cert generation, cross-compile helpers
└── docs/
    ├── DESIGN.md            # Full system design document
    ├── INSTALL.md           # Step-by-step deployment guide
    └── CAPABILITIES.md      # End-user feature overview
```

## Documentation

| Document | Audience | Content |
|----------|----------|---------|
| [`docs/CAPABILITIES.md`](docs/CAPABILITIES.md) | End-users, venue managers | What the system does, how to use it |
| [`docs/DESIGN.md`](docs/DESIGN.md) | Developers, integrators | Architecture, network design, data model, deployment |
| [`docs/INSTALL.md`](docs/INSTALL.md) | Integrators | Full OpenWrt install and deployment procedure |
| [`docs/UPGRADE.md`](docs/UPGRADE.md) | Integrators | Remote upgrade via Tailscale |

## License

MIT — see [LICENSE](LICENSE).
