#!/usr/bin/env bash
# Published files must not carry planning ids or cite the local, unpublished trees.
# Allowed: `<feature>/SR-###` evidence markers in tests (AGENTS.md).
set -euo pipefail
cd "$(dirname "$0")/.."
published=(crates fuzz/fuzz_targets site/content README.md AGENTS.md CHANGELOG.md SECURITY.md)
fail=0
if grep -rnE '\b(FR|SC|SR)-[0-9]{3}\b|\bT0[0-9]{2}\b' "${published[@]}" | grep -vE '[a-z-]+/SR-[0-9]{3}'; then
  echo "error: planning ids in published files (use <feature>/SR-### markers in tests only)"; fail=1
fi
if grep -rnE '(^|[^a-z])(concepts|specs|\.specify)/' "${published[@]}" --include='*.rs' --include='*.md' --include='*.toml'; then
  echo "error: published files cite unpublished paths (concepts/, specs/, .specify/)"; fail=1
fi
exit "$fail"
