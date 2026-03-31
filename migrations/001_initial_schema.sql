CREATE TABLE IF NOT EXISTS plan (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL,
    duration_minutes INTEGER NOT NULL,
    price_ariary     INTEGER NOT NULL,
    active           BOOLEAN NOT NULL DEFAULT 1,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS token (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    code        TEXT NOT NULL UNIQUE,
    name        TEXT,
    plan_id     INTEGER NOT NULL REFERENCES plan(id),
    status      TEXT NOT NULL DEFAULT 'unused'
                CHECK (status IN ('unused', 'active', 'expired', 'revoked')),
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    redeemed_at TEXT,
    expires_at  TEXT
);

CREATE TABLE IF NOT EXISTS session (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    token_id    INTEGER NOT NULL REFERENCES token(id),
    mac_address TEXT NOT NULL,
    ip_address  TEXT NOT NULL,
    started_at  TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at  TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'expired', 'disconnected'))
);

CREATE INDEX IF NOT EXISTS idx_token_code ON token(code);
CREATE INDEX IF NOT EXISTS idx_token_status ON token(status);
CREATE INDEX IF NOT EXISTS idx_session_mac ON session(mac_address);
CREATE INDEX IF NOT EXISTS idx_session_status ON session(status);
