-- Key-value settings table for runtime configuration.
-- Allows the admin to change theme color, cafe name, etc. from the UI
-- without restarting the service or editing the TOML config file.
-- TOML values serve as defaults; DB values override them at runtime.
CREATE TABLE IF NOT EXISTS setting (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
