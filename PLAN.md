# PLAN.md — Implementation Plan

Derived from DESIGN.md sections 1–14. Excludes Phase 2+ / Future Enhancements (section 15).

## Step 0: Codebase Hygiene
- [x] Update all dependencies to latest (`rusqlite 0.39`, `toml 1.1`, `rand 0.10`)
- [x] Replace `bcrypt` with `argon2id` (OWASP recommended)
- [x] Fix all `cargo clippy` warnings
- [x] Remove all `#[allow(dead_code)]`
- [x] Remove the useless `services/plan.rs` wrapper (it just delegates to `db` — call `db` directly)

## Step 1: Proper Error Handling
- [x] Define `AppError` enum with `thiserror` (Database, Firewall, Auth, NotFound, BadRequest, Internal)
- [x] Implement `IntoResponse` for `AppError` (maps to proper HTTP status codes + JSON/HTML)
- [x] Refactor all handlers to return `Result<_, AppError>` instead of `.unwrap_or_default()`
- [x] API returns proper HTTP status codes (400, 401, 404, 500)
- [x] Add `render()` helper for askama templates (askama 0.15 has no built-in axum IntoResponse)
- [x] API: `POST` returns 201 Created, `DELETE` returns 204 No Content
- [x] `revoke_token` validates token exists + status before revoking (NotFound, BadRequest)
- [x] Added `db.get_token_by_id()` method

## Step 2: Async Database Layer (rusqlite → sqlx)
- [x] Replace `rusqlite` with `sqlx` (async-native SQLite with connection pool)
- [x] Rewrite `db/mod.rs`: `Database` wraps `SqlitePool`, all methods are `async`
- [x] Add `sqlx::FromRow` derive to all model structs
- [x] Create `migrations/001_initial_schema.sql` (replaces `db/migrations.rs`)
- [x] Delete old `db/migrations.rs` (referenced rusqlite)
- [x] Update `web/error.rs`: `rusqlite::Error` → `sqlx::Error`
- [x] Update `services/token.rs`: `generate_tokens` + `validate_token` are now `async`
- [x] Update `services/session.rs`: all DB calls use `.await`
- [x] Update `web/portal.rs`, `web/admin.rs`, `web/api.rs`: add `.await` to all async calls
- [x] Update `main.rs`: `Database::open().await`, `migrate().await`, `get_active_sessions().await`
- [x] Fix `ThreadRng` `!Send` issue: separate RNG usage from `.await` points in `generate_tokens`
- [x] `cargo clippy` — 0 warnings, `cargo test` — all pass, `cargo check` — clean

## Step 3: Firewall Trait Abstraction
- [x] Define `trait Firewall: Send + Sync` with boxed-future methods (`authorize_mac`, `deauthorize_mac`, `init_ruleset`)
- [x] `NftablesController` implements `Firewall` trait
- [x] `MockFirewall` implements `Firewall` trait (records calls as `FirewallCall` enum, returns Ok)
- [x] `MockFirewall` gated behind `#[cfg(test)]` — zero dead-code warnings in release
- [x] `AppState.firewall` is now `Arc<dyn Firewall>` (object-safe via `Pin<Box<dyn Future>>` returns)
- [x] Test: `test_mock_records_calls` verifies mock records authorize/deauthorize/init calls in order
- [x] `cargo clippy` — 0 warnings, `cargo test` — 2 tests pass

## Step 4: MAC/IP Extraction from HTTP Requests
- [x] Created `src/net/arp.rs` — ARP table lookup with 3-tier fallback: `/proc/net/arp` (Linux, no subprocess), `ip neigh` (Linux fallback), `arp -an` (macOS/BSD)
- [x] Created `src/web/extractors.rs` — `ClientInfo { ip: IpAddr, mac: String }` custom axum extractor using `ConnectInfo<SocketAddr>` + ARP lookup
- [x] Updated `main.rs`: `axum::serve` now uses `app.into_make_service_with_connect_info::<SocketAddr>()`
- [x] Updated `portal.rs`: `portal_auth` uses `ClientInfo` extractor — removed all `"00:00:00:00:00:00"` and `"0.0.0.0"` placeholders
- [x] 3 unit tests for ARP parsing logic (`/proc/net/arp`, `ip neigh`, `arp -an` output formats)
- [x] `cargo clippy` — 0 warnings, `cargo test` — 5 tests pass

