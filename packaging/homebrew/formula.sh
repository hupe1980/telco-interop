#!/usr/bin/env bash
# Render the Homebrew formula for a release from its SHA256SUMS.
#
#   packaging/homebrew/formula.sh 0.1.0 dist/SHA256SUMS > Formula/telco-interop.rb
set -euo pipefail
version="${1:?usage: formula.sh <version> <SHA256SUMS>}"
sums="${2:?usage: formula.sh <version> <SHA256SUMS>}"
template="$(cd "$(dirname "$0")" && pwd)/telco-interop.rb.in"
out="$(sed "s/@VERSION@/$version/g" "$template")"
for target in aarch64-apple-darwin aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu; do
  archive="telco-interop-v$version-$target.tar.gz"
  sum="$(awk -v a="$archive" '$2 == a || $2 == "*" a { print $1 }' "$sums")"
  if ! [[ "$sum" =~ ^[0-9a-f]{64}$ ]]; then
    echo "formula: no checksum for $archive in $sums" >&2
    exit 1
  fi
  out="${out//@SHA256_$target@/$sum}"
done
if grep -q '@[A-Z0-9_a-z-]*@' <<<"$out"; then
  echo "formula: a placeholder was left" >&2
  exit 1
fi
printf '%s\n' "$out"
