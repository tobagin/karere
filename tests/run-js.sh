#!/usr/bin/env bash
# Run every tests/*.test.js with node (built-ins only). Exits non-zero on failure.
set -u
cd "$(dirname "$0")/.."
command -v node >/dev/null 2>&1 || { echo "node not found" >&2; exit 2; }
fail=0
for t in tests/*.test.js; do
  echo "== $t"
  if ! node "$t"; then
    echo "FAILED: $t" >&2
    fail=1
  fi
done
exit $fail