## Step 5: Admin Authentication (Session Cookies)
- [x] Created `src/services/admin_session.rs` — `AdminSessionStore` with `RwLock<HashMap>`, crypto-random 32-char session IDs (192-bit entropy, exceeds OWASP 128-bit minimum)
- [x] Added `session_timeout_seconds` to `AdminConfig` (default 3600s, serde default)
- [x] Added `admin_sessions: AdminSessionStore` to `AppState`
- [x] Created `AdminSession` extractor in `extractors.rs` — validates `didicafe_admin` cookie against store
- [x] Cookie helper: `extract_cookie()` parses raw `Cookie` header (no cookie crate dependency)
- [x] `login_submit`: on success, creates session + sets `HttpOnly; SameSite=Strict` cookie via `Set-Cookie`
- [x] `POST /admin/logout`: extracts session ID from cookie, invalidates server-side, clears cookie (`Max-Age=0`), redirects to login
- [x] Protected all `/admin/*` handlers (dashboard, tokens_page, generate_tokens) with `_admin: AdminSession`
- [x] Protected all `/api/*` handlers (7 endpoints) with `_admin: AdminSession`
- [x] `AppError::Unauthorized` redirects to `/admin/login` (303 redirect)
- [x] Removed `#[allow(dead_code)]` from `Unauthorized` variant
- [x] Lazy session expiry: expired sessions cleaned up on `validate()` calls
- [x] 11 new tests: session store CRUD + expiry + uniqueness, cookie parsing, cookie format
- [x] `cargo clippy` — 0 warnings, `cargo test` — 16 tests pass

## Step 6: Rate Limiting
- [x] Implement token auth rate limiter (per client IP, configurable from `[rate_limit]` config)
- [x] Track attempts in-memory (IP -> { count, window_start })
- [x] Ban after threshold (IP -> banned_until)
- [x] Integrate into `portal_auth` handler — check *before* any DB lookup (OWASP: never leak token existence)
- [x] `AppError::RateLimited` variant returns 429 Too Many Requests + `Retry-After` header when banned
- [x] 7 unit tests: under threshold, throttle, ban, ban persistence, IP independence, retry_after, window expiry
- [x] `cargo clippy` — 0 warnings, `cargo test` — 23 tests pass

### Step 7 : Admin UI/Portal rework
- [x] Design system: CSS maison (`static/style.css`) — CSS custom properties, warm cafe palette, responsive, accessible
- [x] i18n: `static/i18n.js` — client-side localization with `data-i18n` attributes, MG/FR/EN switcher, localStorage persistence
- [x] Rewrite `templates/base.html` — slim, loads CSS + i18n.js, no inline styles
- [x] Create `templates/admin/base.html` — admin layout with nav bar, lang switcher, logout button
- [x] Rewrite `templates/portal.html` — branded, accessible (ARIA), i18n, single token input, minimal clicks
- [x] Rewrite `templates/success.html` — live JS countdown timer (`aria-live`), auto-redirect on expiry
- [x] Rewrite `templates/expired.html` — clear messaging, CTA to re-enter token
- [x] Rewrite `templates/admin/login.html` — cohesive branding with portal, i18n
- [x] Rewrite `templates/admin/dashboard.html` — stat cards, session table, disconnect via `fetch()` DELETE (no form hack)
- [x] Rewrite `templates/admin/tokens.html` — generate form, token list with badges
- [x] No CDN, no npm, no build step — all assets served locally from SBC
- [x] `cargo clippy` — 0 warnings, `cargo test` — 23 tests pass

## Step 8: Portal Auth Endpoint Fix + Input Validation
- [x] DESIGN.md specifies `POST /portal/auth` — current code uses `POST /portal`. Align with design.
- [x] `GET /portal/status` endpoint (DESIGN.md section 6): returns current session status/time remaining for a client (by MAC)
- [x] Validate token input format: regex `^[A-Z0-9]{4}-[A-Z0-9]{4}$` (after prefix strip), charset-only, reject invalid chars
- [x] Validate token input length before DB lookup
- [x] Fix `success_page` to show real remaining time from DB session (was hardcoded 60 min)
- [x] Success page JS countdown syncs with `/portal/status` every 60s (drift correction)
- [x] 7 unit tests for `validate_token_format`: valid, wrong prefix, wrong length, missing dash, invalid charset, garbage, generated-tokens roundtrip
- [x] `cargo clippy` — 0 warnings, `cargo test` — 30 tests pass

## Step 9: CPD (Captive Portal Detection) Probe Handling
- [x] Intercept known CPD probe URLs and return 302 redirect to `/portal`:
  - Apple: `GET /hotspot-detect.html`
  - Android: `GET /generate_204`
  - Windows: `GET /connecttest.txt`
  - Linux/GNOME: `GET /check_network_status.txt`
- [x] Catch-all fallback: any unmatched route redirects to `/portal` (covers unknown CPD probes + stray DNAT traffic)
- [x] `cargo clippy` — 0 warnings, `cargo test` — 30 tests pass

## Step 10: Config Validation
- [x] Validate config on load: port in 1–65535, charset non-empty, token length >= 2 and even, password hash is valid Argon2id PHC string (starts with `$argon2id$`), rate limit coherence (ban >= max), all required fields non-empty
- [x] Return clear error messages on invalid config
- [x] 6 unit tests: valid config, port zero, empty charset, odd token length, bad password hash, ban below max
- [x] `cargo clippy` — 0 warnings, `cargo test` — 36 tests pass

