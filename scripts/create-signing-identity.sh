#!/usr/bin/env bash
# Create a local, self-signed code-signing identity (free, no Apple
# account). Why bother instead of ad-hoc signing: macOS ties the
# Accessibility grant to the code signature, and ad-hoc signatures change
# on every rebuild — so the permission is lost after each update. A stable
# self-signed identity keeps the grant across rebuilds.
#
# Usage:  ./scripts/create-signing-identity.sh
# Then:   export APPLE_SIGNING_IDENTITY="EspressoMacchiato Local Signing"
#         npm run tauri build
set -euo pipefail

NAME="EspressoMacchiato Local Signing"
WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

if security find-identity -v -p codesigning | grep -q "$NAME"; then
  echo "Identity already present: $NAME"
  exit 0
fi

cat > "$WORKDIR/cert.cnf" <<EOF
[ req ]
distinguished_name = dn
x509_extensions = v3
prompt = no
[ dn ]
CN = $NAME
O = EspressoMacchiato
[ v3 ]
basicConstraints = critical,CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = critical,codeSigning
# Apple's "Code Signing" certificate extension marker.
1.2.840.113635.100.6.1.14 = critical,DER:0500
EOF

openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
  -keyout "$WORKDIR/key.pem" -out "$WORKDIR/cert.pem" -config "$WORKDIR/cert.cnf"

# -legacy: macOS `security` cannot read AES-encrypted PKCS#12 from
# OpenSSL 3.
openssl pkcs12 -export -legacy -out "$WORKDIR/identity.p12" \
  -inkey "$WORKDIR/key.pem" -in "$WORKDIR/cert.pem" \
  -passout pass:espresso -name "$NAME"

security import "$WORKDIR/identity.p12" -k ~/Library/Keychains/login.keychain-db \
  -P espresso -T /usr/bin/codesign -T /usr/bin/security
security add-trusted-cert -r trustRoot -p codeSign \
  -k ~/Library/Keychains/login.keychain-db "$WORKDIR/cert.pem"

security find-identity -v -p codesigning | grep "$NAME"
echo
echo "Done. Build with:"
echo "  export APPLE_SIGNING_IDENTITY=\"$NAME\""
echo "  npm run tauri build"
