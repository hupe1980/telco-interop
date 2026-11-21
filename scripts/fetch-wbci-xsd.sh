#!/usr/bin/env bash
# Fetch the AK S&P's WBCI XML schema (Anlage 4 V2.00) for the conformance test.
#
# The schema is copyrighted and never committed: it is downloaded here, accepted only if its
# SHA-256 equals the pin, and only the known entries are written, by name, into DIR.
#
# Usage: scripts/fetch-wbci-xsd.sh [DIR]   (default: target/wbci-xsd)
# Env:   WBCI_XSD_ZIP=<file>  use a local zip instead of downloading (still hash-checked)
# Exit:  0 ok · 1 download or tool failure · 2 hash mismatch (schema changed or tampered)
set -euo pipefail

URL="https://ak-spri.de/wp-content/uploads/2017/11/Anl4_XML_Schema_WBCI_V2.00.zip"
SHA256="711dbe23bbb5683b460e1fa63eab0f9510ced34b8a7ca7457eff7dd04a4662b6"
PREFIX="Anl4 XML Schema_WBCI_V2.00"
ENTRIES=(
  wsdl/wbci-envelope.xsd wsdl/wbci.wsdl wsdl/policy.xml
  xsd/wbci-message.xsd xsd/wbci-request.xsd xsd/wbci-type-complex.xsd
  xsd/wbci-type-enum.xsd xsd/wbci-type-simple.xsd
)

cd "$(dirname "$0")/.."
DIR="${1:-target/wbci-xsd}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [[ -n "${WBCI_XSD_ZIP:-}" ]]; then
  cp "$WBCI_XSD_ZIP" "$tmp/schema.zip"
else
  curl --fail --silent --show-error --location --max-filesize 1048576 --output "$tmp/schema.zip" "$URL" \
    || { echo "error: could not download $URL" >&2; exit 1; }
fi

if command -v sha256sum >/dev/null; then
  actual="$(sha256sum "$tmp/schema.zip" | cut -d' ' -f1)"
else
  actual="$(shasum -a 256 "$tmp/schema.zip" | cut -d' ' -f1)"
fi
if [[ "$actual" != "$SHA256" ]]; then
  echo "error: the WBCI schema changed or was tampered with" >&2
  echo "  expected sha256 $SHA256" >&2
  echo "  actual   sha256 $actual" >&2
  echo "  Review the new schema from $URL, then update the pin in $0." >&2
  exit 2
fi

# Only known entries, by name: no path from the archive decides where a file goes.
rm -rf "$DIR"
mkdir -p "$DIR/wsdl" "$DIR/xsd"
for entry in "${ENTRIES[@]}"; do
  unzip -p "$tmp/schema.zip" "$PREFIX/$entry" > "$DIR/$entry"
done
cp "$tmp/schema.zip" "$DIR/schema.zip"
echo "WBCI schema V2.00 verified (sha256 $SHA256) → $DIR"