## Step 11: `init_ruleset` on Startup
- [x] DESIGN.md boot sequence calls `init_ruleset` on startup — added `fw.init_ruleset().await?` before session restoration
- [x] `cargo clippy` — 0 warnings, `cargo test` — 36 tests pass

## Step 12: Templates — Return Directly
- [x] Verified: askama 0.15 does NOT implement `IntoResponse` for templates (the separate `askama_axum` crate is gone). The `render()` helper in `web/error.rs` is the correct approach. No changes needed.

## Step 13: API Completeness (per DESIGN.md section 6)
- [x] `PUT /api/plans/:id` — update a plan (name, duration, price, active)
- [x] Token list with filters: `GET /api/tokens?status=unused` (query param filtering with validation)
- [x] Consistent API response envelope: `{ "ok": true, "data": { ... } }` for all endpoints
- [x] Input validation on all write endpoints (name non-empty, duration > 0, count bounds, plan existence + active check)
- [x] Dashboard disconnect already uses `fetch()` DELETE (Step 7) — no changes needed
- [x] `cargo clippy` — 0 warnings, `cargo test` — 36 tests pass

## Step 14: Plan Management Admin UI + Sessions Page
- [x] Admin page to create/edit/deactivate plans (`GET/POST /admin/plans`)
- [x] Plan deactivation via `fetch()` PUT to `/api/plans/:id` (JS toggle button)
- [x] `templates/admin/sessions.html` — dedicated session management page with disconnect buttons
- [x] Updated admin nav bar with Plans + Sessions links
- [x] `cargo clippy` — 0 warnings, `cargo test` — 36 tests pass

## Step 15: Security Hardening
- [x] Security headers middleware: `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Content-Security-Policy`, `Referrer-Policy`, `X-XSS-Protection: 0`
- [x] Type-safe enums: `TokenStatus` (Unused/Active/Expired/Revoked) and `SessionStatus` (Active/Expired/Disconnected) with `Display`, `from_str`, `ALL` constant
- [x] All string status comparisons replaced with typed enum checks (`token_status()`, `session_status()`)
- [x] `cargo clippy` — 0 warnings, `cargo test` — 36 tests pass

## Step 16: Unit Tests
- [x] `db/` — CRUD operations on in-memory SQLite: plans (create, get, update, list), tokens (create, get, redeem, expire, revoke, list with filter), sessions (create, get by MAC, active list, disconnect, expire), daily stats
- [x] `db/models.rs` — `remaining_seconds` (future, past, bad format), `TokenStatus` roundtrip, `SessionStatus` roundtrip
- [x] `services/token.rs` — 7 tests: valid format, wrong prefix, wrong length, missing dash, invalid charset, garbage input, generated-tokens roundtrip
- [x] `services/rate_limit.rs` — 7 tests (already from Step 6)
- [x] `config.rs` — 6 tests: valid config, port zero, empty charset, odd token length, bad password hash, ban below max
- [x] `firewall/mock.rs` — mock records calls (already from Step 3)
- [x] `services/admin_session.rs` — session store CRUD + expiry + uniqueness, cookie parsing (already from Step 5)
- [x] `cargo clippy` — 0 warnings, `cargo test` — 52 tests pass

## Step 17: Integration / E2E Tests
- [x] Fixed ARP loopback issue: `ClientInfo` extractor now has `#[cfg(test)]` fallback MAC for loopback addresses
- [x] Fixed test requests: use `test_get()` and `test_post_form()` helpers that inject `ConnectInfo` extension
- [x] 7 integration tests in `web/portal.rs`: GET /portal, POST invalid format, POST nonexistent token, full auth flow, GET /portal/status, GET /portal/expired, GET /portal/success redirect
- [x] `cargo clippy` — 0 warnings, `cargo test` — 59 tests pass

## Step 18: Dev Mode (No Router Required)
- [x] Added `profile.dev` in Cargo.toml for faster builds
- [x] Dev mode uses `MockFirewall` instead of nftables (no root required)
- [x] Dev mode returns deterministic MAC based on IP (no ARP lookup needed)
- [x] ARP functions gated behind `#[cfg(not(debug_assertions))]` to avoid dead code warnings
- [x] `cargo clippy` — 0 warnings, `cargo test` — 59 tests pass

## Step 19: Cross-Compilation & Deployment
- [ ] Cross-compile as `aarch64-unknown-linux-musl` static binary
- [ ] Create deployment scripts for Alpine Linux on BPI-R3 Mini
- [ ] Test binary on target hardware

## Step 18: Cross-Compilation & Deployment Script
- [ ] `scripts/cross-compile.sh` — sets up `aarch64-unknown-linux-musl` target, builds release binary
- [ ] `scripts/setup.sh` — Alpine provisioning on BPI-R3 Mini (install packages, copy configs, enable services)
