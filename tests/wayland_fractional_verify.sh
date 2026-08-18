#!/usr/bin/env bash
# KARE-020 — Operator-gated fractional Wayland verification.
# Applies transient 125%/150% logical monitor scales via Mutter DisplayConfig
# (method 1 transient) with EXIT/INT/TERM trap restore, re-runs synthetic+chrome
# probe through the production GDK→GTK→CEF→page path, validates |clientX-expected|<=1px.
#
# Usage:
#   KARERE_FRACTIONAL_WAYLAND=1 bash tests/wayland_fractional_verify.sh [--bin PATH] [--chrome-only|--synthetic-only] [--help]
#   bash tests/wayland_fractional_verify.sh --fractional-wayland [--bin PATH]
#
# Default without opt-in → SKIP JSON exit 0, no display mutation (CI-safe).
# Requires live GNOME/Mutter Wayland session with scale-monitor-framebuffer or
# xwayland-native-scaling in org.gnome.mutter experimental-features.
# See TESTING.md "Fractional Wayland — operator-driven verification (KARE-020)".
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE_HTML="$REPO_ROOT/tests/fixtures/coord-probe.html"
FIXTURE_CHROME="$REPO_ROOT/tests/fixtures/coord-probe-whatsapp.html"

KARERE_BIN="${KARERE_BIN:-}"
FRACTIONAL_OPTIN="${KARERE_FRACTIONAL_WAYLAND:-}"
CHROME_ONLY=0
SYNTHETIC_ONLY=0
HELP=0
EXTRA_BIN=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin) EXTRA_BIN="$2"; KARERE_BIN="$2"; shift 2;;
    --fractional-wayland) FRACTIONAL_OPTIN=1; shift;;
    --chrome-only) CHROME_ONLY=1; shift;;
    --synthetic-only) SYNTHETIC_ONLY=1; shift;;
    --help|-h) HELP=1; shift;;
    *) echo "Unknown arg: $1" >&2; exit 2;;
  esac
done

if [[ $HELP -eq 1 ]]; then
  cat <<'EOF'
Usage: tests/wayland_fractional_verify.sh [--bin PATH] [--chrome-only|--synthetic-only] [--fractional-wayland] [--help]

Operator-gated fractional Wayland verification (KARE-020).
Requires KARERE_FRACTIONAL_WAYLAND=1 or --fractional-wayland, plus a live
GNOME/Mutter Wayland session with scale-monitor-framebuffer and/or
xwayland-native-scaling. Applies transient 125%/150% logical scales via
gdbus org.gnome.Mutter.DisplayConfig ApplyMonitorsConfig method 1 (temporary)
with trap restore, re-runs the synthetic+chrome probe, validates |clientX-expected|<=1px.

Without opt-in or on unsuitable hosts: emits SKIP JSON and exits 0 without touching display.
Also SKIPs with reason "cdp-port-held" when CDP port 9333 is already in use (a running
Karere started with --debug owns it; the probe cannot bind its own port since
DEVTOOLS_PORT is a fixed constant in src/devtools.rs).

Environment: KARERE_BIN, KARERE_FRACTIONAL_WAYLAND
Prerequisites: gdbus, gsettings, python3 (with gi/Gio), xdotool, curl (optional CDP),
Wayland session, CDP port 9333 free (close any running Karere started with --debug).
EOF
  exit 0
fi

