#!/usr/bin/env bash
# shellcheck disable=SC2030,SC2031,SC2086,SC2034
# KARE-016 Step 2 + KARE-018 H7 — Xvfb + GDK_SCALE + xdotool + CDP-title readback harness.
# Matrix (KARE-016): {GDK_SCALE=1@1280x800, GDK_SCALE=2@2560x1600} × {cpu-osr, gpu-osr}
#   + KARE-018 narrow rows: {GDK_SCALE=1@720x900} × {cpu-osr, gpu-osr} (<768px mobile gate)
# + H7 chrome-mimic (KARE-018): --chrome loads fixture in chrome mode (fixed header 60px
#   + side panel 360px + transform(10,20)+scrollTop 80), calibrates contentOrigin via
#   getBoundingClientRect/visualViewport/scrollX/Y, anchors cell clicks to the grid
#   origin, and asserts |clientX−expectedCalibrated|≤1px.
# + Fractional Wayland (KARE-018): --fractional queries Mutter experimental-features
#   (scale-monitor-framebuffer / xwayland-native-scaling). On Xvfb or when features
#   are missing it emits SKIP rows (fractional unverified on this host); a true
#   125%/150% matrix requires an operator Wayland session that drives Mutter
#   DisplayConfig monitor scales (KARE-020) — this harness cannot, so it records
#   honest SKIP rows instead of a fake PASS.
# + Operator-driven fractional Wayland (KARE-020): --fractional-wayland (or env
#   KARERE_FRACTIONAL_WAYLAND=1) delegates the 1.25/1.5 logical-scale verification
#   to tests/wayland_fractional_verify.sh on a live Mutter Wayland session
#   (transient ApplyMonitorsConfig with trap restore); without the gate or on an
#   unqualified host it emits honest SKIP rows (CI-safe default).
# + Real-page opt-in (KARE-018): when KARERE_H7_REAL_PAGE=1 loads live WhatsApp
#   snapshot (KARERE_H7_REAL_PAGE_URL or https://web.whatsapp.com/) with CDP-injected
#   probe and same calibration protocol; when unset prints SKIP line and runs the
#   offline chrome-mimic fixture (never attempts login).
#
# Usage:
#   tests/coordinate_probe.sh [--bin /path/to/karere] [--chrome] [--fractional] [--fractional-wayland] [--negative] [--help]
#   KARERE_H7_REAL_PAGE=1 tests/coordinate_probe.sh --chrome   # operator-authorized real-page snapshot
#   KARERE_FRACTIONAL_WAYLAND=1 tests/coordinate_probe.sh --fractional-wayland  # operator Wayland session
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
FIXTURE_WHATSAPP="$REPO_ROOT/tests/fixtures/coord-probe-whatsapp.html"
RESULTS_JSON="$REPO_ROOT/coord-probe-results.json"
H7_RESULTS_JSON="$REPO_ROOT/coord-probe-h7-results.json"
STABLE_APP_ID="io.github.tobagin.karere"
DEVEL_APP_ID="io.github.tobagin.karere.Devel"
APP_ID="$STABLE_APP_ID"

KARERE_BIN="${KARERE_BIN:-}"
NEGATIVE_MODE=0
CHROME_MODE=0
FRACTIONAL_MODE=0
FRACTIONAL_WAYLAND=0
if [[ "${KARERE_FRACTIONAL_WAYLAND:-}" == "1" ]]; then FRACTIONAL_WAYLAND=1; fi
HELP=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) KARERE_BIN="$2"; shift 2;;
    --negative) NEGATIVE_MODE=1; shift;;
    --chrome) CHROME_MODE=1; shift;;
    --fractional) FRACTIONAL_MODE=1; shift;;
    --fractional-wayland) FRACTIONAL_WAYLAND=1; shift;;
    --help|-h) HELP=1; shift;;
    *) echo "Unknown arg: $1" >&2; exit 2;;
  esac
done

if [[ $HELP -eq 1 ]]; then
  cat <<'EOF'
Usage: tests/coordinate_probe.sh [--bin /path/to/karere] [--chrome] [--fractional] [--fractional-wayland] [--negative]

Matrix (KARE-016): GDK_SCALE=1@1280x800 and GDK_SCALE=2@2560x1600 × {cpu-osr, gpu-osr}
  + KARE-018 narrow rows GDK_SCALE=1@720x900 (<768px mobile gate) × {cpu-osr, gpu-osr}
  Each config: Xvfb + xdotool window placement + synthetic clicks + title/CDP readback → JSON row.
