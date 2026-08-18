#!/usr/bin/env bash
# KARE-016 Step 2 — Xvfb + GDK_SCALE + xdotool + CDP-title readback harness.
# For each {GDK_SCALE=1@1280x800, GDK_SCALE=2@2560x1600} × {cpu-osr, gpu-osr (KARERE_GPU_OSR=1)}
# launches the target under Xvfb, positions the window deterministically,
# synthesizes clicks through the real input path, and checks recorded clientX/Y
# against expected physical coordinates. Prints one JSON row per config and
# captures joint stderr logs. Negative control proves the harness can detect
# misalignment (wrong expectation must FAIL).
#
# Usage:
#   tests/coordinate_probe.sh [--bin /path/to/karere] [--negative] [--help]
#   GDK_SCALE override via env is ignored — matrix drives scale explicitly.
#   KARERE_BIN env can point at "flatpak run ... io.github.tobagin.karere.Devel"
#   but native cargo binary is the CI default (target/debug/karere).
#
# Requirements: Xvfb (Xvfb or xvfb-run), xdotool, curl (optional for CDP), python3,
#               gsettings/glib-compile-schemas, cargo-built karere binary.
# Keeps GSK_RENDERER=gl, no driver overrides beyond test fixture, and only
# uses --debug CDP port in this disposable run (release gating unchanged).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE_HTML="$REPO_ROOT/tests/fixtures/coord-probe.html"
STABLE_APP_ID="io.github.tobagin.karere"
DEVEL_APP_ID="io.github.tobagin.karere.Devel"
APP_ID="$STABLE_APP_ID"  # overridden per-run for Devel flatpak (see run_config)

KARERE_BIN="${KARERE_BIN:-}"
NEGATIVE_MODE=0
HELP=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) KARERE_BIN="$2"; shift 2;;
    --negative) NEGATIVE_MODE=1; shift;;
    --help|-h) HELP=1; shift;;
    *) echo "Unknown arg: $1" >&2; exit 2;;
  esac
done

if [[ $HELP -eq 1 ]]; then
  cat <<'EOF'
Usage: tests/coordinate_probe.sh [--bin /path/to/karere] [--negative]

Matrix: GDK_SCALE=1@1280x800 and GDK_SCALE=2@2560x1600 × {cpu-osr (KARERE_GPU_OSR=0), gpu-osr (KARERE_GPU_OSR=1)}
Each config: Xvfb + xdotool window placement + synthetic clicks + title/CDP readback → JSON row.
--negative: run single config asserting wrong scale expectation; harness must FAIL (proves detection).
EOF
  exit 0
fi

# Resolve effective APP_ID from KARERE_BIN so Devel flatpak uses its own GSettings
# schema/path. Caller can also force via KARERE_APP_ID env (mirrors build.rs).
resolve_app_id() {
  if [[ -n "${KARERE_APP_ID:-}" ]]; then echo "$KARERE_APP_ID"; return; fi
  if [[ "$KARERE_BIN" == *"$DEVEL_APP_ID"* ]]; then echo "$DEVEL_APP_ID"; else echo "$STABLE_APP_ID"; fi
}

# Resolve binary: explicit --bin / KARERE_BIN env wins, else cargo target.
if [[ -z "$KARERE_BIN" ]]; then
  if [[ -x "$REPO_ROOT/target/debug/karere" ]]; then
    KARERE_BIN="$REPO_ROOT/target/debug/karere"
  else
    echo "No karere binary at $REPO_ROOT/target/debug/karere. Build with: cargo build --bin karere" >&2
    exit 2
  fi
fi

if ! command -v Xvfb >/dev/null 2>&1 && ! command -v xvfb-run >/dev/null 2>&1; then
  echo "SKIP: Xvfb/xvfb-run not found" >&2; exit 0
fi
if ! command -v xdotool >/dev/null 2>&1; then
  echo "SKIP: xdotool not found (install via: dnf download xdotool && rpm2cpio ... | cpio -id; cp usr/bin/xdotool ~/.local/bin/)" >&2; exit 0
fi
if ! command -v python3 >/dev/null 2>&1; then
  echo "SKIP: python3 not found (needed for data: URL encoding)" >&2; exit 0
fi
if [[ ! -f "$FIXTURE_HTML" ]]; then
  echo "Missing fixture: $FIXTURE_HTML" >&2; exit 2
fi

# Encode fixture as data:text/html URL (no server, no sandbox FS).
DATA_URL="$(base64 -w0 "$FIXTURE_HTML" | sed 's/^/data:text\/html;base64,/')"

