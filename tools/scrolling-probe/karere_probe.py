#!/usr/bin/env python3
"""Capture allowlisted rendering logs and resource counters, never page contents."""
import argparse
import atexit
import hashlib
import html
import json
import os
from pathlib import Path
import re
import selectors
import socket
import subprocess
import queue
import threading
import time
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent
BUILD = ROOT / ".build"
RESULTS = ROOT / "results"
APP = "io.github.tobagin.karere"
LOG_FILTER = "warn,karere::web_view=debug,karere::handlers::render=debug,karere::gl_dmabuf=info,karere::graphics=info,karere::presenter=info,karere::vulkan_frame=info"
ALLOW = re.compile(r"graphics:|render cadence:|accel_osr:|GLArea (context ready|realize error)|coord: J1 size_allocate|coord: J2 (on_paint|on_accelerated_paint|screen_info|view_rect)|coord: J4 draw|accelerated OSR (import failed|disabled)|gl_dmabuf:|^pump-probe ")
KEYS = ["gpu-rendering", "start-in-background", "window-width", "window-height", "is-maximized", "zoom-level", "reduce-motion"]


def measure_chat_list(results):
    """Only evaluate timing/geometry on the visible WhatsApp page."""
    from cdp_local import Client
    client = None
    deadline = time.monotonic() + 90
    try:
        while time.monotonic() < deadline:
            try:
                with urllib.request.urlopen('http://127.0.0.1:9333/json/list', timeout=.5) as response:
                    targets = json.load(response)
                for target in targets:
                    if target.get('type') != 'page' or urllib.parse.urlparse(target.get('url', '')).hostname != 'web.whatsapp.com':
                        continue
                    candidate = Client(target['webSocketDebuggerUrl'])
                    ready = candidate.evaluate("document.visibilityState === 'visible' && !!document.querySelector('#pane-side')")
                    if ready:
                        client = candidate
                        break
                    candidate.close()
                if client:
                    break
            except (OSError, ValueError, RuntimeError):
                pass
            time.sleep(1)
        if not client:
            results.put(('chat_list_error', {'reason': 'visible chat list not ready'}))
            return
        results.put(('chat_list_ready', {}))
        time.sleep(20)
        report = client.evaluate((ROOT / 'chat_list_probe.js').read_text(), await_promise=True)
        if not isinstance(report, dict) or not report.get('ready'):
            results.put(('chat_list_error', {'reason': 'scrollable chat list not found'}))
        else:
            results.put(('chat_list_result', report))
    except (OSError, ValueError, RuntimeError, KeyError):
        results.put(('chat_list_error', {'reason': 'timing probe could not complete'}))
    finally:
        if client:
            client.close()


def read(path):
    try:
        return Path(path).read_text().strip()
    except OSError:
        return None


def descendants(root):
    found, pending = set(), [root]
    while pending:
        pid = pending.pop()
        if pid in found:
            continue
        found.add(pid)
        try:
            tasks = list(Path(f"/proc/{pid}/task").iterdir())
        except OSError:
            tasks = []
        for task in tasks:
            children = read(task / "children") or ""
            pending.extend(int(p) for p in children.split())
    return found


def process(pid):
    stat = read(f"/proc/{pid}/stat")
    if not stat:
        return None
    fields = stat[stat.rfind(")") + 2:].split()
    try:
        argv = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
        role = next((x.decode(errors="replace").split("=", 1)[1] for x in argv if x.startswith(b"--type=")), "main")
        name = stat[stat.find("(") + 1:stat.rfind(")")]
        return {"pid": pid, "role": role, "name": name, "ticks": int(fields[11]) + int(fields[12]), "start": fields[19], "rss_bytes": int(fields[21]) * os.sysconf("SC_PAGE_SIZE")}
    except (OSError, ValueError, IndexError):
        return None


