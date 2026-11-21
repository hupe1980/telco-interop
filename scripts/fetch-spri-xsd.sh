#!/usr/bin/env bash
# Fetch the AK S&P's S/PRI XML schema (Anlage 3, schema 4.00.03 for S/PRI 4.x) for the
# conformance test.
#
# The schema is copyrighted and never committed: it is downloaded here, accepted only if its
# SHA-256 equals the pin, and only the known entries are written, by name, into DIR.
#
# Usage: scripts/fetch-spri-xsd.sh [DIR]   (default: target/spri-xsd)
# Env:   SPRI_XSD_ZIP=<file>  use a local zip instead of downloading (still hash-checked)
# Exit:  0 ok · 1 download or tool failure · 2 hash mismatch (schema changed or tampered)
set -euo pipefail

URL="https://ak-spri.de/wp-content/uploads/Anlage_3_XSD_SPRI_V.4.x.zip"
SHA256="e54d404771248a917b4df4a8df21f93e235a29b15dc771495851435b9b200f46"
# The archive has no top-level folder.
ENTRIES=(
  wsdl/oss-envelope.xsd wsdl/spri.wsdl wsdl/policy.xml
  xsd/oss-message.xsd xsd/oss-order.xsd xsd/oss-product-FTTx.xsd
  xsd/oss-type-complex.xsd xsd/oss-type-enum.xsd xsd/oss-type-simple.xsd
)

cd "$(dirname "$0")/.."
DIR="${1:-target/spri-xsd}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [[ -n "${SPRI_XSD_ZIP:-}" ]]; then
  cp "$SPRI_XSD_ZIP" "$tmp/schema.zip"
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
  echo "error: the S/PRI schema changed or was tampered with" >&2
  echo "  expected sha256 $SHA256" >&2
  echo "  actual   sha256 $actual" >&2
  echo "  Review the new schema from $URL, then update the pin in $0." >&2
  exit 2
fi

# Only known entries, by name: no path from the archive decides where a file goes.
rm -rf "$DIR"
mkdir -p "$DIR/wsdl" "$DIR/xsd"
for entry in "${ENTRIES[@]}"; do
  unzip -p "$tmp/schema.zip" "$entry" > "$DIR/$entry"
done
cp "$tmp/schema.zip" "$DIR/schema.zip"
echo "S/PRI schema 4.00.03 verified (sha256 $SHA256) → $DIR"
