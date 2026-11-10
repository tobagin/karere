#!/usr/bin/env bash
# Run every offline check before a release (T4). One pass/fail line each;
# exits non-zero if any fail. Output of a failing check follows its line.
#
# Not run here: tests/coordinate_probe.sh and tests/wayland_fractional_verify.sh are
# interactive harnesses that need a display; both reference
# tests/fixtures/coord-probe-whatsapp.html, which is not in the tree (rows SKIP).
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

CHECKS=(
    tests/cef_version_refs.sh
    tests/version_refs.sh
    tests/manifest_drift.sh
    tests/flathub_beta_cef_policy.sh
    tests/test-build-sh.sh
    tools/verify-po.sh
)
[[ -f tests/run-js.sh ]] && CHECKS+=(tests/run-js.sh)

FAILED=0
for c in "${CHECKS[@]}"; do
    if [[ ! -f "$c" ]]; then
        echo "FAIL  $c (missing)"
        FAILED=$((FAILED + 1))
        continue
    fi
    if out="$(bash "$c" 2>&1)"; then
        echo "PASS  $c"
    else
        echo "FAIL  $c"
        { echo "$out" | grep -iE "fail" || echo "$out" | tail -n 15; } | head -n 15 | sed 's/^/        /'
        FAILED=$((FAILED + 1))
    fi
done

echo "---"
if [[ "$FAILED" -ne 0 ]]; then
    echo "$FAILED of ${#CHECKS[@]} offline checks failed"
    exit 1
fi
echo "All ${#CHECKS[@]} offline checks passed"
