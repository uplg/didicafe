# AGENTS.md — Development Guidelines

## Development Practices

### Quality Standards

- **Zero warnings**: `cargo clippy` and `cargo check` must pass with zero warnings at all times. No `#[allow(dead_code)]` or `#[allow(unused)]` unless structurally justified with a comment.
- **Latest dependencies**: All crates pinned to their latest stable version. Run `cargo upgrade --incompatible` before any work session.
- **DRY**: No duplicated logic. Extract shared patterns into helpers, traits, or macros. If two functions look similar, refactor.
- **Error handling**: Use `thiserror` for domain errors, `anyhow` only at the binary boundary (`main`). Handlers return typed errors, never `.unwrap()` in production paths. `.unwrap()` is acceptable only in tests.
- **Type safety**: Use newtypes and enums over raw strings/integers. Token status, session status, MAC addresses, plan IDs — all should be typed where practical.
- **No placeholders**: Every function does what it claims. No `// TODO` in shipped code without a tracking issue.
- **Defense-in-depth**: Always think about loopholes, edge cases, security needs to be SOTA even innovative.

### Rust / Axum / Tokio Best Practices

- **Axum handlers** return `Result<impl IntoResponse, AppError>` with a proper `AppError` type implementing `IntoResponse`. No `.unwrap_or_default()` in handlers — propagate errors.
- **State**: Use `Arc<AppState>` via axum's `State` extractor. No globals.
- **Async discipline**: Never block the tokio runtime. Database uses `sqlx` (async-native SQLite with connection pool) — no `spawn_blocking` needed. All DB methods are `async` and called with `.await`.
- **Templates**: askama 0.15 implements `IntoResponse` natively. Return the template directly, don't wrap in `Html(tpl.render().unwrap())`.
- **Middleware**: Use tower layers for cross-cutting concerns (auth, rate limiting, logging). Don't inline auth checks in every handler.
- **Extractors**: Write custom axum extractors for recurring patterns (client IP/MAC, authenticated admin session).
- **Serialization**: API responses use a consistent envelope. Errors return proper HTTP status codes (400, 401, 404, 500), not 200 with `{"error": "..."}`.

### Testing

- **Unit tests**: Every module has `#[cfg(test)] mod tests`. Test the logic, not the framework.
- **Integration tests**: `tests/` directory for API-level tests using `axum::test::TestClient` or direct handler calls with mock state.
- **Database tests**: Use in-memory SQLite (`:memory:`) for fast, isolated tests. Each test gets a fresh DB.
- **Firewall tests**: `NftablesController` is behind a trait. Tests use a mock implementation — never call real `nft` in CI.
- **Run before commit**: `cargo clippy && cargo test && cargo build --release`.

### Security (OWASP)

Follow OWASP best practices across the board. Reference cheat sheets: https://cheatsheetseries.owasp.org

- **Password storage**: Argon2id only (OWASP Password Storage Cheat Sheet). Minimum params: `m=19456` (19 MiB), `t=2`, `p=1`. No bcrypt, no scrypt, no PBKDF2. Hash stored as PHC string format (`$argon2id$v=19$m=19456,t=2,p=1$...`).
- **Session management**: Server-side sessions. Cookies: `HttpOnly`, `SameSite=Strict`, `Secure` (when over HTTPS). Session IDs are cryptographically random (min 128 bits entropy). Sessions expire after inactivity. Explicit logout invalidates server-side state.
- **Rate limiting**: Per-IP throttling on authentication endpoints. Temporary ban after repeated failures. Return `429 Too Many Requests`, never leak whether a token exists.
- **Input validation**: Validate and sanitize all user input at the boundary. Token codes are uppercased and trimmed. Reject unexpected characters. SQL parameterized queries only — no string interpolation.
- **Error handling**: Never leak internal details (stack traces, SQL errors, file paths) to the client. Log full details server-side. Return generic messages to users.
- **Headers**: Set security headers where applicable (`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Content-Security-Policy`).
- **Secrets in config**: Password hashes in config files, never plaintext passwords. Config files excluded from version control if they contain real credentials.
- **Minimal data collection**: Only store what's necessary (MAC, IP, session duration). No tracking beyond what's needed for the service. Clear expired session data.

### Code Organization

- One concern per file. If a file exceeds ~300 lines, split it.
- Public API at module level (`mod.rs` re-exports), implementation in submodules.
- Config structs validate on load (e.g., port range, non-empty charset, valid Argon2id PHC hash format).