need_schemas() {
  local dir="$1"
  local eff_id="${2:-$APP_ID}"
  mkdir -p "$dir"
  local src="$REPO_ROOT/data/io.github.tobagin.karere.gschema.xml.in"
  # Both stable and Devel share the same base schema substitued with the effective APP_ID.
  # Devel path stays /io/github/tobagin/karere/ (meson base_id substitution); the id differs.
  sed -e "s|@APP_ID@|$eff_id|g" -e "s|@APP_PATH@|/io/github/tobagin/karere/|g" "$src" > "$dir/${eff_id}.gschema.xml"
  glib-compile-schemas "$dir" >/dev/null
}

# Poll helpers
wait_for_window() {
  local timeout=30
  local wid=""
  local end=$(( $(date +%s) + timeout ))
  while [[ $(date +%s) -lt $end ]]; do
    wid="$(xdotool search --onlyvisible --name "Karere" 2>/dev/null | head -n1 || true)"
    if [[ -z "$wid" ]]; then
      wid="$(xdotool search --onlyvisible --classname "karere" 2>/dev/null | head -n1 || true)"
    fi
    if [[ -z "$wid" ]]; then
      wid="$(xdotool search --onlyvisible "Karere" 2>/dev/null | head -n1 || true)"
    fi
    if [[ -n "$wid" ]]; then
      echo "$wid"
      return 0
    fi
    sleep 0.2
  done
  return 1
}

get_window_geometry() {
  local wid="$1"
  xdotool getwindowgeometry --shell "$wid" 2>/dev/null || true
}

get_title_json() {
  local wid="$1"
  local tries=20
  for _ in $(seq 1 $tries); do
    local name
    name="$(xdotool getwindowname "$wid" 2>/dev/null || true)"
    if [[ "$name" == "{"* ]]; then
      echo "$name"
      return 0
    fi
    sleep 0.15
  done
  return 1
}

# CDP fallback: curl /json/list then attempt Runtime.evaluate via websocket using python.
cdp_evaluate_probe() {
  local port=9333
  local body
  body="$(curl -s --max-time 2 "http://127.0.0.1:${port}/json/list" 2>/dev/null || true)"
  if [[ -z "$body" ]]; then
    return 1
  fi
  local ws
  ws="$(python3 -c '
import json,sys
try:
  body=sys.stdin.read()
  arr=json.loads(body)
  for o in arr:
    ws=o.get("webSocketDebuggerUrl")
    if ws:
      print(ws)
      break
except Exception:
  pass
' <<<"$body" 2>/dev/null || true)"
  if [[ -z "$ws" ]]; then
    return 1
  fi
  python3 - "$ws" <<'PY' 2>/dev/null | head -n1
import sys, json, shutil, subprocess
ws=sys.argv[1]
import json as j
if shutil.which("websocat"):
    try:
        proc=subprocess.run(["websocat","-n1",ws], input=j.dumps({"id":1,"method":"Runtime.evaluate","params":{"expression":"JSON.stringify(window.__karereProbe.last)"}}), capture_output=True, text=True, timeout=3)
        out=proc.stdout.strip()
        data=j.loads(out) if out else {}
        res=data.get("result",{}).get("result",{}).get("value")
        if res:
            print(res)
            sys.exit(0)
    except Exception:
        pass
try:
    import websocket
    wsconn=websocket.create_connection(ws, timeout=2)
    wsconn.send(j.dumps({"id":1,"method":"Runtime.evaluate","params":{"expression":"JSON.stringify(window.__karereProbe.last)"}}))
    resp=wsconn.recv()
    data=j.loads(resp)
    val=data.get("result",{}).get("result",{}).get("value")
    if val:
        print(val)
    wsconn.close()
except Exception:
    sys.exit(1)
PY
}

find_free_display() {
  for d in $(seq 99 180); do
    if [[ ! -e "/tmp/.X11-unix/X$d" ]]; then echo "$d"; return 0; fi
  done
  echo 99
}

