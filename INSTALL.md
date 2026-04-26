# DidiCafe — Install Guide for BPI-R3 Mini

End-to-end procedure for installing **OpenWrt 25.12.2** (mainline, official)
on the eMMC of a Banana Pi BPI-R3 Mini, then deploying the `didicafe`
captive-portal daemon on top.

The board's SPI NAND keeps the factory OpenWrt as a **recovery image**.
A hardware switch on the PCB selects which medium boots: NAND (recovery)
or eMMC (production). If the eMMC firmware ever fails, flip the switch
back to NAND and you have a working router again.

---

## 0. Why this stack

We tried Alpine Linux first and burnt many hours on bring-up: no official
image, OpenRC silent hangs, MAC subsystem mis-init, MAC randomization
breaking captive CSRF. Pivoting to OpenWrt was the right call:

- **Official BPI-R3 Mini image** maintained by upstream OpenWrt (target
  `mediatek/filogic`).
- **procd init**: ~5s boot, no hang traps.
- **`fw4` + nftables natively**: clean firewall config via UCI.
- **hostapd / dnsmasq pre-configured**: minimal extra setup.
- **WED hardware NAT offload** active by default → wire-speed throughput.
- **musl** native: our `aarch64-unknown-linux-musl` Rust binary runs
  unchanged.