def device_info(pid):
    devices = set()
    try:
        for fd in Path(f"/proc/{pid}/fd").iterdir():
            try:
                name = str(fd.readlink())
                if name.startswith("/dev/dri/") or name.startswith("/dev/nvidia"):
                    devices.add(name)
            except OSError:
                pass
    except OSError:
        pass
    maps = read(f"/proc/{pid}/maps") or ""
    libs = {line.split()[-1] for line in maps.splitlines() if re.search(r"radeonsi|swrast|libEGL|libGLX|libnvidia", line)}
    return {"devices": sorted(devices), "graphics_libraries": sorted(libs)}


def configure_presentation_probe(parser, args):
    """Select a validated app ID and restrict sizing to isolated fixtures."""
    global APP
    APP = args.app_id
    if not re.fullmatch(r"[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+", APP):
        parser.error("Invalid Flatpak application ID")
    if args.windowed and not args.synthetic:
        parser.error("--windowed requires an isolated --synthetic fixture")


def synthetic_visibility():
    """Read only the generated page's visibility state for the background check."""
    from cdp_local import Client
    with urllib.request.urlopen('http://127.0.0.1:9333/json/list', timeout=2) as response:
        targets = json.load(response)
    for target in targets:
        if target.get('type') == 'page' and target.get('url', '').startswith('data:'):
            client = Client(target['webSocketDebuggerUrl'])
            try:
                return client.evaluate('document.visibilityState')
            finally:
                client.close()
    raise RuntimeError('Generated page unavailable for the background visibility check')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("label")
    parser.add_argument("--normal-logging", action="store_true")
    parser.add_argument("--pump-probe", action="store_true")
    parser.add_argument("--fast-backstop", action="store_true")
    parser.add_argument("--schedule-probe", action="store_true")
    parser.add_argument("--synthetic", action="store_true")
    parser.add_argument("--synthetic-gpu", action="store_true")
    parser.add_argument("--chat-list", action="store_true")
    parser.add_argument("--gsk-renderer", choices=["gl", "vulkan"])
    parser.add_argument("--cef-graphics", choices=["gl", "vulkan", "software"])
    parser.add_argument("--frame-transfer", choices=["gl", "vulkan"])
    parser.add_argument("--cpu-presenter", choices=["gl", "snapshot"])
    parser.add_argument("--app-id", default=APP)
    parser.add_argument("--windowed", action="store_true", help="Windowed isolated fixture")
    parser.add_argument("--wayland-timing", action="store_true", help="Collect allowlisted compositor feedback")
    parser.add_argument("--binary", type=Path, help="Test a compiled Karere inside the installed Flatpak runtime")
    parser.add_argument("--cef-directory", type=Path, help="Matching CEF library/resources for an engine control")
    parser.add_argument("--idle-seconds", type=float, default=0,
                        help="After scrolling and 5 seconds settling, measure this many idle seconds")
    parser.add_argument("--idle-window", choices=['visible', 'minimized', 'background'], default='visible',
                        help="Window action before idle measurement; background requires --synthetic")
    args = parser.parse_args()
    if not 0 <= args.idle_seconds <= 300:
        parser.error('--idle-seconds must be between 0 and 300')
    if args.idle_seconds and not (args.synthetic or args.chat_list):
        parser.error('--idle-seconds requires an automatic workload')
    if args.idle_window != 'visible' and not args.idle_seconds:
        parser.error('--idle-window requires --idle-seconds')
    if args.idle_window == 'background' and not args.synthetic:
        parser.error('Background mode requires the isolated --synthetic profile')
    if args.synthetic_gpu and not args.synthetic:
        parser.error('--synthetic-gpu requires --synthetic')
    if args.chat_list:
        if args.synthetic:
            parser.error('--chat-list and --synthetic are separate workloads')
        args.schedule_probe = True
    binary_hash = None
    if args.binary:
        args.binary = args.binary.resolve(strict=True)
        if not args.binary.is_file() or not os.access(args.binary, os.X_OK):
            parser.error('--binary must be an executable file')
        binary_hash = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    cef_hash = None
    if args.cef_directory:
        args.cef_directory = args.cef_directory.resolve(strict=True)
        if not all((args.cef_directory / item).exists() for item in ("libcef.so", "icudtl.dat", "locales")):
            parser.error("--cef-directory requires matching library, resources and locales")
        with (args.cef_directory / "libcef.so").open("rb") as library:
            digest = hashlib.sha256()
            for block in iter(lambda: library.read(1024 * 1024), b""):
                digest.update(block)
            cef_hash = digest.hexdigest()
    configure_presentation_probe(parser, args)
    if not re.fullmatch(r"[a-zA-Z0-9_-]+", args.label):
        parser.error("label must contain only letters, numbers, underscores, or hyphens")
    os.umask(0o077)
    RESULTS.mkdir(exist_ok=True)
    output = RESULTS / f"{args.label}.jsonl"
    if output.exists():
        parser.error(f"capture already exists: {output}")
    rows = subprocess.check_output(["flatpak", "ps", "--columns=application"], text=True).splitlines()
    if APP in [r.strip() for r in rows]:
        parser.error("Karere is already running; quit normally before launching a capture")
    if args.synthetic or args.chat_list or os.environ.get("KARERE_PROBE_CDP"):
        with socket.socket() as check:
            check.settimeout(.2)
            if check.connect_ex(('127.0.0.1', 9333)) == 0:
                parser.error('The diagnostic port 9333 is already in use')
    prefs = subprocess.check_output(["flatpak", "run", "--command=sh", APP, "-c", 'for key in ' + ' '.join(KEYS) + '; do printf "%s=" "$key"; gsettings get ' + APP + ' "$key"; done'], text=True)
    settings = dict(line.split("=", 1) for line in prefs.splitlines() if "=" in line)
    launch = ["flatpak", "run", "--env=RUST_LOG=" + ("warn" if args.normal_logging else LOG_FILTER), "--env=RUST_LOG_STYLE=never"]
    if args.pump_probe or args.fast_backstop or args.schedule_probe:
        library = 'schedule_probe.so' if args.schedule_probe else 'pump_probe.so'
        if not (BUILD / library).is_file():
            parser.error('Probe not built; run prepare.py first')
        launch += [f"--filesystem={ROOT}:ro", f"--env=LD_PRELOAD={BUILD / library}"]
    if args.fast_backstop:
        launch += ["--env=KARERE_PROBE_BACKSTOP_MS=8"]
    if args.chat_list or os.environ.get("KARERE_PROBE_CDP"):
        launch += ["--env=KARERE_PROBE_CDP=1"]
    if args.wayland_timing:
        launch += ["--env=WAYLAND_DEBUG=client"]
    if args.gsk_renderer:
        launch += [f"--env=GSK_RENDERER={args.gsk_renderer}"]
    for value, name in ((args.cef_graphics, "KARERE_CEF_GRAPHICS"),
                        (args.frame_transfer, "KARERE_FRAME_TRANSFER"),
                        (args.cpu_presenter, "KARERE_CPU_PRESENTER")):
        if value:
            launch.append(f"--env={name}={value}")
    if args.synthetic:
        schema_dir = BUILD / (('schemas_windowed' if args.windowed else 'schemas') + ('_gpu' if args.synthetic_gpu else ''))
        if not (schema_dir / 'gschemas.compiled').is_file():
            parser.error('Isolated schemas not built; run prepare.py first')
        launch += [f"--filesystem={ROOT}:ro", "--env=GSETTINGS_BACKEND=memory",
                   f"--env=GSETTINGS_SCHEMA_DIR={schema_dir}",
                   f"--env=XDG_DATA_HOME=/tmp/karere-perf-{args.label}/data",
                   f"--env=XDG_CONFIG_HOME=/tmp/karere-perf-{args.label}/config",
                   f"--env=XDG_CACHE_HOME=/tmp/karere-perf-{args.label}/cache"]
    if args.binary:
        launch += [f"--filesystem={args.binary.parent}:ro", f"--command={args.binary}"]
    if args.cef_directory:
        launch += [f"--filesystem={args.cef_directory}:ro", f"--env=LD_LIBRARY_PATH={args.cef_directory}"]
    launch.append(APP)
    if args.binary or args.cef_directory:
        cef_directory = args.cef_directory or Path('/app/lib/cef')
        launch += [f'--resources-dir-path={cef_directory}', f'--locales-dir-path={cef_directory / "locales"}']
    if args.synthetic:
        page = 'data:text/html;charset=utf-8,' + urllib.parse.quote((ROOT / 'scroll_probe.html').read_text())
        launch += ['--debuglevel=error', '--url', page]
    child = subprocess.Popen(launch, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    def quit_on_exit():
        if child.poll() is None:
            try:
                subprocess.run(['gapplication', 'action', APP, 'quit'],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
                child.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                child.terminate()
    atexit.register(quit_on_exit)
    live_results = queue.Queue()
    if args.chat_list:
        threading.Thread(target=measure_chat_list, args=(live_results,), daemon=True).start()
    os.set_blocking(child.stdout.fileno(), False)
    selector = selectors.DefaultSelector()
    selector.register(child.stdout, selectors.EVENT_READ)
    start = time.monotonic()
    previous, seen, buffer = {}, set(), b""
    last_sample = start
    phase = "startup"
    count = 0
    metrics_received = False
    idle_start = idle_end = idle_epoch = None
    idle_visibility = None
    synthetic_debug_reported = False
    print(json.dumps({"capture": str(output), "launcher_pid": child.pid, "settings": settings}), flush=True)
    with output.open("x", buffering=1) as stream:
        def emit(kind, **values):
            """Record a capture-time clock pair for later cross-clock alignment."""
            now = time.monotonic()
            stream.write(json.dumps({"t": now - start, "wall": time.time(), "monotonic": now,
                                     "kind": kind, "phase": phase, **values}) + "\n")
        def finish_measurement(success):
            nonlocal metrics_received, idle_start, idle_end
            metrics_received = True
            if success and args.idle_seconds:
                if args.idle_window != 'visible':
                    subprocess.run([
                        'gdbus', 'call', '--session', '--dest', APP,
                        '--object-path', '/io/github/tobagin/karere/window/1',
                        '--method', 'org.gtk.Actions.Activate',
                        'close' if args.idle_window == 'background' else 'minimize', '[]', '{}',
                    ], check=True, stdout=subprocess.DEVNULL)
                idle_start = time.monotonic() + 5
                idle_end = idle_start + args.idle_seconds
            else:
                subprocess.run(['gapplication', 'action', APP, 'quit'], check=True)
        emit("start", label=args.label, settings=settings, launcher_pid=child.pid, normal_logging=args.normal_logging, pump_probe=args.pump_probe, fast_backstop=args.fast_backstop, schedule_probe=args.schedule_probe, synthetic=args.synthetic, synthetic_gpu=args.synthetic_gpu, chat_list=args.chat_list, gsk_renderer=args.gsk_renderer, cef_graphics=args.cef_graphics, frame_transfer=args.frame_transfer, cpu_presenter=args.cpu_presenter, binary_sha256=binary_hash, cef_library_sha256=cef_hash, windowed=args.windowed, wayland_timing=args.wayland_timing)
        while True:
            while not live_results.empty():
                kind, report = live_results.get_nowait()
                emit(kind, **report)
                print(json.dumps({kind: report}), flush=True)
                if kind in ('chat_list_result', 'chat_list_error'):
                    finish_measurement(kind == 'chat_list_result')
            for key, _ in selector.select(timeout=0.1):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                buffer += chunk
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    text = line.decode(errors="replace")
                    if args.wayland_timing and re.search(r"wp_presentation|wl_surface#\d+\.(?:commit|frame)|wl_callback#\d+\.done", text):
                        emit("wayland", message=text)
                    if ALLOW.search(text):
                        emit("render", message=text)
                        count += 1
            now = time.monotonic()
            if idle_start is not None and now >= idle_start and idle_epoch is None:
                if args.synthetic:
                    idle_visibility = synthetic_visibility()
                    if args.idle_window == 'background' and idle_visibility != 'hidden':
                        raise RuntimeError('Background action did not hide the generated page')
                idle_epoch = time.time() * 1000
                emit('idle_start', start_epoch_ms=idle_epoch)
            if idle_end is not None and now >= idle_end:
                emit('idle_result', start_epoch_ms=idle_epoch, end_epoch_ms=time.time() * 1000,
                     requested_window_state=args.idle_window, page_visibility=idle_visibility)
                idle_end = None
                subprocess.run(['gapplication', 'action', APP, 'quit'], check=True)
            if now - last_sample >= 1:
                new_phase = read(ROOT / "phase.txt") or "unmarked"
                if new_phase != phase:
                    phase = new_phase
                    emit("phase")
                samples = []
                current = {}
                for pid in descendants(child.pid):
                    info = process(pid)
                    if info is None:
                        continue
                    identity = (pid, info["start"])
                    if identity not in seen:
                        emit("process", **info, **device_info(pid))
                        seen.add(identity)
                    current[identity] = info["ticks"]
                    info["cpu_pct"] = (info["ticks"] - previous.get(identity, info["ticks"])) / os.sysconf("SC_CLK_TCK") / (now - last_sample) * 100
                    samples.append(info)
                previous = current
                gpu = read("/sys/class/drm/card0/device/gpu_busy_percent")
                emit("sample", processes=samples, cpu_pct=sum(s["cpu_pct"] for s in samples), gpu_busy_pct=int(gpu) if gpu and gpu.isdigit() else None, memory_pressure=read("/proc/pressure/memory"), on_ac=read("/sys/class/power_supply/AC0/online"))
                if args.synthetic and not metrics_received and now - start > 5:
                    try:
                        with urllib.request.urlopen('http://127.0.0.1:9333/json/list', timeout=.2) as response:
                            targets = json.load(response)
                        for target in targets:
                            title = html.unescape(target.get('title', ''))
                            if not synthetic_debug_reported:
                                emit('synthetic_target', page_type=target.get('type'), title_kind='result' if title.startswith('KARERE_PERF:') else 'loading', url_scheme=target.get('url', '').split(':', 1)[0])
                                synthetic_debug_reported = True
                            if title.startswith('KARERE_PERF:'):
                                report = json.loads(title.removeprefix('KARERE_PERF:'))
                                emit('synthetic_result', **report)
                                print(json.dumps({'synthetic_result': report}), flush=True)
                                finish_measurement(True)
                                break
                    except (OSError, ValueError):
                        pass
                    if now - start > 60 and not metrics_received:
                        emit('synthetic_timeout')
                        print('Synthetic result timeout; quitting the temporary profile.', flush=True)
                        metrics_received = True
                        subprocess.run(['gapplication', 'action', APP, 'quit'], check=True)
                last_sample = now
            if args.chat_list and now - start > 150 and not metrics_received:
                emit('chat_list_error', reason='capture timeout')
                metrics_received = True
                subprocess.run(['gapplication', 'action', APP, 'quit'], check=True)
            if child.poll() is not None and not selector.get_map():
                emit("exit", returncode=child.returncode, render_lines=count)
                print(json.dumps({"finished": args.label, "returncode": child.returncode, "render_lines": count}), flush=True)
                break


if __name__ == "__main__":
    main()