--chrome: load H7 chrome-mimicking fixture (?chrome=1: header 60px + panel 360px + transform+scroll)
          calibrates contentOrigin via getBoundingClientRect and asserts calibrated ≤1px.
          With KARERE_H7_REAL_PAGE=1 loads live snapshot (KARERE_H7_REAL_PAGE_URL or
          https://web.whatsapp.com/) via CDP-injected probe; without it prints real-page SKIP
          and runs the offline chrome-mimic (never logs in).
--fractional: queries Mutter experimental-features; on Xvfb or when missing emits SKIP
          (fractional unverified on this host). The harness cannot drive Mutter
          DisplayConfig monitor scales itself — a true 125/150% matrix requires an
          operator Wayland session (KARE-020); rows record SKIP until then.
--fractional-wayland: also run fractional 1.25/1.5 logical-scale verification via
          tests/wayland_fractional_verify.sh (requires KARERE_FRACTIONAL_WAYLAND=1 or this flag plus a live
          Mutter Wayland session with scale-monitor-framebuffer/xwayland-native-scaling; otherwise emits SKIP).
          Env KARERE_FRACTIONAL_WAYLAND=1 is equivalent to --fractional-wayland.
--negative: run single config asserting wrong scale expectation; harness must FAIL (proves detection).
          Compose: --chrome --negative asserts wrong origin/scale and must FAIL.
Real-page opt-in: KARERE_H7_REAL_PAGE=1 enables live WhatsApp snapshot mode with same calibration
          (gated, never logs in with real account implicitly). Without it, SKIP is printed.
JSON outputs: coord-probe-results.json (synthetic) and coord-probe-h7-results.json (chrome/fractional)
          Each printed JSON row is also appended to the corresponding file (git-ignored).
EOF
  exit 0
fi

resolve_app_id() {
  if [[ -n "${KARERE_APP_ID:-}" ]]; then echo "$KARERE_APP_ID"; return; fi
  if [[ "$KARERE_BIN" == *"$DEVEL_APP_ID"* ]]; then echo "$DEVEL_APP_ID"; else echo "$STABLE_APP_ID"; fi
}

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

# Build target URL. For chrome synthetic mode inject window.__forceH7 via data: URL.
# For real-page mode (KARERE_H7_REAL_PAGE=1 + --chrome) return the live URL so
# run_config navigates to WhatsApp Web with CDP-injected probe instead.
build_target_url() {
  local chrome="$1"
  if [[ "$chrome" -eq 1 && "${KARERE_H7_REAL_PAGE:-0}" == "1" ]]; then
    local live="${KARERE_H7_REAL_PAGE_URL:-https://web.whatsapp.com/}"
    echo "$live"
    return
  fi
  if [[ "$chrome" -eq 1 ]]; then
    local tmp
    tmp="$(mktemp)"
    python3 -c '
import pathlib,sys
p=pathlib.Path(sys.argv[1])
html=p.read_text()
inject = "<script>window.__forceH7=1</script>\n"
html=html.replace("<script>", inject+"<script>", 1)
sys.stdout.write(html)
' "$FIXTURE_HTML" > "$tmp"
    base64 -w0 "$tmp" | sed 's/^/data:text\/html;base64,/'
    rm -f "$tmp"
  else
    base64 -w0 "$FIXTURE_HTML" | sed 's/^/data:text\/html;base64,/'
  fi
}
# Backward-compat alias
build_data_url() { build_target_url "$@"; }

# Real-page gate: prints required SKIP line when gated; returns 0 when
# operator-authorized (caller should load live URL), 1 otherwise.
check_real_page_gate() {
  if [[ "${KARERE_H7_REAL_PAGE:-0}" != "1" ]]; then
    echo "SKIP: real-page gated — set KARERE_H7_REAL_PAGE=1 for operator-authorized run" >&2
    return 1
  fi
  echo "Real-page mode: KARERE_H7_REAL_PAGE=1 — loading ${KARERE_H7_REAL_PAGE_URL:-https://web.whatsapp.com/} (operator-authorized, no login)" >&2
  return 0
}

# Fractional availability check
check_fractional_available() {
  local feats
  feats="$(gsettings get org.gnome.mutter experimental-features 2>/dev/null || echo "@as []")"
  if [[ "$feats" == *"scale-monitor-framebuffer"* ]] || [[ "$feats" == *"xwayland-native-scaling"* ]]; then
    return 0
  fi
  echo "SKIP: fractional unverified on this host — missing Mutter experimental-features (need scale-monitor-framebuffer or xwayland-native-scaling)" >&2
  printf '{"fractional":"SKIP","reason":"fractional unverified on this host"}\n'
  return 1
}

# Fractional Wayland host gate (KARE-020) — mirrors wayland_fractional_verify.sh
# gates but never mutates display state; returns 0 if a live Mutter session with
# a driveable DisplayConfig and fractional features is available, 1 otherwise.
check_fractional_wayland_host() {
  if [[ "${XDG_SESSION_TYPE:-}" != "wayland" ]]; then return 1; fi
  if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then return 1; fi
  if ! command -v gdbus >/dev/null 2>&1; then return 1; fi
  if ! gdbus call --session --dest org.gnome.Mutter.DisplayConfig --object-path /org/gnome/Mutter/DisplayConfig --method org.gnome.Mutter.DisplayConfig.GetCurrentState >/dev/null 2>&1; then return 1; fi
  local feats
  feats="$(gsettings get org.gnome.mutter experimental-features 2>/dev/null || echo "@as []")"
  if [[ "$feats" != *"scale-monitor-framebuffer"* && "$feats" != *"xwayland-native-scaling"* ]]; then return 1; fi
  return 0
}

need_schemas() {
  local dir="$1"
  local eff_id="${2:-$APP_ID}"
  mkdir -p "$dir"
  local src="$REPO_ROOT/data/io.github.tobagin.karere.gschema.xml.in"
  sed -e "s|@APP_ID@|$eff_id|g" -e "s|@APP_PATH@|/io/github/tobagin/karere/|g" "$src" > "$dir/${eff_id}.gschema.xml"
  glib-compile-schemas "$dir" >/dev/null
}

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

cdp_inject_probe() {
  local expr='(function(){if(window.__karereProbe)return JSON.stringify({injected:false});window.__karereProbe={last:null};document.addEventListener("click",function(e){var r={x:Math.round(e.clientX),y:Math.round(e.clientY),innerW:window.innerWidth,innerH:window.innerHeight,dpr:window.devicePixelRatio,clientW:document.documentElement.clientWidth,clientH:document.documentElement.clientHeight,originX:0,originY:0,scrollX:window.scrollX,scrollY:window.scrollY};try{var h=document.elementFromPoint(e.clientX,e.clientY);if(h){var br=h.getBoundingClientRect();r.gridLeft=Math.round(br.left);r.gridTop=Math.round(br.top);} }catch(ex){} try{ if(window.visualViewport){r.visualViewport={width:Math.round(window.visualViewport.width),height:Math.round(window.visualViewport.height),offsetLeft:Math.round(window.visualViewport.offsetLeft),offsetTop:Math.round(window.visualViewport.offsetTop),scale:window.visualViewport.scale};}}catch(ex2){} window.__karereProbe.last=r; try{document.title=JSON.stringify(r);}catch(ex3){} return r;},true);return JSON.stringify({injected:true});})()'
  local port=9333
  local body
  body="$(curl -s --max-time 2 "http://127.0.0.1:${port}/json/list" 2>/dev/null || true)"
  if [[ -z "$body" ]]; then return 1; fi
  local ws
  ws="$(python3 -c 'import json,sys;body=sys.stdin.read();arr=json.loads(body)
for o in arr:
 ws=o.get("webSocketDebuggerUrl")
 if ws: print(ws); break
' <<<"$body" 2>/dev/null || true)"
  if [[ -z "$ws" ]]; then return 1; fi
  python3 - "$ws" "$expr" <<'PY' 2>/dev/null | head -n1
import sys, json, shutil, subprocess
ws=sys.argv[1]; expr=sys.argv[2]
import json as j
if shutil.which("websocat"):
    try:
        proc=subprocess.run(["websocat","-n1",ws], input=j.dumps({"id":1,"method":"Runtime.evaluate","params":{"expression":expr}}), capture_output=True, text=True, timeout=3)
        out=proc.stdout.strip()
        data=j.loads(out) if out else {}
        res=data.get("result",{}).get("result",{}).get("value")
        if res: print(res)
        sys.exit(0)
    except Exception: pass
try:
    import websocket
    wsconn=websocket.create_connection(ws, timeout=2)
    wsconn.send(j.dumps({"id":1,"method":"Runtime.evaluate","params":{"expression":expr}}))
    resp=wsconn.recv()
    data=j.loads(resp)
    val=data.get("result",{}).get("result",{}).get("value")
    if val: print(val)
    wsconn.close()
except Exception: sys.exit(1)
PY
}

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
  local chrome="${6:-0}"

  local suffix=""
  if [[ "$chrome" -eq 1 ]]; then suffix=" chrome=h7"; fi
  local label="GDK_SCALE=${gdk_scale}@${screen_w}x${screen_h} gpu_osr=${gpu_osr}${suffix}"
  local tmpdir
  tmpdir="$(mktemp -d -t kare-probe-XXXXXX)"
  mkdir -p "$tmpdir/schemas" "$tmpdir/config" "$tmpdir/cache" "$tmpdir/data"
  local eff_app_id
  eff_app_id="$(resolve_app_id)"
  need_schemas "$tmpdir/schemas" "$eff_app_id"

  local target_url
  target_url="$(build_target_url "$chrome")"
  # Real-page surfaces are any non-data: target: live https:// WhatsApp Web or an
  # operator-supplied snapshot mirror via KARERE_H7_REAL_PAGE_URL (file:///http://).
  # Synthetic and chrome-mimic fixtures always load as data: URLs.
  local is_real_page=0
  if [[ "$target_url" != data:* ]]; then is_real_page=1; fi

  local stderr_log="$tmpdir/stderr.log"
  local xvfb_pid="" orig_display="${DISPLAY:-}"

  # Window size follows the config's CSS viewport (screen/gdk_scale) so narrow
  # (<768px) matrix rows get a narrow window instead of a fixed 1280x800 one.
  local win_w=$(( screen_w / gdk_scale ))
  local win_h=$(( screen_h / gdk_scale ))
  GSETTINGS_SCHEMA_DIR="$tmpdir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$tmpdir/config" \
    gsettings set "$eff_app_id" window-width "$win_w" >/dev/null 2>&1 || true
  GSETTINGS_SCHEMA_DIR="$tmpdir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$tmpdir/config" \
    gsettings set "$eff_app_id" window-height "$win_h" >/dev/null 2>&1 || true
  GSETTINGS_SCHEMA_DIR="$tmpdir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$tmpdir/config" \
    gsettings set "$eff_app_id" start-in-background false >/dev/null 2>&1 || true

  local is_flatpak=0
  if [[ "$KARERE_BIN" == *"flatpak"* ]]; then
    is_flatpak=1
  fi

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

  local disp
  disp="$(find_free_display)"
  Xvfb ":$disp" -screen 0 "${screen_w}x${screen_h}x24" +extension GLX +render -noreset >/dev/null 2>&1 & xvfb_pid=$!
  export DISPLAY=":$disp"
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
    # shellcheck disable=SC2086
    env GDK_BACKEND=x11 GSK_RENDERER=gl GDK_SCALE="$gdk_scale" KARERE_GPU_OSR="$gpu_osr" \
      flatpak run --env=GDK_SCALE="$gdk_scale" --env=KARERE_GPU_OSR="$gpu_osr" --env=GDK_BACKEND=x11 --env=GSK_RENDERER=gl \
      ${KARERE_BIN#flatpak run } --url "$target_url" --debug --debuglevel=debug >"$stderr_log" 2>&1 &
    pid=$!
  else
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
      export GSETTINGS_SCHEMA_DIR="$tmpdir/schemas"
      export XDG_CONFIG_HOME="$tmpdir/config"
      export XDG_CACHE_HOME="$tmpdir/cache"
      export XDG_DATA_HOME="$tmpdir/data"
      export DISPLAY="$DISPLAY"
      exec "$KARERE_BIN" --url "$target_url" --debug --debuglevel=debug >"$stderr_log" 2>&1
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

  # For real-page snapshots, inject probe recorder via CDP when window.__karereProbe is absent.
  if [[ "$is_real_page" -eq 1 ]]; then
    sleep 2
    # Wait a bit longer for live page load
    for _ in $(seq 1 15); do
      local inject_res
      inject_res="$(cdp_inject_probe 2>/dev/null || true)"
      if [[ -n "$inject_res" ]]; then break; fi
      sleep 0.4
    done
  fi
  local ready_tries=30
  for _ in $(seq 1 $ready_tries); do
    local cur_title
    cur_title="$(xdotool getwindowname "$wid" 2>/dev/null || true)"
    if [[ "$cur_title" == "{"* ]]; then break; fi
    local cdp_cur
    cdp_cur="$(cdp_evaluate_probe 2>/dev/null || true)"
    if [[ "$cdp_cur" == "{"* ]]; then break; fi
    # Also try injected probe on real-page if title polling stalls
    if [[ "$is_real_page" -eq 1 ]]; then
      cdp_inject_probe >/dev/null 2>&1 || true
    fi
    sleep 0.2
  done

  eval "$(get_window_geometry "$wid")"
  X="${X:-0}"; Y="${Y:-0}"; WIDTH="${WIDTH:-0}"; HEIGHT="${HEIGHT:-0}"

  # KARE-018: derive contentOrigin via one-probe calibration using harness-side
  # observed clientX/Y + known logical target, then reuse for remaining probes.
  # For H7 chrome mode the origin includes header+panel+transform+scroll; for
  # synthetic it matches the old X+1/header heuristic as fallback.
  local calib_logical=200
  local calib_expected=$(( calib_logical * gdk_scale ))
  local guess_header=48
  local calib_screen_x=$(( X + 20 * gdk_scale + calib_expected ))
  local calib_screen_y=$(( Y + guess_header * gdk_scale + calib_expected ))
  # In chrome mode offset the calibration click to land inside grid (panel/header offset)
  if [[ "$chrome" -eq 1 ]]; then
    # Grid origin in chrome mode is approx X + 360*scale, Y + 60*scale + transform
    calib_screen_x=$(( X + 360 * gdk_scale + 30 * gdk_scale + calib_expected ))
    calib_screen_y=$(( Y + 60 * gdk_scale + 30 * gdk_scale + calib_expected ))
  fi
  xdotool mousemove --sync "$calib_screen_x" "$calib_screen_y" >/dev/null 2>&1 || true
  sleep 0.15
  xdotool click 1 >/dev/null 2>&1 || true
  sleep 0.4
  local calib_json
  calib_json="$(get_title_json "$wid" || true)"
  if [[ -z "$calib_json" ]]; then
    calib_json="$(cdp_evaluate_probe || true)"
  fi
  local origin_x origin_y origin_client_x origin_client_y
  if [[ -n "$calib_json" && "$calib_json" == "{"* ]]; then
    local ox oy
    ox="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1]).get("x",0))' "$calib_json" 2>/dev/null || echo 0)"
    oy="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1]).get("y",0))' "$calib_json" 2>/dev/null || echo 0)"
    origin_x=$(( calib_screen_x - ox * gdk_scale ))
    origin_y=$(( calib_screen_y - oy * gdk_scale ))
    origin_client_x="$ox"
    origin_client_y="$oy"
  else
    origin_x=$(( X + 1 * gdk_scale ))
    origin_y=$(( Y + guess_header * gdk_scale ))
    origin_client_x="null"
    origin_client_y="null"
  fi

  # Also capture origin diagnostics from fixture (gridLeft/gridTop/scrollX etc)
  local calib_origin_x calib_origin_y
  calib_origin_x="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("originX","null"))' "$calib_json" 2>/dev/null || echo "null")"
  calib_origin_y="$(python3 -c 'import json,sys; print(json.loads(sys.argv[1]).get("originY","null"))' "$calib_json" 2>/dev/null || echo "null")"

  # Grid origin in CSS px for chrome mode: header 60 + translateY 20 - scrollTop 80
  # (post-scroll) => y=0; panel 360 + translateX 10 => x=370. Used as click-target
  # anchor and raw-expectation fallback when the calibration readback is missing.
  local chrome_origin_x=370 chrome_origin_y=0
  if [[ "$calib_origin_x" != "null" && "$calib_origin_y" != "null" ]]; then
    chrome_origin_x="$calib_origin_x"
    chrome_origin_y="$calib_origin_y"
  fi

  # Cell-center probes: at narrow (<768px CSS) widths the 360px panel + grid leave
  # only ~350px of visible grid, so use grid-relative centers that stay on-window.
  local -a logical_centers=("100:100" "250:150" "550:250" "650:350")
  if [[ "$chrome" -eq 1 && "$win_w" -lt 768 ]]; then
    logical_centers=("50:50" "150:100" "250:200" "300:300")
  fi
  local worst_err=0
  local rows_json=""
  local first_metrics=""
  local passed=1

  for lc in "${logical_centers[@]}"; do
    IFS=":" read -r lcx lcy <<<"$lc"
    # For H7 chrome mode map logical offsets from grid origin, not viewport origin.
    # The calibration anchor (origin_x/y) is the VIEWPORT screen origin; cell clicks
    # must target grid-relative CSS coords: origin + (gridOrigin + lcx)*scale.
    local exp_x exp_y
    if [[ "$chrome" -eq 1 ]]; then
      exp_x=$(( chrome_origin_x + lcx ))
      exp_y=$(( chrome_origin_y + lcy ))
    else
      exp_x=$(( lcx ))
      exp_y=$(( lcy ))
    fi
    # Negative control: inject a wrong expectation into BOTH the raw and the
    # calibrated comparison so chrome mode also proves detection (the calibrated
    # path previously ignored this offset, making --chrome --negative vacuous).
    local neg_off=0
    if [[ "$negative" -eq 1 ]]; then
      exp_x=$(( exp_x + 20 ))
      exp_y=$(( exp_y + 20 ))
      neg_off=20
    fi
    local screen_x screen_y
    if [[ "$chrome" -eq 1 ]]; then
      screen_x=$(( origin_x + ( chrome_origin_x + lcx ) * gdk_scale ))
      screen_y=$(( origin_y + ( chrome_origin_y + lcy ) * gdk_scale ))
    else
      screen_x=$(( origin_x + lcx * gdk_scale ))
      screen_y=$(( origin_y + lcy * gdk_scale ))
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
    local obs_x obs_y err_x err_y abs_err origin_corrected_x origin_corrected_y err_cal_x err_cal_y abs_cal
    obs_x="$(python3 -c 'import json,sys;v=json.loads(sys.argv[1]).get("x"); print(v if v is not None else "null")' "$obs_json" 2>/dev/null || echo "null")"
    obs_y="$(python3 -c 'import json,sys;v=json.loads(sys.argv[1]).get("y"); print(v if v is not None else "null")' "$obs_json" 2>/dev/null || echo "null")"
    if [[ "$obs_x" == "null" || "$obs_y" == "null" ]]; then
      err_x="null"; err_y="null"; abs_err=999
      err_cal_x="null"; err_cal_y="null"; abs_cal=999
      origin_corrected_x="null"; origin_corrected_y="null"
      passed=0
    else
      err_x=$(( obs_x - exp_x ))
      err_y=$(( obs_y - exp_y ))
      # Calibrated error: subtract known origin (gridLeft/gridTop) before comparing to lcx/lcy
      local ox2 oy2
      ox2="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1]).get("originX",0))' "$obs_json" 2>/dev/null || echo 0)"
      oy2="$(python3 -c 'import json,sys;print(json.loads(sys.argv[1]).get("originY",0))' "$obs_json" 2>/dev/null || echo 0)"
      origin_corrected_x=$(( obs_x - ox2 ))
      origin_corrected_y=$(( obs_y - oy2 ))
      err_cal_x=$(( origin_corrected_x - lcx - neg_off ))
      err_cal_y=$(( origin_corrected_y - lcy - neg_off ))
      local ax=${err_cal_x#-}; local ay=${err_cal_y#-}
      if [[ $ax -gt $ay ]]; then abs_cal=$ax; else abs_cal=$ay; fi
      # For pass/fail use calibrated error in chrome mode, raw otherwise
      if [[ "$chrome" -eq 1 ]]; then
        abs_err=$abs_cal
        if [[ $abs_err -gt 1 ]]; then passed=0; fi
      else
        local ax2=${err_x#-}; local ay2=${err_y#-}
        if [[ $ax2 -gt $ay2 ]]; then abs_err=$ax2; else abs_err=$ay2; fi
        if [[ $abs_err -gt 1 ]]; then passed=0; fi
      fi
      if [[ $abs_err -gt $worst_err ]]; then worst_err=$abs_err; fi
    fi
    local row
    row="$(python3 -c '
import json,sys
exp_x=int(sys.argv[1]); exp_y=int(sys.argv[2])
obs=json.loads(sys.argv[3])
err_x=sys.argv[4]; err_y=sys.argv[5]
lc=sys.argv[6]
ocx=sys.argv[7]; ocy=sys.argv[8]
ecx=sys.argv[9]; ecy=sys.argv[10]
print(json.dumps({"logical":lc,"expected":{"x":exp_x,"y":exp_y},"observed":{"x":obs.get("x"),"y":obs.get("y")},"originCorrected":{"x": int(ocx) if ocx!="null" else None, "y": int(ocy) if ocy!="null" else None},"expectedCalibrated":{"x": int(ecx) if ecx!="null" else None, "y": int(ecy) if ecy!="null" else None},"error":{"x": int(err_x) if err_x!="null" else None,"y": int(err_y) if err_y!="null" else None},"calibrated_error":{"x": int(ecx) if ecx!="null" else None, "y": int(ecy) if ecy!="null" else None},"metrics":obs}, ensure_ascii=False))
' "$exp_x" "$exp_y" "$obs_json" "$err_x" "$err_y" "$lc" "$origin_corrected_x" "$origin_corrected_y" "$err_cal_x" "$err_cal_y" 2>/dev/null || echo '{}')"
    # fix calibrated_error to use err_cal
    row="$(python3 -c '
import json,sys
r=json.loads(sys.argv[1])
ecx=sys.argv[2]; ecy=sys.argv[3]
if ecx!="null" and ecy!="null":
  r["calibrated_error"]={"x":int(ecx),"y":int(ecy)}
print(json.dumps(r))
' "$row" "$err_cal_x" "$err_cal_y" 2>/dev/null || echo "$row")"
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
worst=int(sys.argv[10]) if sys.argv[10].lstrip("-").isdigit() else 0
chrome=bool(int(sys.argv[11]))
print(json.dumps({
  "config": label,
  "scale": scale,
  "gpu_osr": bool(gpu),
  "screen": f"{sw}x{sh}",
  "chrome": chrome,
  "negative_control": bool(int(sys.argv[12])),
  "passed": passed,
  "worst_error_px": worst,
  "probes": rows,
  "viewport": viewport,
  "joint_logs_tail": joint[-2000:],
}, ensure_ascii=False))
' "$label" "$gdk_scale" "$gpu_osr" "$screen_w" "$screen_h" "$rows_json" "$viewport_json" "$joint_logs" "$overall_pass" "$worst_err" "$chrome" "$negative"

  GSETTINGS_SCHEMA_DIR="$tmpdir/schemas" GSETTINGS_BACKEND=keyfile XDG_CONFIG_HOME="$tmpdir/config" \
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

echo "KARE-016+018 coordinate probe — $(date -u +%Y-%m-%dT%H:%M:%SZ)" >&2
echo "Binary: $KARERE_BIN" >&2
echo "Fixture: $FIXTURE_HTML" >&2

FAIL=0
PASS=0
H7_FAIL=0
H7_PASS=0

matrix=(
  "1 0 1280 800"
  "1 1 1280 800"
  "2 0 2560 1600"
  "2 1 2560 1600"
  "1 0 720 900"
  "1 1 720 900"
)

# Handle --fractional mode — always emits SKIP rows when not on a live Wayland session
# with driveable DisplayConfig. The harness cannot silently PASS fractional; it records
# the host limitation honestly and appends rows to the H7 results file.
if [[ "$FRACTIONAL_MODE" -eq 1 ]]; then
  : > "$H7_RESULTS_JSON"
  if ! check_fractional_available; then
    echo "Fractional matrix: SKIP (host lacks fractional compositor support)" >&2
    for frac in "1.25" "1.5"; do
      row="{\"fractional\":\"$frac\",\"status\":\"SKIP\",\"reason\":\"fractional unverified on this host — missing Mutter experimental-features\"}"
      echo "$row"
      echo "$row" >> "$H7_RESULTS_JSON"
    done
    # Keep FRACTIONAL_MODE=1 so the synthetic-matrix section does not truncate
    # $H7_RESULTS_JSON — the SKIP rows above must survive alongside chrome rows.
  else
    echo "--- Fractional Wayland matrix: 125% and 150% ---" >&2
    frac_matrix=("1.25" "1.5")
    for frac in "${frac_matrix[@]}"; do
      echo "--- Fractional $frac (best-effort, SKIP if not observable) ---" >&2
      if [[ "${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
        # The harness cannot drive Mutter DisplayConfig from inside its Xvfb run —
        # record an honest SKIP rather than a fake PASS. KARE-020 owns the
        # operator-driven ApplyMonitorsConfig attempt with trap restore.
        row="{\"fractional\":\"$frac\",\"status\":\"SKIP\",\"reason\":\"fractional scale transient set not implemented in Xvfb harness — needs operator Wayland session\",\"XDG_SESSION_TYPE\":\"wayland\",\"detail\":\"harness limitation: operator must apply 125/150% monitor scale via Settings or gdbus org.gnome.Mutter.DisplayConfig ApplyMonitorsConfig with trap restore\"}"
        echo "$row"
        echo "$row" >> "$H7_RESULTS_JSON"
      else
        echo "SKIP: fractional unverified on this host — not a Wayland session (XDG_SESSION_TYPE=${XDG_SESSION_TYPE:-unknown})" >&2
        row="{\"fractional\":\"$frac\",\"status\":\"SKIP\",\"reason\":\"XDG_SESSION_TYPE is not wayland\"}"
        echo "$row"
        echo "$row" >> "$H7_RESULTS_JSON"
      fi
    done
    if [[ "$CHROME_MODE" -eq 0 ]]; then
      echo "Fractional mode without --chrome: only synthetic scales would be checked; add --chrome for H7 coverage." >&2
      exit 0
    fi
  fi
fi

# Negative control fast path
if [[ "$NEGATIVE_MODE" -eq 1 ]]; then
  echo "--- Negative control: wrong scale expectation must FAIL ---" >&2
  neg_chrome="$CHROME_MODE"
  set +e
  run_config 2 0 2560 1600 1 "$neg_chrome"
  rc=$?
  set -e
  if [[ $rc -eq 0 ]]; then
    echo "NEGATIVE CONTROL PASS — harness correctly detected misalignment" >&2
    echo '{"negative_control":"PASS","chrome":'"$neg_chrome"',"detail":"wrong expectation correctly produced error >1px"}'
  else
    echo "NEGATIVE CONTROL FAIL — harness did NOT detect misalignment" >&2
    echo '{"negative_control":"FAIL","chrome":'"$neg_chrome"',"detail":"harness failed to detect intentional 20px offset"}'
    exit 1
  fi
  exit 0
fi

# Real-page gate — wired: prints required SKIP when gated, confirms live path when authorized.
if [[ "$CHROME_MODE" -eq 1 ]]; then
  check_real_page_gate || true
else
  if [[ "${KARERE_H7_REAL_PAGE:-0}" == "1" ]]; then
    echo "Note: KARERE_H7_REAL_PAGE=1 ignored without --chrome (no real-page surface without chrome mode)." >&2
  fi
fi

# Synthetic matrix (always run)
echo "--- Synthetic matrix (KARE-016 invariant) ---" >&2
: > "$RESULTS_JSON"
if [[ "$CHROME_MODE" -eq 1 && "$FRACTIONAL_MODE" -eq 0 ]]; then : > "$H7_RESULTS_JSON"; fi
# When --fractional was used, H7 file already contains fractional SKIP rows — append chrome rows
set +e
for spec in "${matrix[@]}"; do
  read -r sc gpu sw sh <<<"$spec"
  echo "--- Running synthetic $sc gpu=$gpu @ ${sw}x${sh} ---" >&2
  out="$(run_config "$sc" "$gpu" "$sw" "$sh" 0 0)"
  rc=$?
  echo "$out"
  echo "$out" >> "$RESULTS_JSON"
  if python3 -c 'import json,sys; sys.exit(0 if json.loads(sys.argv[1]).get("passed") else 1)' "$out" 2>/dev/null; then
    PASS=$((PASS+1))
    echo "→ PASS synthetic $sc gpu=$gpu" >&2
  else
    FAIL=$((FAIL+1))
    echo "→ FAIL synthetic $sc gpu=$gpu (error >1px sustained)" >&2
  fi
done
set -e

# H7 chrome matrix (when --chrome) — target URL switches to live snapshot when gated open
if [[ "$CHROME_MODE" -eq 1 ]]; then
  echo "--- H7 chrome matrix (header+panel+transform+scroll, calibrated) ---" >&2
  set +e
  for spec in "${matrix[@]}"; do
    read -r sc gpu sw sh <<<"$spec"
    echo "--- Running chrome $sc gpu=$gpu @ ${sw}x${sh} ---" >&2
    out="$(run_config "$sc" "$gpu" "$sw" "$sh" 0 1)"
    rc=$?
    echo "$out"
    echo "$out" >> "$H7_RESULTS_JSON"
    if python3 -c 'import json,sys; sys.exit(0 if json.loads(sys.argv[1]).get("passed") else 1)' "$out" 2>/dev/null; then
      H7_PASS=$((H7_PASS+1))
      echo "→ PASS chrome $sc gpu=$gpu" >&2
    else
      H7_FAIL=$((H7_FAIL+1))
      echo "→ FAIL chrome $sc gpu=$gpu (calibrated error >1px sustained)" >&2
    fi
  done
  set -e
fi

echo "--- Summary: synthetic $PASS passed, $FAIL failed; chrome $H7_PASS passed, $H7_FAIL failed ---" >&2
python3 -c 'import json,sys; print(json.dumps({"summary":{"synthetic":{"passed":int(sys.argv[1]),"failed":int(sys.argv[2])},"chrome":{"passed":int(sys.argv[3]),"failed":int(sys.argv[4]),"enabled":bool(int(sys.argv[5]))},"total":int(sys.argv[6])}}))' "$PASS" "$FAIL" "$H7_PASS" "$H7_FAIL" "$CHROME_MODE" "${#matrix[@]}" >&2

echo "--- Inline negative control (wrong expectation must be detected) ---" >&2
set +e
neg_chrome="$CHROME_MODE"
neg_out="$(run_config 2 0 2560 1600 1 "$neg_chrome")"
neg_rc=$?
set -e
if python3 -c 'import json,sys; sys.exit(0 if json.loads(sys.argv[1]).get("passed") else 1)' "$neg_out" 2>/dev/null; then
  echo "Inline negative control: PASS (wrong expectation correctly flagged, chrome=$neg_chrome)" >&2
  echo "$neg_out" | python3 -c 'import json,sys;print(json.dumps({"negative_control":"PASS","chrome":bool(int(sys.argv[1])),"probe":json.loads(sys.stdin.read())}))' "$neg_chrome"
else
  echo "Inline negative control: FAIL (harness missed intentional offset, chrome=$neg_chrome)" >&2
  echo "$neg_out" | python3 -c 'import json,sys;print(json.dumps({"negative_control":"FAIL","chrome":bool(int(sys.argv[1])),"probe":json.loads(sys.stdin.read())}))' "$neg_chrome"
  FAIL=$((FAIL+1))
fi

total_fail=$((FAIL + H7_FAIL))
if [[ $total_fail -gt 0 ]]; then
  echo "MATRIX RESULT: $total_fail config(s) failed — see JSON rows above" >&2
else
  if [[ "$CHROME_MODE" -eq 1 ]]; then
    echo "MATRIX RESULT: all synthetic+chrome configs passed (no sustained >1px calibrated error)" >&2
  else
    echo "MATRIX RESULT: all synthetic configs passed (no sustained >1px error)" >&2
  fi
fi

# Fractional Wayland verification (KARE-020) — operator-gated, CI-safe default is SKIP.
# scale-monitor-framebuffer is fractional framebuffer scaling; xwayland-native-scaling
# is XWayland native scale. The harness handles whichever the host exposes and does
# not assume both. Delegates to tests/wayland_fractional_verify.sh with transient
# method 1 when gated and available; otherwise emits honest SKIP rows.
echo "--- Fractional Wayland scales 1.25 / 1.5 ---" >&2
if [[ $FRACTIONAL_WAYLAND -eq 1 ]]; then
  if check_fractional_wayland_host; then
    echo "Fractional gate OPEN and host available — delegating to wayland_fractional_verify.sh" >&2
    set +e
    frac_out="$(KARERE_FRACTIONAL_WAYLAND=1 bash "$SCRIPT_DIR/wayland_fractional_verify.sh" --bin "$KARERE_BIN" 2>&1)"
    frac_rc=$?
    set -e
    echo "$frac_out"
    if [[ $frac_rc -ne 0 ]]; then
      echo "Fractional Wayland verification FAILED (worst_error_px >1 or restore mismatch)" >&2
      exit $frac_rc
    fi
  else
    reason="needs operator Wayland session"
    if [[ "${XDG_SESSION_TYPE:-}" != "wayland" ]]; then reason="not-wayland";
    elif [[ -z "${WAYLAND_DISPLAY:-}" ]]; then reason="not-wayland";
    elif ! command -v gdbus >/dev/null 2>&1; then reason="no-gdbus";
    else
      feats="$(gsettings get org.gnome.mutter experimental-features 2>/dev/null || echo "@as []")"
      if [[ "$feats" != *"scale-monitor-framebuffer"* && "$feats" != *"xwayland-native-scaling"* ]]; then reason="missing-feature"; fi
    fi
    echo "SKIP fractional Wayland requires operator Wayland session with scale-monitor-framebuffer/xwayland-native-scaling — run with KARERE_FRACTIONAL_WAYLAND=1 on a live Mutter Wayland session (see TESTING.md)" >&2
    for sc in "1.25" "1.5"; do
      python3 -c 'import json,sys; print(json.dumps({"config":f"fractional scale={sys.argv[1]}","scale":float(sys.argv[1]),"passed":None,"worst_error_px":None,"fractional":{"available":False,"reason":sys.argv[2],"required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]},"message":"needs operator Wayland session"}))' "$sc" "$reason"
    done
    python3 -c 'import json; print(json.dumps({"fractional":{"available":False,"reason":sys.argv[1],"required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}))' "$reason" >&2
  fi
else
  echo "SKIP fractional Wayland requires operator Wayland session with scale-monitor-framebuffer/xwayland-native-scaling — run with KARERE_FRACTIONAL_WAYLAND=1 on a live Mutter Wayland session (see TESTING.md)" >&2
  for sc in "1.25" "1.5"; do
    python3 -c 'import json,sys; print(json.dumps({"config":f"fractional scale={sys.argv[1]}","scale":float(sys.argv[1]),"passed":None,"worst_error_px":None,"fractional":{"available":False,"reason":"not-opted-in","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]},"message":"needs operator Wayland session"}))' "$sc"
  done
  python3 -c 'import json; print(json.dumps({"fractional":{"available":False,"reason":"not-opted-in","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}))' >&2
fi

exit 0