- **apk** package manager (Alpine's, ported into OpenWrt 24+) — `apk add`
  for any extra package.

---

## 1. Prerequisites

### Hardware

- BPI-R3 Mini, 2 GB / 8 GB eMMC / 128 MB SPI NAND
- USB-C cable that delivers data **and** ≥ 12 V PD power. The board's
  USB-C port doubles as console serial (CP2102N bridge) on most revisions.
  ⚠️ Mac Studio's USB-C ports negotiate PD reliably; small phone chargers
  may be 5 V only and brown out under WiFi load.
- Cat5e or better Ethernet cable
- Mac (or Linux) workstation with a serial terminal (`tio`, `screen`, …)

### Software (host machine)

- Rust toolchain: `rustup target add aarch64-unknown-linux-musl`
- Cross-link via `zig`: `brew install zig && cargo install cargo-zigbuild`
- `dtc`, `u-boot-tools`: `brew install dtc u-boot-tools`
- A serial terminal: `brew install tio`

---

## 2. Hardware switch & first contact

The BPI-R3 Mini has a small slide switch on the PCB labelled
**NAND / eMMC**. Out of the factory it's on **NAND** and OpenWrt 21.02
factory boots from the SPI NAND. Keep it there for now.

1. Connect the USB-C to the Mac. The board powers on (red PWR LED).
2. Identify the serial port:
   ```sh
   ls /dev/cu.usbserial-*
   ```
3. Open the console at **115200 8N1**:
   ```sh
   tio /dev/cu.usbserial-XXXX
   ```
4. Press Enter → factory OpenWrt prompt: `root@OpenWrt:~#`. No password.

> ⚠️ If you see a reboot loop after T+30s with `EDCCAInit() ... compensation`
> messages, your USB-C source is under-powered. Use a 12 V PD charger or a
> dock with PD passthrough.

---

## 3. Flash OpenWrt 25.12.2 to the eMMC

We keep the NAND untouched (recovery) and install on eMMC. The hardware
switch will let us choose which one to boot.

### 3.1 Download the artefacts on the Mac

```sh
mkdir -p ~/openwrt-r3mini && cd ~/openwrt-r3mini

URL=https://downloads.openwrt.org/releases/25.12.2/targets/mediatek/filogic
PREFIX=openwrt-25.12.2-mediatek-filogic-bananapi_bpi-r3-mini

curl -O ${URL}/${PREFIX}-emmc-preloader.bin
curl -O ${URL}/${PREFIX}-emmc-bl31-uboot.fip
curl -O ${URL}/${PREFIX}-emmc-gpt.bin
curl -O ${URL}/${PREFIX}-squashfs-sysupgrade.itb

ls -la
```

### 3.2 Bring the factory OpenWrt online (Internet via Mac sharing)

On macOS: System Settings → General → Sharing → Internet Sharing →
Share Wi-Fi to **Ethernet**. Plug an Ethernet cable from the Mac to the
**WAN** port of the board.

On the board:
```sh
ip -br addr show eth1   # should show 192.168.2.x from the Mac DHCP
ping -c 2 1.1.1.1
```

### 3.3 Serve the artefacts to the board

On the Mac:
```sh
cd ~/openwrt-r3mini
python3 -m http.server 8080
```

On the board:
```sh
cd /tmp
URL=http://192.168.2.10:8080
PREFIX=openwrt-25.12.2-mediatek-filogic-bananapi_bpi-r3-mini

for f in emmc-preloader.bin emmc-bl31-uboot.fip emmc-gpt.bin squashfs-sysupgrade.itb; do
  curl -O ${URL}/${PREFIX}-${f}
done
```

(Replace `192.168.2.10` with whatever IP the Mac uses on its sharing
interface — `arp -a` on the Mac shows it.)

### 3.4 Stop OpenWrt's auto-mounter

Block-mount can re-grab eMMC partitions while we re-write the GPT. Stop it:
```sh
/etc/init.d/fstab stop 2>/dev/null
/etc/init.d/block stop 2>/dev/null
```

### 3.5 Write GPT, BL2, FIP, sysupgrade

```sh
PREFIX=openwrt-25.12.2-mediatek-filogic-bananapi_bpi-r3-mini

# 1. GPT layout
dd if=/tmp/${PREFIX}-emmc-gpt.bin of=/dev/mmcblk0 bs=512 conv=fsync
partprobe /dev/mmcblk0 2>/dev/null || blockdev --rereadpt /dev/mmcblk0

# 2. BL2 in the eMMC HW boot partition (force_ro=0 first!)
echo 0 > /sys/block/mmcblk0boot0/force_ro
dd if=/tmp/${PREFIX}-emmc-preloader.bin of=/dev/mmcblk0boot0 bs=512 conv=fsync

# 3. FIP — find the partition by GPT label (sysfs)
for p in /sys/block/mmcblk0/mmcblk0p*; do
  name=$(grep PARTNAME $p/uevent | cut -d= -f2)
  case "$name" in
    fip)         FIP=/dev/$(basename $p) ;;
    production)  PROD=/dev/$(basename $p) ;;
  esac
done
echo "FIP=$FIP  PROD=$PROD"

dd if=/tmp/${PREFIX}-emmc-bl31-uboot.fip of=$FIP bs=512 conv=fsync

# 4. Kernel + squashfs (the sysupgrade ITB)
dd if=/tmp/${PREFIX}-squashfs-sysupgrade.itb of=$PROD bs=512 conv=fsync

# 5. Tell the BootROM to boot mmcblk0boot0
mmc bootpart enable 1 1 /dev/mmcblk0
sync
```

### 3.6 Boot from eMMC

```sh
poweroff
```

When the system halts:
1. **Unplug the USB-C** (cuts power)
2. **Slide the hardware switch from NAND to eMMC**
3. **Plug the USB-C back**

The board boots BL2 (OpenWrt's preloader) → FIP (OpenWrt's U-Boot) →
kernel 6.x → procd → login prompt:

```
root@OpenWrt:/# 
```

Set a strong root password right away:
```sh
passwd
```

---

## 4. Configure OpenWrt for DidiCafe

This is all UCI. Each `uci commit` writes to `/etc/config/*`, which is
in the f2fs `/overlay` (persistent across reboots).

### 4.1 LAN on 10.10.0.1/24

```sh
uci set network.lan.ipaddr='10.10.0.1'
uci set network.lan.netmask='255.255.255.0'
uci -q delete network.lan.ip6assign     # IPv4-only LAN for now
uci commit network
service network reload
```

### 4.2 WiFi AP, dual-band, SSID DidiCafe

```sh
# 2.4 GHz radio
uci set wireless.radio0.disabled='0'
uci set wireless.radio0.country='MG'
uci -q delete wireless.default_radio0.disabled
uci set wireless.default_radio0.ssid='DidiCafe'
uci set wireless.default_radio0.encryption='none'

# 5 GHz radio
uci set wireless.radio1.disabled='0'
uci set wireless.radio1.country='MG'
uci -q delete wireless.default_radio1.disabled
uci set wireless.default_radio1.ssid='DidiCafe'
uci set wireless.default_radio1.encryption='none'

uci commit wireless
wifi reload

iw dev | grep -E "Interface|ssid"   # must show phy0-ap0 and phy1-ap0
```

### 4.3 DHCP/DNS — option 114 + local domain

```sh
# RFC 8910 — advertise the CAPPORT API URL
uci -q delete dhcp.lan.dhcp_option
uci add_list dhcp.lan.dhcp_option='114,http://didicafe.local:8080/api/captive'

# IPv4-only DHCP to keep things simple at first
uci set dhcp.lan.dhcpv6='disabled'
uci set dhcp.lan.ra='disabled'

# Resolve didicafe.local + admin.didicafe.local locally
uci add dhcp domain
uci set dhcp.@domain[-1].name='didicafe.local'
uci set dhcp.@domain[-1].ip='10.10.0.1'

uci add dhcp domain
uci set dhcp.@domain[-1].name='admin.didicafe.local'
uci set dhcp.@domain[-1].ip='10.10.0.1'

uci commit dhcp
service dnsmasq reload
```

### 4.4 Free up port 443 (uhttpd uses it for LuCI by default)

For a production captive-portal box you don't need LuCI:
```sh
killall -9 uhttpd 2>/dev/null
/etc/init.d/uhttpd disable
```

(Or move LuCI to another port: `uci set uhttpd.main.listen_https='0.0.0.0:4443'`.)

---

## 5. Cross-compile and deploy didicafe

### 5.1 Build the binary on the Mac

```sh
cd ~/Github/didicafe
rustup target add aarch64-unknown-linux-musl
brew install zig
cargo install cargo-zigbuild

cargo zigbuild --release --target aarch64-unknown-linux-musl
file target/aarch64-unknown-linux-musl/release/didicafe
# → ELF 64-bit LSB executable, ARM aarch64, statically linked, stripped
```

### 5.2 Push it to the board

```sh
BOARD=192.168.2.23      # adjust to the WAN IP from the Mac sharing

ssh root@${BOARD} "mkdir -p /usr/local/bin /etc/didicafe /opt/didicafe /var/lib/didicafe /opt/didicafe/certs"

scp -O target/aarch64-unknown-linux-musl/release/didicafe root@${BOARD}:/usr/local/bin/
scp -O config/didicafe.toml root@${BOARD}:/etc/didicafe/
scp -O -r static templates migrations root@${BOARD}:/opt/didicafe/

ssh root@${BOARD} "chmod +x /usr/local/bin/didicafe && ln -sf /opt/didicafe/static /usr/local/bin/static"
```

> ⚠️ `-O` is required: dropbear has no `sftp-server` so plain `scp` fails.
> Modern OpenSSH defaults to SFTP — `-O` forces the legacy scp protocol.

### 5.3 Generate TLS certs for the admin (HTTPS)

On the Mac:
```sh
cd ~/Github/didicafe
./scripts/gen-certs.sh certs didicafe.local admin.didicafe.local 10.10.0.1
```

Push the server cert + key to the board:
```sh
scp -O certs/server.pem certs/server-key.pem root@${BOARD}:/opt/didicafe/certs/
ssh root@${BOARD} "chmod 600 /opt/didicafe/certs/server-key.pem && chmod 644 /opt/didicafe/certs/server.pem"
```

Keep `certs/ca.pem` on the Mac — install it once on the manager's
phone/laptop so the admin HTTPS doesn't show a cert warning. See the
header of `scripts/gen-certs.sh` for per-OS instructions.

### 5.4 Adapt `/etc/didicafe/didicafe.toml`

The shipped TOML has Alpine paths and weak defaults. Edit on the board:

```sh
vi /etc/didicafe/didicafe.toml
```

Required values:
```toml
[server]
listen = "10.10.0.1"
port = 8080
interfaces = "br-lan"

[database]
path = "/var/lib/didicafe/didicafe.db"

[admin]
username = "admin"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$...REPLACE_ME..."

[portal]
domain = "didicafe.local"
admin_domain = "admin.didicafe.local"
cafe_name = "DidiCafe"
theme_color = "#b45309"

[tls]
enabled = true
cert_path = "/opt/didicafe/certs/server.pem"
key_path = "/opt/didicafe/certs/server-key.pem"
admin_port = 443

[firewall]
nft_path = "/usr/sbin/nft"
table_name = "didicafe"
set_name = "auth_clients"
```

Generate a real Argon2id hash on the Mac (don't ship the default one,
the binary refuses to start with it):
```sh
python3 -c "from passlib.hash import argon2; print(argon2.using(memory_cost=19456, time_cost=2, parallelism=1, type='ID').hash('YOUR_STRONG_PASSWORD'))"
```

Paste it into `password_hash` on the board.

### 5.5 nftables fragment for the captive flow

didicafe creates the table `inet didicafe` with the `auth_clients` set
on startup, but the DNAT/forward chains are ours. Drop them in a file
loaded by the init script every boot:

```sh
cat > /etc/nftables.didicafe.nft <<'EOF'
#!/usr/sbin/nft -f

# Idempotent: if the table already exists (from a previous didicafe run),
# nuke and recreate so chains stay in sync with this file.
table inet didicafe { }
delete table inet didicafe

table inet didicafe {
    set auth_clients {
        type ether_addr . ipv4_addr
        flags timeout
    }

    chain prerouting {
        type nat hook prerouting priority dstnat - 5; policy accept;

        iifname "br-lan" ether saddr . ip saddr @auth_clients accept
        iifname "br-lan" udp dport { 67, 68 } accept
        iifname "br-lan" udp dport 53 dnat ip to 10.10.0.1:53
        iifname "br-lan" tcp dport 53 dnat ip to 10.10.0.1:53
        iifname "br-lan" tcp dport 80 dnat ip to 10.10.0.1:8080
    }

    chain forward {
        type filter hook forward priority filter - 5; policy accept;
        iifname "br-lan" oifname "eth1" ether saddr . ip saddr @auth_clients accept
        iifname "br-lan" oifname "eth1" drop
    }
}
EOF
```

### 5.6 procd init script

```sh
cat > /etc/init.d/didicafe <<'EOF'
#!/bin/sh /etc/rc.common

USE_PROCD=1
START=95
STOP=10

start_service() {
    [ -d /var/lib/didicafe ] || mkdir -p /var/lib/didicafe

    # Apply the captive-portal nftables fragment before starting didicafe.
    [ -f /etc/nftables.didicafe.nft ] && /usr/sbin/nft -f /etc/nftables.didicafe.nft

    procd_open_instance
    procd_set_param command /usr/local/bin/didicafe --config /etc/didicafe/didicafe.toml
    procd_set_param respawn 3600 5 0
    procd_set_param stdout 1
    procd_set_param stderr 1
    procd_set_param env RUST_LOG=didicafe=info
    procd_close_instance
}
EOF
chmod +x /etc/init.d/didicafe
/etc/init.d/didicafe enable
/etc/init.d/didicafe start
```

Verify:
```sh
service didicafe running && echo OK
netstat -tlnp 2>/dev/null | grep -E ":443|:8080"
nft list table inet didicafe | head
logread -e didicafe | tail
```

You should see two listeners on `10.10.0.1` (`8080` HTTP portal, `443`
HTTPS admin) and the nftables table populated.

---

## 6. End-to-end test

1. On a phone, connect to the WiFi `DidiCafe`.
2. The captive sheet pops up automatically (Apple/Android CPD probe is
   DNATed to didicafe).
3. To bootstrap as the manager, dismiss the captive sheet ("Use without
   internet"), open Chrome/Firefox, go to **`https://admin.didicafe.local`**
   → admin login.
4. Login with `admin` + your password. didicafe authorizes your MAC for 7
   days, you now have internet **and** the admin GUI.
5. From the admin GUI: create a plan, generate tokens, hand a token to a
   customer.
6. The customer types the token in the captive page → didicafe validates
   it → the MAC is added to `auth_clients` for the plan duration → the
   customer browses freely.

---

## 7. Recovery

If the eMMC firmware breaks or you need to start over:

1. **Slide the hardware switch back to NAND.**
2. Power-cycle. The factory OpenWrt 21.02 boots and you have a working
   shell again.
3. From there you can re-flash the eMMC (re-do §3) or repair via dd.

The eMMC and NAND are physically separate; nothing you do on one side
can corrupt the other.

---

## 8. Hardening before going live

Before handing the box to a real café, durcir :

- [ ] Replace the temporary `Allow-SSH-WAN-temp` firewall rule with
      LAN-only SSH (or remove SSH from WAN entirely)
- [ ] Set `tls.enabled = true` and confirm the cert is valid
- [ ] Install `certs/ca.pem` on the manager's device(s)
- [ ] Set `admin.allowed_networks = ["10.10.0.0/24"]` in the TOML
- [ ] Enable `dropbear` key-based auth, disable password SSH
- [ ] Set up daily `sqlite3 .backup` of `/var/lib/didicafe/didicafe.db`
- [ ] Check `/etc/sysupgrade.conf` includes our config files so OpenWrt
      sysupgrades don't wipe them (`/etc/didicafe/`, `/opt/didicafe/`,
      `/etc/nftables.didicafe.nft`, `/etc/init.d/didicafe`,
      `/var/lib/didicafe/`)
