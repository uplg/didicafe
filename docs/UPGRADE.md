# DidiCafe — Remote Upgrade Guide

How to upgrade the `didicafe` binary on a deployed box via Tailscale, without physical access.

---

## Prerequisites

- Tailscale installed and running on the box (see [INSTALL.md §8](./INSTALL.md#8-remote-access-tailscale))
- Tailscale installed on your workstation
- The box is tagged or named in your tailnet (e.g., `didicafe-cafename`)

---

## 1. Build the new binary

On your workstation:

```sh
cd ~/Github/didicafe

# Make sure you're on the right branch/tag
git checkout v1.0.1   # or main, or whatever

# Cross-compile for OpenWrt (aarch64 musl)
cargo zigbuild --release --target aarch64-unknown-linux-musl

# Verify the binary
file target/aarch64-unknown-linux-musl/release/didicafe
# → ELF 64-bit LSB executable, ARM aarch64, statically linked, stripped
```

---

## 2. SSH into the box via Tailscale

```sh
ssh root@didicafe-cafename
```

If you set up Tailscale SSH (`tailscale up --ssh`), this uses your tailnet identity — no SSH keys needed.

---

## 3. Back up the database

```sh
cp /srv/didicafe/didicafe.db /srv/didicafe/didicafe.db.bak.$(date +%Y%m%d)
ls -lh /srv/didicafe/didicafe.db.bak.*
```

If something goes wrong, you can always restore:
```sh
cp /srv/didicafe/didicafe.db.bak.20260518 /srv/didicafe/didicafe.db
```

---

## 4. Note current state

```sh
# Check running version (if you added --version support)
didicafe --version 2>/dev/null || echo "no version flag"

# Note active sessions count
logread -e didicafe | tail -5

# Check service status
service didicafe running && echo "running" || echo "stopped"
```

---

## 5. Upload the new binary

From your workstation (in a second terminal):

```sh
# Option A: Direct SCP over Tailscale
scp -O target/aarch64-unknown-linux-musl/release/didicafe \
  root@didicafe-cafename:/usr/local/bin/didicafe.new

# Option B: If you have a Tailscale SSH tunnel open
# (from the SSH session in step 2, you could also use rsync)
rsync -avz target/aarch64-unknown-linux-musl/release/didicafe \
  root@didicafe-cafename:/usr/local/bin/didicafe.new
```

---

## 6. Swap and restart

Back in your SSH session on the box:

```sh
# Verify the new binary is valid
chmod +x /usr/local/bin/didicafe.new
/usr/local/bin/didicafe.new --help >/dev/null 2>&1 && echo "binary OK" || echo "binary BROKEN"

# If binary is OK, swap it
if [ $? -eq 0 ]; then
    service didicafe stop
    mv /usr/local/bin/didicafe /usr/local/bin/didicafe.old
    mv /usr/local/bin/didicafe.new /usr/local/bin/didicafe
    service didicafe start
    echo "restart initiated"
else
    echo "ABORT: new binary failed --help check"
    rm /usr/local/bin/didicafe.new
fi
```

---

## 7. Verify

```sh
# Service should be running (procd respawns on crash)
sleep 3
service didicafe running && echo "OK" || echo "FAILED"

# Check listeners
netstat -tlnp 2>/dev/null | grep -E ":443|:8080"

# Check logs
logread -e didicafe | tail -10

# Verify sessions were restored
logread -e "restored" | tail -3
```

You should see something like:
```
didicafe: loading configuration from /etc/didicafe/didicafe.toml
didicafe: opening database at /srv/didicafe/didicafe.db
didicafe: restored N active sessions, expired M during downtime
didicafe: firewall ruleset initialized
didicafe: portal (HTTP) listening on 10.10.0.1:8080
didicafe: admin (HTTPS) listening on 10.10.0.1:443
```

---

## 8. Test the portal

From a phone or laptop on the café WiFi:

1. Connect to the `DidiCafe` WiFi
2. The captive portal should load normally
3. Test with an unused token to confirm auth still works

---

## Rollback

If the new version crashes or misbehaves:

```sh
# Stop the broken service
service didicafe stop

# Restore the old binary
mv /usr/local/bin/didicafe.old /usr/local/bin/didicafe

# Restart
service didicafe start

# Verify
logread -e didicafe | tail -5
```

The old binary is kept at `didicafe.old` until you explicitly delete it in step 9.

---

## 9. Clean up

If everything is stable after a few minutes:

```sh
rm /usr/local/bin/didicafe.old
# Keep the backup DB for a day or two, then:
# rm /srv/didicafe/didicafe.db.bak.*
```

---

## Upgrading config or templates

If the new version includes changes to `didicafe.toml`, templates, or static assets:

```sh
# Upload new config (if schema changed)
scp -O config/didicafe.toml root@didicafe-cafename:/etc/didicafe/didicafe.toml.new

# Upload new templates and statics
scp -O -r templates static migrations root@didicafe-cafename:/opt/didicafe/

# Run migrations if needed
/usr/local/bin/didicafe --config /etc/didicafe/didicafe.toml --migrate

# Then restart as described above
service didicafe stop && service didicafe start
```

> **Note:** Always review config changes before overwriting. The existing `/etc/didicafe/didicafe.toml` contains your production password hash and cafe-specific settings. Never blindly overwrite it.
