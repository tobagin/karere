#!/usr/bin/env bash
# Regression test for build.sh --dev dispatch (KARE-007).
# Mocks flatpak-builder and python3 so no real build or source-regen runs.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PASS=0
FAIL=0

pass() {
    PASS=$((PASS + 1))
    echo "PASS: $1"
}

fail() {
    FAIL=$((FAIL + 1))
    echo "FAIL: $1" >&2
}

# Create isolated mock bin + log files
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT
MOCK_BIN="$TMPDIR/mock-bin"
MOCK_LOG="$TMPDIR/flatpak-builder.log"
PY_MARKER="$TMPDIR/python3-called"
mkdir -p "$MOCK_BIN"

cat >"$MOCK_BIN/flatpak-builder" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >> "$MOCK_LOG"
exit 0
EOF
chmod +x "$MOCK_BIN/flatpak-builder"

cat >"$MOCK_BIN/python3" <<EOF
#!/usr/bin/env bash
touch "$PY_MARKER"
exit 0
EOF
chmod +x "$MOCK_BIN/python3"

export PATH="$MOCK_BIN:$PATH"
export MOCK_LOG
export PY_MARKER

reset_state() {
    : > "$MOCK_LOG"
    rm -f "$PY_MARKER"
}

# Helper to run build.sh and capture outputs
run_build() {
    # usage: run_build [args...]
    # sets globals: RUN_EXIT, RUN_STDOUT, RUN_STDERR, RUN_COMBINED
    local stdout_file="$TMPDIR/stdout"
    local stderr_file="$TMPDIR/stderr"
    set +e
    PATH="$MOCK_BIN:$PATH" bash "$REPO_ROOT/build.sh" "$@" >"$stdout_file" 2>"$stderr_file"
    RUN_EXIT=$?
    set -e
    RUN_STDOUT="$(cat "$stdout_file")"
    RUN_STDERR="$(cat "$stderr_file")"
}

# --- Case 1: symptom-gone — ./build.sh --dev ---
reset_state
run_build --dev
if [[ "$RUN_EXIT" -ne 0 ]]; then
    fail "case 1 --dev exit code expected 0 got $RUN_EXIT (stdout: $RUN_STDOUT stderr: $RUN_STDERR)"
else
    pass "case 1 --dev exits 0"
fi
if echo "$RUN_STDERR" | grep -q "Unknown option"; then
    fail "case 1 --dev stderr must not contain Unknown option (got: $RUN_STDERR)"
else
    pass "case 1 --dev no Unknown option on stderr"
fi
EXPECTED="--force-clean --user --install build-dir packaging/io.github.tobagin.karere.Devel.yml"
LAST_LOG="$(tail -n 1 "$MOCK_LOG" 2>/dev/null || true)"
if [[ "$LAST_LOG" != "$EXPECTED" ]]; then
    fail "case 1 --dev mock log expected '$EXPECTED' got '$LAST_LOG'"
else
    pass "case 1 --dev dispatches Devel manifest with correct args"
fi
if echo "$RUN_STDOUT" | tail -n 5 | grep -q "Run with: flatpak run io.github.tobagin.karere.Devel"; then
    pass "case 1 --dev stdout ends with Devel run hint"
else
    fail "case 1 --dev stdout should contain 'Run with: flatpak run io.github.tobagin.karere.Devel' (got: $RUN_STDOUT)"
fi

# --- Case 2: default ./build.sh ---
reset_state
run_build
if [[ "$RUN_EXIT" -ne 0 ]]; then
    fail "case 2 default exit code expected 0 got $RUN_EXIT"
else
    pass "case 2 default exits 0"
fi
EXPECTED2="--force-clean --user --install build-dir packaging/io.github.tobagin.karere.yml"
LAST_LOG="$(tail -n 1 "$MOCK_LOG" 2>/dev/null || true)"
if [[ "$LAST_LOG" != "$EXPECTED2" ]]; then
    fail "case 2 default mock log expected '$EXPECTED2' got '$LAST_LOG'"
else
    pass "case 2 default dispatches production manifest"
fi
if echo "$RUN_STDOUT" | grep -q "Run with: flatpak run io.github.tobagin.karere$"; then
    pass "case 2 default stdout has production run hint"
else
    fail "case 2 default stdout should contain production run hint (got: $RUN_STDOUT)"
fi

# --- Case 3: --dev --no-install omits --install ---
reset_state
run_build --dev --no-install
if [[ "$RUN_EXIT" -ne 0 ]]; then
    fail "case 3 --dev --no-install exit code expected 0 got $RUN_EXIT"
else
    pass "case 3 --dev --no-install exits 0"
fi
LAST_LOG="$(tail -n 1 "$MOCK_LOG" 2>/dev/null || true)"
if echo "$LAST_LOG" | grep -q -- "--install"; then
    fail "case 3 --dev --no-install should omit --install (got: $LAST_LOG)"
else
    pass "case 3 --dev --no-install omits --install"
fi
EXPECTED3="--force-clean --user build-dir packaging/io.github.tobagin.karere.Devel.yml"
if [[ "$LAST_LOG" != "$EXPECTED3" ]]; then
    fail "case 3 --dev --no-install mock log expected '$EXPECTED3' got '$LAST_LOG'"
else
    pass "case 3 --dev --no-install exact args"
fi
if echo "$RUN_STDOUT" | grep -q "Run with:"; then
    fail "case 3 --dev --no-install should not print Run with (got: $RUN_STDOUT)"
else
    pass "case 3 --dev --no-install no Run with hint"
fi

