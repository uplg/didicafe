#!/bin/sh
# Generate a local CA and server certificate for DidiCafe admin HTTPS.
#
# Usage:
#   ./scripts/gen-certs.sh [output_dir] [domain] [admin_domain] [ip]
#
# Defaults:
#   output_dir   = ./certs
#   domain       = wifi.didicafe
#   admin_domain = admin.wifi.didicafe
#   ip           = 10.10.0.1
#
# After generation, install ca.pem on the manager's phone/PC once:
#   - Android: Settings > Security > Encryption > Install from storage
#   - iOS: AirDrop or email ca.pem, then Settings > General > VPN & Device Management
#   - Windows: Double-click ca.pem > Install Certificate > Local Machine > Trusted Root
#   - Linux: cp ca.pem /usr/local/share/ca-certificates/didicafe-ca.crt && update-ca-certificates
#
# The server certificate is valid for 2 years. The CA certificate is valid for 10 years.

set -eu

CERT_DIR="${1:-./certs}"
DOMAIN="${2:-wifi.didicafe}"
ADMIN_DOMAIN="${3:-admin.wifi.didicafe}"
IP="${4:-10.10.0.1}"

CA_KEY="${CERT_DIR}/ca-key.pem"
CA_CERT="${CERT_DIR}/ca.pem"
SERVER_KEY="${CERT_DIR}/server-key.pem"
SERVER_CERT="${CERT_DIR}/server.pem"
SERVER_CSR="${CERT_DIR}/server.csr"

mkdir -p "${CERT_DIR}"

echo "=== DidiCafe TLS Certificate Generator ==="
echo "Output:       ${CERT_DIR}"
echo "Domain:       ${DOMAIN}"
echo "Admin domain: ${ADMIN_DOMAIN}"
echo "IP:           ${IP}"
echo ""

# --- CA (10 years) ---
if [ ! -f "${CA_KEY}" ]; then
    echo "[1/4] Generating CA private key..."
    openssl genrsa -out "${CA_KEY}" 4096

    echo "[2/4] Generating CA certificate (10 years)..."
    openssl req -new -x509 -key "${CA_KEY}" -out "${CA_CERT}" \
        -days 3650 \
        -subj "/CN=DidiCafe Local CA/O=DidiCafe/C=MG"
else
    echo "[1/4] CA key already exists, skipping..."
    echo "[2/4] CA cert already exists, skipping..."
fi

# --- Server certificate (2 years) ---
echo "[3/4] Generating server private key and CSR..."
openssl genrsa -out "${SERVER_KEY}" 2048

openssl req -new -key "${SERVER_KEY}" -out "${SERVER_CSR}" \
    -subj "/CN=${DOMAIN}/O=DidiCafe/C=MG"

echo "[4/4] Signing server certificate with CA (2 years)..."
# Create temporary SAN config
SAN_CONF=$(mktemp)
cat > "${SAN_CONF}" <<EOF
[v3_req]
subjectAltName = DNS:${DOMAIN}, DNS:${ADMIN_DOMAIN}, IP:${IP}
basicConstraints = CA:FALSE
keyUsage = digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
EOF

openssl x509 -req -in "${SERVER_CSR}" -CA "${CA_CERT}" -CAkey "${CA_KEY}" \
    -CAcreateserial -out "${SERVER_CERT}" \
    -days 730 \
    -extfile "${SAN_CONF}" -extensions v3_req

# Cleanup
rm -f "${SERVER_CSR}" "${SAN_CONF}" "${CERT_DIR}/ca.srl"

# Set restrictive permissions
chmod 600 "${CA_KEY}" "${SERVER_KEY}"
chmod 644 "${CA_CERT}" "${SERVER_CERT}"

echo ""
echo "=== Done ==="
echo "CA certificate (install on devices): ${CA_CERT}"
echo "Server certificate:                  ${SERVER_CERT}"
echo "Server private key:                  ${SERVER_KEY}"
echo ""
echo "Add to didicafe.toml:"
echo "  [tls]"
echo "  enabled = true"
echo "  cert_path = \"${SERVER_CERT}\""
echo "  key_path = \"${SERVER_KEY}\""
echo "  admin_port = 443"
