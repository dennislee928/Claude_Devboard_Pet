#!/usr/bin/env bash
# make-selfsigned-cert.sh — create a self-signed code-signing identity and put
# it in a keychain so codesign/productsign can use it.
#
# A self-signed certificate does NOT make macOS trust the app automatically —
# it only gives the binaries a stable, verifiable publisher identity and a
# signature users (or an MDM) can choose to trust. Gatekeeper still asks on
# first launch unless the certificate is added to the login keychain as trusted.
#
#   ./make-selfsigned-cert.sh [--keychain PATH] [--password PW]
# Prints the identity name on success.
set -euo pipefail

CN="${DEVPET_CERT_CN:-DevPet Project}"
ORG="${DEVPET_CERT_ORG:-DevPet Project}"
KEYCHAIN="${DEVPET_KEYCHAIN:-$HOME/Library/Keychains/devpet-signing.keychain-db}"
PASSWORD="${DEVPET_KEYCHAIN_PASSWORD:-devpet}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

while [[ $# -gt 0 ]]; do
    case "$1" in
        --keychain) KEYCHAIN="${2:?}"; shift ;;
        --password) PASSWORD="${2:?}"; shift ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -keyout "$WORK/key.pem" -out "$WORK/cert.pem" \
    -subj "/CN=$CN/O=$ORG" \
    -addext "basicConstraints=critical,CA:false" \
    -addext "keyUsage=critical,digitalSignature" \
    -addext "extendedKeyUsage=critical,codeSigning" >/dev/null 2>&1

openssl pkcs12 -export -legacy -out "$WORK/devpet.p12" \
    -inkey "$WORK/key.pem" -in "$WORK/cert.pem" \
    -name "$CN" -passout "pass:$PASSWORD" 2>/dev/null ||
openssl pkcs12 -export -out "$WORK/devpet.p12" \
    -inkey "$WORK/key.pem" -in "$WORK/cert.pem" \
    -name "$CN" -passout "pass:$PASSWORD"

security delete-keychain "$KEYCHAIN" 2>/dev/null || true
security create-keychain -p "$PASSWORD" "$KEYCHAIN" >/dev/null
security set-keychain-settings -lut 21600 "$KEYCHAIN" >/dev/null
security unlock-keychain -p "$PASSWORD" "$KEYCHAIN" >/dev/null
security import "$WORK/devpet.p12" -k "$KEYCHAIN" -P "$PASSWORD" \
    -T /usr/bin/codesign -T /usr/bin/productsign -T /usr/bin/security >/dev/null
security set-key-partition-list -S apple-tool:,apple:,codesign:,productsign: \
    -s -k "$PASSWORD" "$KEYCHAIN" >/dev/null
# make it visible to codesign without disturbing the user's default keychain
security list-keychains -d user -s "$KEYCHAIN" $(security list-keychains -d user | tr -d '"') >/dev/null

# export the public certificate so users can inspect / trust it
CERT_OUT="${DEVPET_CERT_OUT:-$(cd "$(dirname "$0")/.." && pwd)/dist/DevPet-selfsigned.cer}"
mkdir -p "$(dirname "$CERT_OUT")"
cp "$WORK/cert.pem" "$CERT_OUT"

echo "$CN"