# --- Case 4: --bogus guard still live ---
reset_state
run_build --bogus
if [[ "$RUN_EXIT" -ne 1 ]]; then
    fail "case 4 --bogus exit code expected 1 got $RUN_EXIT"
else
    pass "case 4 --bogus exits 1"
fi
if echo "$RUN_STDERR" | grep -q "Unknown option: --bogus"; then
    pass "case 4 --bogus stderr contains Unknown option: --bogus"
else
    fail "case 4 --bogus stderr should contain 'Unknown option: --bogus' (got: $RUN_STDERR)"
fi
if [[ -s "$MOCK_LOG" ]]; then
    fail "case 4 --bogus should not invoke flatpak-builder (log: $(cat "$MOCK_LOG"))"
else
    pass "case 4 --bogus did not invoke flatpak-builder"
fi

# --- Case 5: --help ---
reset_state
run_build --help
if [[ "$RUN_EXIT" -ne 0 ]]; then
    fail "case 5 --help exit code expected 0 got $RUN_EXIT"
else
    pass "case 5 --help exits 0"
fi
if echo "$RUN_STDOUT" | grep -q -- "--dev"; then
    pass "case 5 --help mentions --dev"
else
    fail "case 5 --help should mention --dev (got: $RUN_STDOUT)"
fi
if echo "$RUN_STDOUT" | grep -q "FLATPAK_BUILDER_EXTRA_ARGS"; then
    pass "case 5 --help mentions FLATPAK_BUILDER_EXTRA_ARGS"
else
    fail "case 5 --help should mention FLATPAK_BUILDER_EXTRA_ARGS (got: $RUN_STDOUT)"
fi
if [[ -s "$MOCK_LOG" ]]; then
    fail "case 5 --help should not invoke flatpak-builder"
else
    pass "case 5 --help did not invoke flatpak-builder"
fi

# --- Case 6: --dev --regen-sources invokes python3 mock and still dispatches ---
reset_state
run_build --dev --regen-sources
if [[ "$RUN_EXIT" -ne 0 ]]; then
    fail "case 6 --dev --regen-sources exit code expected 0 got $RUN_EXIT"
else
    pass "case 6 --dev --regen-sources exits 0"
fi
if [[ -f "$PY_MARKER" ]]; then
    pass "case 6 --dev --regen-sources invoked python3 mock"
else
    fail "case 6 --dev --regen-sources should invoke python3 mock (marker missing)"
fi
if echo "$RUN_STDOUT" | grep -q "Regenerating.*Devel"; then
    pass "case 6 regen line names Devel manifest"
else
    fail "case 6 regen line should name Devel manifest (got: $RUN_STDOUT)"
fi
if [[ -s "$MOCK_LOG" ]]; then
    pass "case 6 still dispatched flatpak-builder"
else
    fail "case 6 should still dispatch flatpak-builder after regen"
fi
LAST_LOG="$(tail -n 1 "$MOCK_LOG" 2>/dev/null || true)"
if [[ "$LAST_LOG" != "$EXPECTED" ]]; then
    fail "case 6 mock log expected '$EXPECTED' got '$LAST_LOG'"
else
    pass "case 6 mock log correct after regen"
fi

# --- Case 7: FLATPAK_BUILDER_EXTRA_ARGS passthrough ---
reset_state
set +e
PATH="$MOCK_BIN:$PATH" FLATPAK_BUILDER_EXTRA_ARGS="--disable-rofiles-fuse" bash "$REPO_ROOT/build.sh" --dev >"$TMPDIR/stdout" 2>"$TMPDIR/stderr"
RUN_EXIT=$?
set -e
LAST_LOG="$(tail -n 1 "$MOCK_LOG" 2>/dev/null || true)"
EXPECTED7="--force-clean --user --install --disable-rofiles-fuse build-dir packaging/io.github.tobagin.karere.Devel.yml"
if [[ "$RUN_EXIT" -ne 0 ]]; then
    fail "case 7a extra args exit code expected 0 got $RUN_EXIT"
else
    pass "case 7a FLATPAK_BUILDER_EXTRA_ARGS exits 0"
fi
if [[ "$LAST_LOG" != "$EXPECTED7" ]]; then
    fail "case 7a extra args mock log expected '$EXPECTED7' got '$LAST_LOG'"
else
    pass "case 7a extra flag appended"
fi

reset_state
set +e
PATH="$MOCK_BIN:$PATH" FLATPAK_BUILDER_EXTRA_ARGS="" bash "$REPO_ROOT/build.sh" --dev >"$TMPDIR/stdout" 2>"$TMPDIR/stderr"
RUN_EXIT=$?
set -e
LAST_LOG="$(tail -n 1 "$MOCK_LOG" 2>/dev/null || true)"
if [[ "$LAST_LOG" != "$EXPECTED" ]]; then
    fail "case 7b empty extra args should not change log expected '$EXPECTED' got '$LAST_LOG'"
else
    pass "case 7b empty FLATPAK_BUILDER_EXTRA_ARGS inert"
fi
# Check no empty-string arg: count words — should be 5 args before build-dir
WORD_COUNT="$(echo "$LAST_LOG" | wc -w)"
EXPECTED_WORDS="$(echo "$EXPECTED" | wc -w)"
if [[ "$WORD_COUNT" -ne "$EXPECTED_WORDS" ]]; then
    fail "case 7b word count mismatch empty extra added spurious arg (got $WORD_COUNT expected $EXPECTED_WORDS)"
else
    pass "case 7b no spurious empty arg"
fi

# --- Summary ---
echo "---"
echo "Results: $PASS passed, $FAIL failed"
if [[ "$FAIL" -ne 0 ]]; then
    exit 1
fi
exit 0