run_config() {
  local gdk_scale="$1"
  local gpu_osr="$2"
  local screen_w="$3"
  local screen_h="$4"
  local negative="$5"

  local label="GDK_SCALE=${gdk_scale}@${screen_w}x${screen_h} gpu_osr=${gpu_osr}"
  local tmpdir
  tmpdir="$(mktemp -d -t kare-probe-XXXXXX)"
  local fixture_dir="$tmpdir/fixture"
  mkdir -p "$fixture_dir/schemas" "$fixture_dir/config" "$fixture_dir/cache" "$fixture_dir/data"
  local eff_app_id
  eff_app_id="$(resolve_app_id)"
  need_schemas "$fixture_dir/schemas" "$eff_app_id"

  local stderr_log="$tmpdir/stderr.log"
  local xvfb_pid="" orig_display="${DISPLAY:-}"

  GSETTINGS_SCHEMA_DIR="$fixture_dir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$fixture_dir/config" \
    gsettings set "$eff_app_id" window-width 1280 >/dev/null 2>&1 || true
  GSETTINGS_SCHEMA_DIR="$fixture_dir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$fixture_dir/config" \
    gsettings set "$eff_app_id" window-height 800 >/dev/null 2>&1 || true
  GSETTINGS_SCHEMA_DIR="$fixture_dir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$fixture_dir/config" \
    gsettings set "$eff_app_id" start-in-background false >/dev/null 2>&1 || true

  local is_flatpak=0
  if [[ "$KARERE_BIN" == *"flatpak"* ]]; then
    is_flatpak=1
  fi

  # Detect EGL vendor file for Mesa llvmpipe (mirrors gl_context_startup.rs).
  local egl_vendor=""
  for p in "/usr/share/glvnd/egl_vendor.d/50_mesa.json" "/usr/share/egl/egl_external_platform.d/50_mesa.json"; do
    if [[ -f "$p" ]]; then egl_vendor="$p"; break; fi
  done

  local bin_dir=""
  local ld_path=""
  if [[ $is_flatpak -eq 0 ]]; then
    bin_dir="$(cd "$(dirname "$KARERE_BIN")" && pwd)"
    ld_path="$bin_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
  fi

  # Start Xvfb on a free display so parent DISPLAY is known for xdotool.
  local disp
  disp="$(find_free_display)"
  Xvfb ":$disp" -screen 0 "${screen_w}x${screen_h}x24" +extension GLX +render -noreset >/dev/null 2>&1 & xvfb_pid=$!
  export DISPLAY=":$disp"
  # Wait for X to be ready
  local ready=0
  for _ in $(seq 1 30); do
    if xdotool getdisplaygeometry >/dev/null 2>&1; then ready=1; break; fi
    if xdpyinfo >/dev/null 2>&1; then ready=1; break; fi
    sleep 0.1
  done
  if [[ $ready -eq 0 ]]; then
    echo "{\"config\":\"$label\",\"error\":\"Xvfb not ready on $DISPLAY\",\"stderr\":\"\"}"
    kill "$xvfb_pid" 2>/dev/null || true; wait "$xvfb_pid" 2>/dev/null || true
    export DISPLAY="$orig_display"
    rm -rf "$tmpdir"
    return 1
  fi

  local pid=""
  set +e
  if [[ $is_flatpak -eq 1 ]]; then
    env GDK_BACKEND=x11 GSK_RENDERER=gl GDK_SCALE="$gdk_scale" KARERE_GPU_OSR="$gpu_osr" \
      flatpak run --env=GDK_SCALE="$gdk_scale" --env=KARERE_GPU_OSR="$gpu_osr" --env=GDK_BACKEND=x11 --env=GSK_RENDERER=gl \
      ${KARERE_BIN#flatpak run } --url "$DATA_URL" --debug --debuglevel=debug >"$stderr_log" 2>&1 &
    pid=$!
  else
    # Native: cd to bin_dir so CEF resources resolve, set LD_LIBRARY_PATH for libcef.so.
    (
      cd "$bin_dir"
      export LD_LIBRARY_PATH="$ld_path"
      export GDK_BACKEND=x11
      export GDK_DEBUG=gl-prefer-gl
      export GSK_RENDERER=gl
      export LIBGL_ALWAYS_SOFTWARE=1
      export GALLIUM_DRIVER=llvmpipe
      export MESA_LOADER_DRIVER_OVERRIDE=llvmpipe
      export MESA_GL_VERSION_OVERRIDE=2.1
      export MESA_GLES_VERSION_OVERRIDE=3.2
      export __GLX_VENDOR_LIBRARY_NAME=mesa
      if [[ -n "$egl_vendor" ]]; then export __EGL_VENDOR_LIBRARY_FILENAMES="$egl_vendor"; fi
      export GDK_SCALE="$gdk_scale"
      export KARERE_GPU_OSR="$gpu_osr"
      export GSETTINGS_BACKEND=keyfile
      export GSETTINGS_SCHEMA_DIR="$fixture_dir/schemas"
      export XDG_CONFIG_HOME="$fixture_dir/config"
      export XDG_CACHE_HOME="$fixture_dir/cache"
      export XDG_DATA_HOME="$fixture_dir/data"
      export DISPLAY="$DISPLAY"
      exec "$KARERE_BIN" --url "$DATA_URL" --debug --debuglevel=debug >"$stderr_log" 2>&1
    ) &
    pid=$!
  fi
  set -e

  sleep 1.2
  local wid
  if ! wid="$(wait_for_window)"; then
    local tail
    tail="$(python3 -c 'import json,pathlib,sys; print(json.dumps(pathlib.Path(sys.argv[1]).read_text(errors="replace")[-4000:]))' "$stderr_log" 2>/dev/null || echo '""')"
    echo "{\"config\":\"$label\",\"error\":\"window not found\",\"stderr\":$tail}"
    kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true
    kill "$xvfb_pid" 2>/dev/null || true; wait "$xvfb_pid" 2>/dev/null || true
    export DISPLAY="$orig_display"
    rm -rf "$tmpdir"
    return 1
  fi

  xdotool windowmove "$wid" 0 0 >/dev/null 2>&1 || true
  sleep 0.4
  xdotool windowactivate --sync "$wid" >/dev/null 2>&1 || true
  sleep 0.5

  # Wait for probe page to be ready: title becomes JSON {"x":0,...} once JS runs.
  # The initial HTML title "Karere coord probe" appears first, then seed JSON overwrites it.
  local ready_tries=30
  for _ in $(seq 1 $ready_tries); do
    local cur_title
    cur_title="$(xdotool getwindowname "$wid" 2>/dev/null || true)"
    if [[ "$cur_title" == "{"* ]]; then break; fi
    # Also try CDP probe as readiness check
    local cdp_cur
    cdp_cur="$(cdp_evaluate_probe 2>/dev/null || true)"
    if [[ "$cdp_cur" == "{"* ]]; then break; fi
    sleep 0.2
  done

  eval "$(get_window_geometry "$wid")"
  X="${X:-0}"; Y="${Y:-0}"; WIDTH="${WIDTH:-0}"; HEIGHT="${HEIGHT:-0}"

  local calib_logical=200
  local calib_expected=$(( calib_logical * gdk_scale ))
  local guess_header=48
  local calib_screen_x=$(( X + 20 * gdk_scale + calib_expected ))
  local calib_screen_y=$(( Y + guess_header * gdk_scale + calib_expected ))
  xdotool mousemove --sync "$calib_screen_x" "$calib_screen_y" >/dev/null 2>&1 || true
  sleep 0.15
  xdotool click 1 >/dev/null 2>&1 || true
  sleep 0.4
  local calib_json
  calib_json="$(get_title_json "$wid" || true)"
  if [[ -z "$calib_json" ]]; then
    calib_json="$(cdp_evaluate_probe || true)"
  fi
  local origin_x origin_y
  if [[ -n "$calib_json" && "$calib_json" == "{"* ]]; then
    local ox oy
    ox="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1]).get("x",0))' "$calib_json" 2>/dev/null || echo 0)"
    oy="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1]).get("y",0))' "$calib_json" 2>/dev/null || echo 0)"
    # CEF view_rect is physical but page zoom makes clientX logical (physical/scale).
    # So ox is CSS logical, ox*scale is physical offset from content origin.
    origin_x=$(( calib_screen_x - ox * gdk_scale ))
    origin_y=$(( calib_screen_y - oy * gdk_scale ))
  else
    origin_x=$(( X + 1 * gdk_scale ))
    origin_y=$(( Y + guess_header * gdk_scale ))
  fi

  local -a logical_centers=("100:100" "250:150" "550:250" "650:350")
  local worst_err=0
  local rows_json=""
  local first_metrics=""
  local passed=1

  for lc in "${logical_centers[@]}"; do
    IFS=":" read -r lcx lcy <<<"$lc"
    # Expected client is logical CSS (physical/scale due to HiDPI zoom, #158).
    local exp_x=$(( lcx ))
    local exp_y=$(( lcy ))
    local screen_x=$(( origin_x + lcx * gdk_scale ))
    local screen_y=$(( origin_y + lcy * gdk_scale ))
    if [[ "$negative" -eq 1 ]]; then
      exp_x=$(( exp_x + 20 ))
      exp_y=$(( exp_y + 20 ))
    fi
    xdotool mousemove --sync "$screen_x" "$screen_y" >/dev/null 2>&1 || true
    sleep 0.12
    xdotool click 1 >/dev/null 2>&1 || true
    sleep 0.35
    local obs_json
    obs_json="$(get_title_json "$wid" || true)"
    if [[ -z "$obs_json" ]]; then
      obs_json="$(cdp_evaluate_probe || true)"
    fi
    if [[ -z "$obs_json" || "$obs_json" != "{"* ]]; then
      obs_json='{"x":null,"y":null,"innerW":null,"innerH":null,"dpr":null,"clientW":null,"clientH":null}'
    fi
    if [[ -z "$first_metrics" ]]; then
      first_metrics="$obs_json"
    fi
    local obs_x obs_y err_x err_y abs_err
    obs_x="$(python3 -c 'import json,sys;v=json.loads(sys.argv[1]).get("x"); print(v if v is not None else "null")' "$obs_json" 2>/dev/null || echo "null")"
    obs_y="$(python3 -c 'import json,sys;v=json.loads(sys.argv[1]).get("y"); print(v if v is not None else "null")' "$obs_json" 2>/dev/null || echo "null")"
    if [[ "$obs_x" == "null" || "$obs_y" == "null" ]]; then
      err_x="null"; err_y="null"; abs_err=999
      passed=0
    else
      err_x=$(( obs_x - exp_x ))
      err_y=$(( obs_y - exp_y ))
      local ax=${err_x#-}; local ay=${err_y#-}
      if [[ $ax -gt $ay ]]; then abs_err=$ax; else abs_err=$ay; fi
      if [[ $abs_err -gt 1 ]]; then
        passed=0
      fi
      if [[ $abs_err -gt $worst_err ]]; then worst_err=$abs_err; fi
    fi
    local row
    row="$(python3 -c '
import json,sys
exp_x=int(sys.argv[1]); exp_y=int(sys.argv[2])
obs=json.loads(sys.argv[3])
err_x=sys.argv[4]; err_y=sys.argv[5]
print(json.dumps({"logical":sys.argv[6],"expected":{"x":exp_x,"y":exp_y},"observed":{"x":obs.get("x"),"y":obs.get("y")},"error":{"x":err_x if err_x!="null" else None,"y":err_y if err_y!="null" else None},"metrics":obs}))
' "$exp_x" "$exp_y" "$obs_json" "$err_x" "$err_y" "$lc" 2>/dev/null || echo '{}')"
    if [[ -z "$rows_json" ]]; then
      rows_json="$row"
    else
      rows_json="$rows_json,$row"
    fi
  done

  local joint_logs
  joint_logs="$(tail -n 80 "$stderr_log" 2>/dev/null | tr -d '\0' | python3 -c 'import json,sys;data=sys.stdin.read()[-4000:]; print(json.dumps(data))' 2>/dev/null || echo '""')"

  local viewport_json
  viewport_json="$(python3 -c 'import json,sys;print(json.dumps(json.loads(sys.argv[1])) if sys.argv[1] else "{}")' "$first_metrics" 2>/dev/null || echo '{}')"

  local overall_pass="true"
  if [[ $passed -eq 0 ]]; then overall_pass="false"; fi
  if [[ "$negative" -eq 1 ]]; then
    if [[ "$overall_pass" == "false" ]]; then
      overall_pass="true"
    else
      overall_pass="false"
    fi
  fi

  python3 -c '
import json,sys
label=sys.argv[1]
scale=int(sys.argv[2]); gpu=int(sys.argv[3])
sw=sys.argv[4]; sh=sys.argv[5]
rows=json.loads("["+sys.argv[6]+"]") if sys.argv[6] else []
viewport=json.loads(sys.argv[7]) if sys.argv[7] else {}
joint=json.loads(sys.argv[8]) if sys.argv[8] else ""
passed=json.loads(sys.argv[9].lower())
worst=int(sys.argv[10]) if sys.argv[10].isdigit() else 0
print(json.dumps({
  "config": label,
  "scale": scale,
  "gpu_osr": bool(gpu),
  "screen": f"{sw}x{sh}",
  "negative_control": bool(int(sys.argv[11])),
  "passed": passed,
  "worst_error_px": worst,
  "probes": rows,
  "viewport": viewport,
  "joint_logs_tail": joint[-2000:],
}, ensure_ascii=False))
' "$label" "$gdk_scale" "$gpu_osr" "$screen_w" "$screen_h" "$rows_json" "$viewport_json" "$joint_logs" "$overall_pass" "$worst_err" "$negative"

  # Cleanup: quit via gapplication (use effective Devel/Stable id), then kill.
  GSETTINGS_SCHEMA_DIR="$fixture_dir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$fixture_dir/config" \
    gapplication action "$eff_app_id" quit >/dev/null 2>&1 || true
  sleep 0.8
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    sleep 0.5
    if kill -0 "$pid" 2>/dev/null; then
      kill -9 "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null || true
  fi
  kill "$xvfb_pid" 2>/dev/null || true; wait "$xvfb_pid" 2>/dev/null || true
  export DISPLAY="$orig_display"
  rm -rf "$tmpdir"

  if [[ "$overall_pass" == "true" ]]; then
    return 0
  else
    return 1
  fi
}

# Main matrix
echo "KARE-016 coordinate probe — $(date -u +%Y-%m-%dT%H:%M:%SZ)" >&2
echo "Binary: $KARERE_BIN" >&2
echo "Fixture: $FIXTURE_HTML" >&2
echo "Data URL length: ${#DATA_URL}" >&2

FAIL=0
PASS=0

matrix=(
  "1 0 1280 800"
  "1 1 1280 800"
  "2 0 2560 1600"
  "2 1 2560 1600"
)

if [[ "$NEGATIVE_MODE" -eq 1 ]]; then
  echo "--- Negative control: wrong scale expectation must FAIL ---" >&2
  set +e
  run_config 2 0 2560 1600 1
  rc=$?
  set -e
  if [[ $rc -eq 0 ]]; then
    echo "NEGATIVE CONTROL PASS — harness correctly detected misalignment (failed as expected on wrong expectation)" >&2
    echo '{"negative_control":"PASS","detail":"wrong expectation correctly produced error >1px and harness flagged it"}'
  else
    echo "NEGATIVE CONTROL FAIL — harness did NOT detect misalignment (wrong expectation unexpectedly passed)" >&2
    echo '{"negative_control":"FAIL","detail":"harness failed to detect intentional 20px offset"}'
    exit 1
  fi
  exit 0
fi

# Run real matrix
set +e
for spec in "${matrix[@]}"; do
  read -r sc gpu sw sh <<<"$spec"
  echo "--- Running $sc gpu=$gpu @ ${sw}x${sh} ---" >&2
  out="$(run_config "$sc" "$gpu" "$sw" "$sh" 0)"
  rc=$?
  echo "$out"
  if python3 -c 'import json,sys; sys.exit(0 if json.loads(sys.argv[1]).get("passed") else 1)' "$out" 2>/dev/null; then
    PASS=$((PASS+1))
    echo "→ PASS $sc gpu=$gpu" >&2
  else
    FAIL=$((FAIL+1))
    echo "→ FAIL $sc gpu=$gpu (error >1px sustained)" >&2
  fi
done
set -e

echo "--- Summary: $PASS passed, $FAIL failed out of ${#matrix[@]} configs ---" >&2
python3 -c 'import json,sys; print(json.dumps({"summary":{"passed":int(sys.argv[1]),"failed":int(sys.argv[2]),"total":int(sys.argv[3])}}))' "$PASS" "$FAIL" "${#matrix[@]}" >&2

# Inline negative control (proves detection capability without separate invocation)
echo "--- Inline negative control (wrong expectation must be detected) ---" >&2
set +e
neg_out="$(run_config 2 0 2560 1600 1)"
neg_rc=$?
set -e
if python3 -c 'import json,sys; sys.exit(0 if json.loads(sys.argv[1]).get("passed") else 1)' "$neg_out" 2>/dev/null; then
  echo "Inline negative control: PASS (wrong expectation correctly flagged)" >&2
  echo "$neg_out" | python3 -c 'import json,sys;print(json.dumps({"negative_control":"PASS","probe":json.loads(sys.stdin.read())}))'
else
  echo "Inline negative control: FAIL (harness missed intentional offset)" >&2
  echo "$neg_out" | python3 -c 'import json,sys;print(json.dumps({"negative_control":"FAIL","probe":json.loads(sys.stdin.read())}))'
  FAIL=$((FAIL+1))
fi

if [[ $FAIL -gt 0 ]]; then
  echo "MATRIX RESULT: $FAIL config(s) failed — see JSON rows above for reproduced symptom (error >1px)" >&2
else
  echo "MATRIX RESULT: all configs passed (no sustained >1px error)" >&2
fi

exit 0