if [[ -n "$EXTRA_BIN" ]]; then
  # Resolve to absolute for python helper (cwd may change)
  if [[ "$EXTRA_BIN" != /* ]] && [[ "$EXTRA_BIN" != flatpak* ]]; then
    KARERE_BIN="$(realpath -m "$REPO_ROOT/$EXTRA_BIN" 2>/dev/null || echo "$REPO_ROOT/$EXTRA_BIN")"
  else
    KARERE_BIN="$EXTRA_BIN"
  fi
fi
if [[ -z "$KARERE_BIN" ]]; then
  if [[ -x "$REPO_ROOT/target/debug/karere" ]]; then
    KARERE_BIN="$REPO_ROOT/target/debug/karere"
  else
    KARERE_BIN="$REPO_ROOT/target/debug/karere"
  fi
fi

TMP_STATE=""
RESTORED=0
SAVED_BEFORE_SCALE=""

cleanup_restore() {
  if [[ $RESTORED -eq 1 ]]; then return 0; fi
  if [[ -z "$TMP_STATE" || ! -f "$TMP_STATE" ]]; then return 0; fi
  local restore_rc=0
  python3 <<'PY' || restore_rc=$?
import os, sys
import json, pathlib
try:
    import gi
    gi.require_version('Gio','2.0')
    from gi.repository import Gio, GLib
    state_path=os.environ.get("TMP_STATE","")
    if not state_path or not os.path.exists(state_path):
        sys.exit(0)
    data=json.loads(pathlib.Path(state_path).read_text())
    lms=data["logical_monitors"]
    bus=Gio.bus_get_sync(Gio.BusType.SESSION, None)
    # Fresh serial required: Mutter bumps serial after every ApplyMonitorsConfig
    res=bus.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','GetCurrentState',None,None,Gio.DBusCallFlags.NONE, -1, None)
    fresh_serial=res.get_child_value(0).get_uint32()
    monitors=res.get_child_value(1)
    mode_by_conn={}
    for mi in range(monitors.n_children()):
        mon=monitors.get_child_value(mi)
        conn_tuple=mon.get_child_value(0)
        conn=conn_tuple.get_child_value(0).get_string()
        modes=mon.get_child_value(1)
        cur=""
        for mj in range(modes.n_children()):
            md=modes.get_child_value(mj)
            props=md.get_child_value(6)
            is_cur=False
            for pk in range(props.n_children()):
                k=props.get_child_value(pk).get_child_value(0).get_string()
                if k=="is-current":
                    is_cur=props.get_child_value(pk).get_child_value(1).get_variant().get_boolean()
            if is_cur:
                cur=md.get_child_value(0).get_string()
                break
        if not cur and modes.n_children()>0:
            cur=modes.get_child_value(0).get_child_value(0).get_string()
        mode_by_conn[conn]=cur
    lm_list=[]
    for lm in lms:
        mons=[(mm["connector"], mode_by_conn.get(mm["connector"], ""), {}) for mm in lm["monitors"]]
        lm_list.append((lm["x"], lm["y"], float(lm["scale"]), int(lm["transform"]), bool(lm["primary"]), mons))
    variant=GLib.Variant('(uua(iiduba(ssa{sv}))a{sv})', (int(fresh_serial), 1, lm_list, {}))
    bus.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','ApplyMonitorsConfig', variant, None, Gio.DBusCallFlags.NONE, -1, None)
    print("restore: reapplied saved configuration", file=sys.stderr)
except Exception as e:
    import traceback; traceback.print_exc()
    print(f"restore failed: {e}", file=sys.stderr)
    sys.exit(1)
PY
  restore_rc=${restore_rc:-0}
  RESTORED=1
  return "$restore_rc"
}

trap 'cleanup_restore' EXIT INT TERM

emit_skip() {
  local reason="$1"
  python3 -c '
import json,sys
reason=sys.argv[1]
print(json.dumps({"fractional":{"available":False,"reason":reason,"required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}))
' "$reason"
}

check_fractional_wayland_available() {
  if [[ -z "$FRACTIONAL_OPTIN" ]]; then
    emit_skip "not-opted-in" >&2
    echo '{"fractional":{"available":false,"reason":"not-opted-in","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  if [[ "$(id -u)" -eq 0 ]]; then
    echo "Refusing to run as root (would mutate display config)" >&2
    emit_skip "permission-denied" >&2
    echo '{"fractional":{"available":false,"reason":"permission-denied","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  if [[ "${XDG_SESSION_TYPE:-}" != "wayland" ]]; then
    emit_skip "not-wayland" >&2
    echo '{"fractional":{"available":false,"reason":"not-wayland","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
    emit_skip "not-wayland" >&2
    echo '{"fractional":{"available":false,"reason":"not-wayland","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  # Detect an Xvfb-owned DISPLAY. A login shell on Wayland exports
  # XDG_SESSION_TYPE=wayland/WAYLAND_DISPLAY even under `xvfb-run`, so session
  # type alone cannot detect the CI case — check whether the current display
  # number is owned by an Xvfb process (real XWayland is owned by Xwayland).
  if [[ -n "${DISPLAY:-}" ]] && pgrep -a Xvfb >/dev/null 2>&1; then
    local disp_num="${DISPLAY%%.*}"
    disp_num="${disp_num#:}"
    if [[ -n "$disp_num" ]] && pgrep -a Xvfb | grep -q -- ":$disp_num"; then
      emit_skip "xvfb" >&2
      echo '{"fractional":{"available":false,"reason":"xvfb","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
      return 1
    fi
  fi
  if ! command -v gdbus >/dev/null 2>&1; then
    emit_skip "no-gdbus" >&2
    echo '{"fractional":{"available":false,"reason":"no-gdbus","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  # Check GetCurrentState succeeds
  if ! gdbus call --session --dest org.gnome.Mutter.DisplayConfig --object-path /org/gnome/Mutter/DisplayConfig --method org.gnome.Mutter.DisplayConfig.GetCurrentState >/dev/null 2>&1; then
    emit_skip "no-displayconfig" >&2
    echo '{"fractional":{"available":false,"reason":"no-displayconfig","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  # Check ApplyMonitorsConfigAllowed
  local allowed
  allowed="$(gdbus call --session --dest org.gnome.Mutter.DisplayConfig --object-path /org/gnome/Mutter/DisplayConfig --method org.freedesktop.DBus.Properties.Get 'org.gnome.Mutter.DisplayConfig' 'ApplyMonitorsConfigAllowed' 2>/dev/null | tr -d "[:space:]" || true)"
  if [[ "$allowed" == *"false"* ]]; then
    emit_skip "permission-denied" >&2
    echo '{"fractional":{"available":false,"reason":"permission-denied","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  local feats
  feats="$(gsettings get org.gnome.mutter experimental-features 2>/dev/null || echo "@as []")"
  if [[ "$feats" != *"scale-monitor-framebuffer"* && "$feats" != *"xwayland-native-scaling"* ]]; then
    emit_skip "missing-feature" >&2
    echo '{"fractional":{"available":false,"reason":"missing-feature","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"]}}'
    return 1
  fi
  return 0
}

save_current_state() {
  TMP_STATE="$(mktemp -t kare-fractional-state-XXXXXX.json)"
  export TMP_STATE
  python3 <<'PY'
import os, json, sys
try:
    import gi
    gi.require_version('Gio','2.0')
    from gi.repository import Gio
    bus=Gio.bus_get_sync(Gio.BusType.SESSION, None)
    res=bus.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','GetCurrentState',None,None,Gio.DBusCallFlags.NONE, -1, None)
    serial=res.get_child_value(0).get_uint32()
    logical=res.get_child_value(2)
    lms=[]
    for i in range(logical.n_children()):
        lm=logical.get_child_value(i)
        x=lm.get_child_value(0).get_int32()
        y=lm.get_child_value(1).get_int32()
        scale=lm.get_child_value(2).get_double()
        trans=lm.get_child_value(3).get_uint32()
        primary=lm.get_child_value(4).get_boolean()
        mons_v=lm.get_child_value(5)
        mons=[]
        for j in range(mons_v.n_children()):
            mc=mons_v.get_child_value(j)
            mons.append({"connector": mc.get_child_value(0).get_string(), "vendor": mc.get_child_value(1).get_string(), "product": mc.get_child_value(2).get_string(), "serial": mc.get_child_value(3).get_string()})
        lms.append({"x": x, "y": y, "scale": scale, "transform": trans, "primary": primary, "monitors": mons})
    out={"serial": serial, "logical_monitors": lms}
    path=os.environ["TMP_STATE"]
    open(path,'w').write(json.dumps(out))
    print(f"save: serial={serial} primary scale={lms[0]['scale'] if lms else 'n/a'}", file=sys.stderr)
except Exception as e:
    import traceback; traceback.print_exc()
    sys.exit(1)
PY
  SAVED_BEFORE_SCALE="$(python3 -c 'import json,pathlib,os; lms=json.loads(pathlib.Path(os.environ["TMP_STATE"]).read_text())["logical_monitors"]; print(([lm for lm in lms if lm["primary"]] or lms)[0]["scale"])' 2>/dev/null || echo "1.0")"
  echo "Pre-state primary scale: $SAVED_BEFORE_SCALE" >&2
}

apply_scale() {
  local target="$1"
  python3 <<PY 2>&1
import os, sys, json, pathlib
target=float("$target")
try:
    import gi
    gi.require_version('Gio','2.0')
    from gi.repository import Gio, GLib
    path=os.environ.get("TMP_STATE","")
    data=json.loads(pathlib.Path(path).read_text())
    lms=data["logical_monitors"]
    bus=Gio.bus_get_sync(Gio.BusType.SESSION, None)
    # Fresh serial: serial from TMP_STATE is stale after prior ApplyMonitorsConfig
    res=bus.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','GetCurrentState',None,None,Gio.DBusCallFlags.NONE, -1, None)
    serial=res.get_child_value(0).get_uint32()
    # Build connector->current mode id map (Apply expects (connector, modeId, props) not vendor)
    monitors=res.get_child_value(1)
    mode_by_conn={}
    for mi in range(monitors.n_children()):
        mon=monitors.get_child_value(mi)
        conn_tuple=mon.get_child_value(0)
        conn=conn_tuple.get_child_value(0).get_string()
        modes=mon.get_child_value(1)
        cur=""
        for mj in range(modes.n_children()):
            md=modes.get_child_value(mj)
            props=md.get_child_value(6)
            is_cur=False
            for pk in range(props.n_children()):
                k=props.get_child_value(pk).get_child_value(0).get_string()
                if k=="is-current":
                    is_cur=props.get_child_value(pk).get_child_value(1).get_variant().get_boolean()
            if is_cur:
                cur=md.get_child_value(0).get_string()
                break
        if not cur and modes.n_children()>0:
            cur=modes.get_child_value(0).get_child_value(0).get_string()
        mode_by_conn[conn]=cur
    lm_list=[]
    for lm in lms:
        scale = target if lm["primary"] else float(lm["scale"])
        mons=[(m["connector"], mode_by_conn.get(m["connector"], ""), {}) for m in lm["monitors"]]
        lm_list.append((lm["x"], lm["y"], scale, int(lm["transform"]), bool(lm["primary"]), mons))
    # Outer is (u serial, u method, a(logical_monitors), a{sv} properties) where logical_monitor is (i,i,d,u,b,a(ssa{sv})) 6-tuple
    variant=GLib.Variant('(uua(iiduba(ssa{sv}))a{sv})', (int(serial), 1, lm_list, {}))
    bus.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','ApplyMonitorsConfig', variant, None, Gio.DBusCallFlags.NONE, -1, None)
    print(f"apply: requested scale {target}", file=sys.stderr)
except Exception as e:
    import traceback; traceback.print_exc()
    # Fallback: try via gdbus CLI — build 7-child struct with a(ssa{sv}) monitors
    try:
        import subprocess
        path=os.environ.get("TMP_STATE","")
        data=json.loads(pathlib.Path(path).read_text())
        import gi
        gi.require_version('Gio','2.0')
        from gi.repository import Gio
        b=Gio.bus_get_sync(Gio.BusType.SESSION, None)
        r=b.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','GetCurrentState',None,None,Gio.DBusCallFlags.NONE, -1, None)
        serial=r.get_child_value(0).get_uint32()
        primary_conn=lms[0]["monitors"][0]["connector"] if lms and lms[0]["monitors"] else "HDMI-1"
        # gdbus textual form: 6-tuple (x, y, scale, transform, primary, monitors) per ApplyMonitorsConfig a(iiduba(ssa{sv}))
        lm_str=f'[ (0, 0, {target}, uint32 0, true, [("{primary_conn}", "", @a{{sv}} {{}})]) ]'
        subprocess.check_call(["gdbus","call","--session","--dest","org.gnome.Mutter.DisplayConfig","--object-path","/org/gnome/Mutter/DisplayConfig","--method","org.gnome.Mutter.DisplayConfig.ApplyMonitorsConfig", str(serial), "1", lm_str, "@a{sv} {}"])
        print(f"apply fallback gdbus succeeded scale {target}", file=sys.stderr)
    except Exception as e2:
        print(f"apply fallback failed: {e2}", file=sys.stderr)
        sys.exit(1)
PY
  sleep 1.5
  # Verify
  local cur
  cur="$(python3 <<'PY' 2>/dev/null
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio
bus=Gio.bus_get_sync(Gio.BusType.SESSION, None)
res=bus.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','GetCurrentState',None,None,Gio.DBusCallFlags.NONE, -1, None)
logical=res.get_child_value(2)
for i in range(logical.n_children()):
    lm=logical.get_child_value(i)
    if lm.get_child_value(4).get_boolean():
        print(lm.get_child_value(2).get_double())
        break
PY
)"
  echo "Post-apply primary scale: $cur (requested $target)" >&2
}

get_current_primary_scale() {
  python3 <<'PY' 2>/dev/null
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio
bus=Gio.bus_get_sync(Gio.BusType.SESSION, None)
res=bus.call_sync('org.gnome.Mutter.DisplayConfig','/org/gnome/Mutter/DisplayConfig','org.gnome.Mutter.DisplayConfig','GetCurrentState',None,None,Gio.DBusCallFlags.NONE, -1, None)
logical=res.get_child_value(2)
for i in range(logical.n_children()):
    lm=logical.get_child_value(i)
    if lm.get_child_value(4).get_boolean():
        print(lm.get_child_value(2).get_double())
        break
PY
}

run_probe_at_scale() {
  local scale="$1"
  local fixture="$2"
  local label="$3"
  local gpu_osr="${4:-0}"
  # Reuse coordinate_probe's probe logic but on the live display (no Xvfb).
  # For now, delegate to a minimal inline probe that validates title/CDP readback.
  # If fixture missing, fall back to synthetic.
  if [[ ! -f "$fixture" ]]; then
    fixture="$FIXTURE_HTML"
  fi
  local data_url
  data_url="$(base64 -w0 "$fixture" | sed 's/^/data:text\/html;base64,/')"
  # Use python helper to launch and probe; pass REPO_ROOT and gpu flag
  REPO_ROOT="$REPO_ROOT" python3 - "$scale" "$data_url" "$KARERE_BIN" "$label" "$gpu_osr" <<'PY'
import os, sys, json, subprocess, time, pathlib, signal, shlex, shutil
scale=float(sys.argv[1])
data_url=sys.argv[2]
karere_bin=sys.argv[3]
label=sys.argv[4]

tmpdir=pathlib.Path(subprocess.check_output(["mktemp","-d","-t","kare-frac-probe-XXXXXX"], text=True).strip())
schemas=tmpdir/"schemas"
schemas.mkdir(parents=True, exist_ok=True)
config=tmpdir/"config"
config.mkdir(parents=True, exist_ok=True)
cache=tmpdir/"cache"
cache.mkdir(parents=True, exist_ok=True)
data_home=tmpdir/"data"
data_home.mkdir(parents=True, exist_ok=True)
stderr_log=tmpdir/"stderr.log"

# Generate schemas
import shutil
repo_root=pathlib.Path(os.environ.get("REPO_ROOT","")) if os.environ.get("REPO_ROOT") else pathlib.Path(__file__).resolve().parent if len(pathlib.Path(__file__).parts)>1 else pathlib.Path(".")
if not (repo_root/"data/io.github.tobagin.karere.gschema.xml.in").exists():
    for p in [pathlib.Path.cwd(), pathlib.Path(repo_root)]:
        cand=p/"data/io.github.tobagin.karere.gschema.xml.in"
        if cand.exists():
            repo_root=p
            break
src=repo_root/"data/io.github.tobagin.karere.gschema.xml.in"
app_id="io.github.tobagin.karere"
if "Devel" in karere_bin:
    app_id="io.github.tobagin.karere.Devel"
txt=src.read_text()
txt=txt.replace("@APP_ID@", app_id).replace("@APP_PATH@", "/io/github/tobagin/karere/")
(schemas/f"{app_id}.gschema.xml").write_text(txt)
subprocess.run(["glib-compile-schemas", str(schemas)], check=True)

# Detect env
is_flatpak="flatpak" in karere_bin
env=dict(os.environ)
env["GSETTINGS_SCHEMA_DIR"]=str(schemas)
env["GSETTINGS_BACKEND"]="keyfile"
env["XDG_CONFIG_HOME"]=str(config)
env["XDG_CACHE_HOME"]=str(cache)
env["XDG_DATA_HOME"]=str(data_home)
env["GSK_RENDERER"]="gl"
env["KARERE_GPU_OSR"]=sys.argv[5] if len(sys.argv)>5 else "0"
# REPO_ROOT passed via env for schema lookup
# label already in sys.argv[4], gpu flag in sys.argv[5]

# Set gsettings window size
subprocess.run(["gsettings","set",app_id,"window-width","1280"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
subprocess.run(["gsettings","set",app_id,"window-height","800"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
subprocess.run(["gsettings","set",app_id,"start-in-background","false"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

# CDP readback of window.__karereProbe.last (mirrors cdp_evaluate_probe in
# tests/coordinate_probe.sh). The app never propagates the CEF page title to the
# GTK window title (on_title_change only stores/logs it), so the xdotool title
# channel never carries JSON — CDP is the effective readback path on every host.
def cdp_eval():
    try:
        import urllib.request, json as j
        body=urllib.request.urlopen("http://127.0.0.1:9333/json/list", timeout=2).read().decode()
        arr=j.loads(body)
        ws=""
        for o in arr:
            if o.get("webSocketDebuggerUrl"):
                ws=o["webSocketDebuggerUrl"]; break
        if not ws:
            return ""
        payload=j.dumps({"id":1,"method":"Runtime.evaluate","params":{"expression":"JSON.stringify(window.__karereProbe.last)"}})
        try:
            import websocket
            conn=websocket.create_connection(ws, timeout=2)
            conn.send(payload)
            resp=conn.recv()
            data=j.loads(resp)
            val=data.get("result",{}).get("result",{}).get("value")
            conn.close()
            if val:
                return val
        except Exception:
            pass
        if shutil.which("websocat"):
            try:
                proc=subprocess.run(["websocat","-n1",ws], input=payload, capture_output=True, text=True, timeout=3)
                out=proc.stdout.strip()
                if out:
                    data=j.loads(out)
                    val=data.get("result",{}).get("result",{}).get("value")
                    if val:
                        return val
            except Exception:
                pass
    except Exception:
        pass
    return ""

# Launch
stderr_f=open(stderr_log,"w")
proc=None
probe_pid=None
pidfile=tmpdir/"karere.pid"
dbus_run=shutil.which("dbus-run-session")
try:
    if is_flatpak:
        cmd=["flatpak","run","--env=GDK_BACKEND=x11","--env=GSK_RENDERER=gl",f"--env=KARERE_GPU_OSR={env['KARERE_GPU_OSR']}"]+karere_bin.split()[2:]+["--url",data_url,"--debug","--debuglevel=debug"]
        # karere_bin is like "flatpak run io.github.tobagin.karere.Devel".
        # dbus-run-session gives the probe its own session bus: on a live desktop a
        # Karere instance is often already running, and single-instance arbitration
        # (src/main.rs is_remote()) would make this launch a secondary that presents
        # the operator's window and exits before CEF init — no CDP, no probe window.
        # start_new_session puts the whole probe tree in its own process group so
        # cleanup killpg can never reach the wrapper or any operator process.
        if dbus_run:
            proc=subprocess.Popen(["dbus-run-session","--"]+cmd, env=env, stdout=stderr_f, stderr=subprocess.STDOUT, start_new_session=True)
        else:
            proc=subprocess.Popen(cmd, env=env, stdout=stderr_f, stderr=subprocess.STDOUT, start_new_session=True)
    else:
        import pathlib as pl
        bin_dir=str(pl.Path(karere_bin).parent)
        env["LD_LIBRARY_PATH"]=bin_dir+ (":"+env["LD_LIBRARY_PATH"] if "LD_LIBRARY_PATH" in env else "")
        env["GDK_BACKEND"]="x11"
        env["DISPLAY"]=os.environ.get("DISPLAY","")
        # Keep WAYLAND_DISPLAY for wayland but force x11 for probe window (XWayland).
        # See flatpak note above for the dbus-run-session isolation and
        # start_new_session for the cleanup-safe process group. sh execs karere
        # with the same pid, so the pidfile identifies the app for a pid-scoped
        # xdotool search (an operator's Karere window also matches the "Karere" name).
        inner=shlex.quote(karere_bin)+" --url "+shlex.quote(data_url)+" --debug --debuglevel=debug"
        shcmd=f"echo $$ > {shlex.quote(str(pidfile))}; exec {inner}"
        if dbus_run:
            proc=subprocess.Popen(["dbus-run-session","--","sh","-c",shcmd], env=env, stdout=stderr_f, stderr=subprocess.STDOUT, cwd=bin_dir, start_new_session=True)
        else:
            proc=subprocess.Popen(["sh","-c",shcmd], env=env, stdout=stderr_f, stderr=subprocess.STDOUT, cwd=bin_dir, start_new_session=True)
    time.sleep(3.0)
    # Find window — prefer the probe's own pid when known
    for _ in range(20):
        try:
            probe_pid=int(pidfile.read_text().strip())
            break
        except Exception:
            time.sleep(0.2)
    wid=""
    for _ in range(60):
        if probe_pid:
            try:
                out=subprocess.check_output(["xdotool","search","--onlyvisible","--pid",str(probe_pid)], text=True, stderr=subprocess.DEVNULL).strip()
                if out:
                    wid=out.splitlines()[0].strip()
                    break
            except: pass
        try:
            out=subprocess.check_output(["xdotool","search","--onlyvisible","--name","Karere"], text=True, stderr=subprocess.DEVNULL).strip()
            if out:
                wid=out.splitlines()[0].strip()
                break
        except: pass
        try:
            out=subprocess.check_output(["xdotool","search","--onlyvisible","--classname","karere"], text=True, stderr=subprocess.DEVNULL).strip()
            if out:
                wid=out.splitlines()[0].strip()
                break
        except: pass
        time.sleep(0.2)
    if not wid:
        print(json.dumps({"config":label,"scale":scale,"passed":False,"worst_error_px":999,"error":"window not found","probes":[]}))
        sys.exit(0)
    subprocess.run(["xdotool","windowmove",wid,"0","0"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.3)
    subprocess.run(["xdotool","windowactivate","--sync",wid], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.4)
    # Wait for probe readiness — title JSON never appears (app does not propagate
    # the page title to the window title), so also poll CDP for probe readiness.
    for _ in range(30):
        try:
            title=subprocess.check_output(["xdotool","getwindowname",wid], text=True, stderr=subprocess.DEVNULL).strip()
            if title.startswith("{"):
                break
        except: pass
        if cdp_eval():
            break
        time.sleep(0.2)
    # Get geometry
    geom={}
    try:
        out=subprocess.check_output(["xdotool","getwindowgeometry","--shell",wid], text=True, stderr=subprocess.DEVNULL)
        for line in out.splitlines():
            if "=" in line:
                k,v=line.split("=",1)
                geom[k]=v
    except: pass
    X=int(geom.get("X","0")); Y=int(geom.get("Y","0"))
    guess_header=48
    # Calibration: use integer scale ceil for screen offset (GTK scale factor is ceil logical).
    # GtkGLArea framebuffer is always integer scale_factor (ceil), so physical = logical * ceil.
    import math
    int_scale=int(math.ceil(scale))
    if int_scale<1:
        int_scale=1
    calib_logical=200
    calib_screen_x=X+20*int_scale+int(calib_logical*int_scale)
    calib_screen_y=Y+guess_header*int_scale+int(calib_logical*int_scale)
    subprocess.run(["xdotool","mousemove","--sync",str(calib_screen_x),str(calib_screen_y)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.15)
    subprocess.run(["xdotool","click","1"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.4)
    calib_json=""
    try:
        title=subprocess.check_output(["xdotool","getwindowname",wid], text=True, stderr=subprocess.DEVNULL).strip()
        if title.startswith("{"):
            calib_json=title
    except: pass
    if not calib_json:
        calib_json=cdp_eval()
    ox,oy=0,0
    if calib_json and calib_json.startswith("{"):
        try:
            d=json.loads(calib_json)
            if isinstance(d, dict):
                ox=int(d.get("x",0)); oy=int(d.get("y",0))
        except: pass
        origin_x=calib_screen_x - int(ox*int_scale)
        origin_y=calib_screen_y - int(oy*int_scale)
    else:
        origin_x=X+1*int_scale
        origin_y=Y+guess_header*int_scale
    logical_centers=[(100,100),(250,150),(550,250),(650,350)]
    worst=0
    rows=[]
    passed=True
    for (lcx,lcy) in logical_centers:
        exp_x,exp_y=lcx,lcy
        screen_x=origin_x+int(lcx*int_scale)
        screen_y=origin_y+int(lcy*int_scale)
        subprocess.run(["xdotool","mousemove","--sync",str(screen_x),str(screen_y)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.12)
        subprocess.run(["xdotool","click","1"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(0.35)
        obs_json=""
        try:
            title=subprocess.check_output(["xdotool","getwindowname",wid], text=True, stderr=subprocess.DEVNULL).strip()
            if title.startswith("{"):
                obs_json=title
        except: pass
        if not obs_json:
            obs_json=cdp_eval()
        if not obs_json:
            obs_json='{"x":null,"y":null,"innerW":null,"innerH":null,"dpr":null}'
        try:
            obs=json.loads(obs_json)
            if not isinstance(obs, dict):
                obs={"x":None,"y":None}
        except:
            obs={"x":None,"y":None}
        obs_x=obs.get("x") if isinstance(obs, dict) else None; obs_y=obs.get("y") if isinstance(obs, dict) else None
        if obs_x is None or obs_y is None:
            err_x=None; err_y=None; abs_err=999; passed=False
            if abs_err>worst:
                worst=abs_err
        else:
            err_x=obs_x-exp_x; err_y=obs_y-exp_y
            abs_err=max(abs(err_x),abs(err_y))
            if abs_err>1:
                passed=False
            if abs_err>worst:
                worst=abs_err
        rows.append({"logical":f"{lcx}:{lcy}","expected":{"x":exp_x,"y":exp_y},"observed":{"x":obs_x,"y":obs_y},"error":{"x":err_x,"y":err_y},"metrics":obs})
    # viewport from first row
    viewport=rows[0]["metrics"] if rows else {}
    # joint logs tail
    try:
        stderr_f.flush()
        joint=pathlib.Path(stderr_log).read_text(errors="replace")[-2000:]
    except:
        joint=""
    print(json.dumps({"config":label,"scale":scale,"passed":passed,"worst_error_px":worst,"probes":rows,"viewport":viewport,"joint_logs_tail":joint[-2000:]}))
finally:
    # Quit app. Never use `gapplication action <app-id> quit` here: it targets the
    # LIVE session bus and would quit the operator's running Karere instance.
    try:
        if probe_pid:
            os.kill(probe_pid, signal.SIGTERM)
    except Exception:
        pass
    time.sleep(0.8)
    # Terminate the dbus-run-session wrapper which owns the bus and CEF children.
    # The probe tree lives in its own session (start_new_session=True), so killpg
    # reaches exactly the wrapper + dbus-daemon + karere + CEF children — never the
    # harness itself or any operator process. Never `fuser -k 9333/tcp`: that kills
    # ANY process on the fixed CDP port, including an operator's live Karere.
    if proc and proc.poll() is None:
        try:
            os.killpg(proc.pid, signal.SIGTERM)
            time.sleep(0.8)
            if proc.poll() is None:
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except Exception:
                    pass
                proc.kill()
            # Give kernel time to release 9333
            time.sleep(1.0)
        except Exception:
            pass
    try:
        stderr_f.close()
    except: pass
    shutil.rmtree(str(tmpdir), ignore_errors=True)
    # Wait for port to be free before next probe iteration
    for _ in range(20):
        try:
            import socket
            s=socket.socket(); s.settimeout(0.2)
            s.connect(("127.0.0.1",9333))
            s.close()
            time.sleep(0.25)
        except Exception:
            break
PY
}

# Main
if ! check_fractional_wayland_available; then
  exit 0
fi

# CDP readback gate — DEVTOOLS_PORT (9333) is a fixed constant (src/devtools.rs),
# so a host Karere started with --debug would own the port: the probe's CEF cannot
# bind its own debugger, Runtime.evaluate hits the WRONG process, observations come
# back null, and every row would spurious-FAIL with worst_error_px=999. Refuse up
# front with a structured reason instead of running a doomed probe.
if python3 -c '
import socket, sys
s = socket.socket()
s.settimeout(1.0)
try:
    s.connect(("127.0.0.1", 9333))
except OSError:
    sys.exit(1)  # refused -> port free
sys.exit(0)      # connected -> port held
'; then
  echo "SKIP fractional Wayland: CDP port 9333 is already held (close any running Karere started with --debug) — probe readback would target the wrong process" >&2
  python3 -c '
import json
print(json.dumps({"fractional":{"available":False,"reason":"cdp-port-held","required_features":["scale-monitor-framebuffer","xwayland-native-scaling"],"message":"CDP port 9333 held by another process (running Karere with --debug?); close it and re-run"}}))
'
  exit 0
fi

save_current_state

scales="1.25 1.5"
failed=0
passed=0

# Build probe list per scale
for sc in $scales; do
  echo "--- Fractional verify scale $sc ---" >&2
  apply_scale "$sc"
  # Synthetic probe: exercise both CPU and GPU OSR paint paths per Surface Enumeration
  if [[ $CHROME_ONLY -eq 0 ]]; then
    for gpu in 0 1; do
      gpu_label="cpu-osr"; [[ "$gpu" == "1" ]] && gpu_label="gpu-osr"
      out="$(run_probe_at_scale "$sc" "$FIXTURE_HTML" "fractional synthetic $gpu_label scale=$sc" "$gpu")"
      echo "$out"
      is_pass="$(python3 -c 'import json,sys; print("1" if json.loads(sys.argv[1]).get("passed") else "0")' "$out" 2>/dev/null || echo 0)"
      if [[ "$is_pass" == "1" ]]; then passed=$((passed+1)); else failed=$((failed+1)); fi
    done
  fi
  if [[ $SYNTHETIC_ONLY -eq 0 ]]; then
    if [[ -f "$FIXTURE_CHROME" ]]; then
      for gpu in 0 1; do
        gpu_label="cpu-osr"; [[ "$gpu" == "1" ]] && gpu_label="gpu-osr"
        out2="$(run_probe_at_scale "$sc" "$FIXTURE_CHROME" "fractional chrome $gpu_label scale=$sc" "$gpu")"
        echo "$out2"
        is_pass2="$(python3 -c 'import json,sys; print("1" if json.loads(sys.argv[1]).get("passed") else "0")' "$out2" 2>/dev/null || echo 0)"
        if [[ "$is_pass2" == "1" ]]; then passed=$((passed+1)); else failed=$((failed+1)); fi
      done
    else
      # Chrome fixture absent — emit SKIP rather than synthetic duplicate
      python3 -c 'import json,sys; print(json.dumps({"config":f"fractional chrome scale={sys.argv[1]}","scale":float(sys.argv[1]),"passed":None,"worst_error_px":None,"reason":"chrome-mimic pending KARE-018","skipped":True}))' "$sc"
    fi
  fi
done

# Restore and verify — gate exit code on restore equality.
# cleanup_restore is invoked with || so a failed restore does not trip `set -e`
# here: the restore verification JSON is still emitted and the failure recorded.
restore_rc=0
cleanup_restore || restore_rc=$?
RESTORED=1
trap - EXIT INT TERM
after_scale="$(get_current_primary_scale 2>/dev/null || echo "unknown")"
equal="false"
if [[ "$after_scale" == "$SAVED_BEFORE_SCALE" ]]; then
  equal="true"
else
  python3 -c "import sys; a=float(sys.argv[1]); b=float(sys.argv[2]); sys.exit(0 if abs(a-b)<0.01 else 1)" "$after_scale" "$SAVED_BEFORE_SCALE" 2>/dev/null && equal="true" || equal="false"
fi
python3 -c '
import json,sys
before=sys.argv[1]; after=sys.argv[2]; equal=json.loads(sys.argv[3].lower())
print(json.dumps({"restore":{"before":float(before) if before!="unknown" else before,"after":float(after) if after!="unknown" and after.replace(".","",1).isdigit() else after,"equal":equal}}))
' "$SAVED_BEFORE_SCALE" "$after_scale" "$equal" >&2
if [[ "$equal" != "true" || $restore_rc -ne 0 ]]; then
  echo "restore verification failed (equal=$equal rc=$restore_rc)" >&2
  failed=$((failed+1))
fi

# Final summary
python3 -c '
import json,sys
passed=int(sys.argv[1]); failed=int(sys.argv[2])
print(json.dumps({"summary":{"passed":passed,"failed":failed,"total":passed+failed}}))
' "$passed" "$failed" >&2

if [[ $failed -gt 0 ]]; then
  exit 1
fi
exit 0
