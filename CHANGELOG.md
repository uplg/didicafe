# Changelog

All notable changes to DidiCafe are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.1] — 2026-05-18

### Fixed
- Replaced `rustls-pemfile` with `rustls-pki-types` (RUSTSEC-2025-0134, repo archived Aug 2025)
- Cleaned up unused license allowances in `deny.toml` for a zero-warning `cargo deny` output

### Changed
- Updated `askama` from 0.15 to 0.16
- Applied `rustfmt.toml` with edition 2024 style conventions across the entire codebase

### Added
- 8 unit tests for `build_tls_acceptor` (`src/main.rs`)
- `docs/UPGRADE.md` — remote upgrade guide via Tailscale

---

## [1.0.0] — 2026-05-18

**Initial release.** A complete captive portal system for time-limited WiFi access, built in Rust for OpenWrt routers.

### Core Features
- **Captive Portal** — RFC 8908/8910 compliant, auto-redirect on iOS, Android, Windows, macOS, and Linux
- **Token-based Authentication** — Human-readable codes (`DIDI-XXXX-XXXX`), single-use, configurable duration and bandwidth
- **Admin Panel** — Web dashboard for token generation, session monitoring, revenue tracking, and audit log
- **Session Resilience** — Active sessions survive reboots; automatically restored from SQLite on startup
- **nftables Firewall** — MAC + IP authorization with automatic cleanup on session expiry or disconnect
- **Dual HTTPS/HTTP Listeners** — Admin panel on HTTPS with self-signed TLS, portal on HTTP

### Security
- **Argon2id** password hashing (OWASP parameters: m=19456, t=2, p=1)
- **Server-side sessions** with CSRF protection (Synchronizer Token Pattern, per-MAC)
- **Rate limiting** — Two-tier throttle + ban on authentication endpoints
- **Strict MAC validation** — Regex-based, rejected before any business logic
- **Parameterized SQL queries** — Zero string interpolation via sqlx
- **Security headers** — CSP, X-Frame-Options, X-Content-Type-Options, HSTS, Permissions-Policy
- **Admin IP allowlist** — Configurable CIDR-based network filtering

### Architecture
- **Single binary** — ~5 MB static musl binary, zero runtime dependencies
- **SQLite** — Async via sqlx, in-memory for tests, file-based in production
- **Axum** — Clean route separation (portal, admin, API, CAPPORT, CPD)
- **Askama templates** — Server-rendered HTML with i18n support (EN/FR)
- **Trait-based firewall** — Mock implementation for dev/testing, real nftables for production

### Target Platform
- Banana Pi BPI-R3 Mini running OpenWrt
- Cross-compiled for `aarch64-unknown-linux-musl` via `cargo-zigbuild`

### Documentation
- `docs/DESIGN.md` — Full system architecture and design decisions
- `docs/INSTALL.md` — Step-by-step OpenWrt deployment guide
- `docs/CAPABILITIES.md` — End-user feature overview
- `docs/UPGRADE.md` — Remote upgrade procedure via Tailscale
