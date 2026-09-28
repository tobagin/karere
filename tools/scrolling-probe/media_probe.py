#!/usr/bin/env python3
"""Generated-only playback/WebRTC producer measurements. Never presented FPS.

Reuses the existing loopback CDP transport. Refuses an occupied diagnostic port,
checks fixture identity, closes the listener and leaves account profiles alone.
"""
import argparse
import functools
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import signal
import socket
import subprocess
import tempfile
import threading
import time
import urllib.request

from cdp_local import Client
from karere_probe import descendants, process as read_process

ROOT = Path(__file__).resolve().parent


def diagnostic_listener_present():
    with socket.socket() as check:
        check.settimeout(.5)
        return check.connect_ex(("127.0.0.1", 9333)) == 0


class MediaClient(Client):
    def __init__(self, url):
        self.properties = {}
        super().__init__(url)
        self.socket.settimeout(55)

    def receive(self):
        reply = super().receive()
        if reply.get("method") == "Media.playerPropertiesChanged":
            for item in reply["params"]["properties"]:
                if item["name"] in {"kVideoDecoderName", "kIsPlatformVideoDecoder", "kVideoTracks", "kResolution", "kVideoPlaybackRoughness", "kFramerate"}:
                    self.properties[item["name"]] = item["value"]
        return reply


class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def send_head(self):
        self.range_remaining = None
        requested = self.headers.get("Range")
        if not requested or self.path != "/sample.mp4":
            return super().send_head()
        match = re.fullmatch(r"bytes=(\d*)-(\d*)", requested)
        if not match or not any(match.groups()):
            self.send_error(416, "single byte range required")
            return None
        try:
            stream = open(self.translate_path(self.path), "rb")
        except OSError:
            self.send_error(404)
            return None
        length = os.fstat(stream.fileno()).st_size
        low, high = match.groups()
        start = int(low) if low else max(0, length - int(high))
        end = min(length - 1, int(high)) if low and high else length - 1
        if start > end or start >= length:
            stream.close()
            self.send_response(416)
            self.send_header("Content-Range", f"bytes */{length}")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return None
        stream.seek(start)
        self.range_remaining = end - start + 1
        self.send_response(206)
        self.send_header("Content-Type", "video/mp4")
        self.send_header("Content-Range", f"bytes {start}-{end}/{length}")
        self.send_header("Content-Length", str(self.range_remaining))
        self.end_headers()
        return stream

    def end_headers(self):
        if self.path == "/sample.mp4":
            self.send_header("Accept-Ranges", "bytes")
        super().end_headers()

    def copyfile(self, source, outputfile):
        try:
            if self.range_remaining is None:
                super().copyfile(source, outputfile)
            else:
                while self.range_remaining:
                    data = source.read(min(65536, self.range_remaining))
                    if not data:
                        break
                    outputfile.write(data)
                    self.range_remaining -= len(data)
        except (BrokenPipeError, ConnectionResetError):
            # Chromium may cancel a buffered range after seek or fixture close.
            pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--probe", required=True, type=Path)
    parser.add_argument("--cef-directory", type=Path, help="Matching library/resources for an isolated engine control")
    parser.add_argument("--transfer", choices=["cpu", "accelerated"], default="cpu")
    parser.add_argument("--kind", choices=["webrtc", "playback"], default="webrtc")
    parser.add_argument("--video", type=Path)
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=720)
    parser.add_argument("--fps", type=int, default=30)
    parser.add_argument("--bitrate", type=int, default=2_000_000)
    parser.add_argument("--codec", default="H264")
    parser.add_argument("--features", default="")
    parser.add_argument("--angle", choices=["gl-egl", "vulkan"], default="gl-egl")
    parser.add_argument("--app-id", default="io.github.tobagin.karere.Devel")
    parser.add_argument("--duration", type=int, default=30)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--graphics-check", action="store_true", help="Verify generated WebGL/WebGPU output after timing")
    args = parser.parse_args()
    if args.output.exists() or args.output.with_suffix(".log").exists():
        parser.error("refusing to overwrite evidence")
    if args.kind == "playback" and not args.video:
        parser.error("playback needs a generated --video fixture")
    if diagnostic_listener_present():
        parser.error("diagnostic port already has a listener")
    if args.cef_directory:
        args.cef_directory = args.cef_directory.resolve(strict=True)
        if not all((args.cef_directory / item).exists() for item in ("libcef.so", "icudtl.dat", "locales")):
            parser.error("CEF directory must include matching library, resources and locales")
    process = client = browser = server = None
    report = {"kind": args.kind, "configuration": {k: str(v) if isinstance(v, Path) else v for k,v in vars(args).items()},
              "presentation_verified": False, "samples": []}
    if args.cef_directory:
        with (args.cef_directory / "libcef.so").open("rb") as stream:
            report["override_libcef_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="karere-media-fixture-") as directory:
        directory = Path(directory)
        served = directory / "www"
        served.mkdir()
        (served / "fixture.html").write_bytes((ROOT / "media_fixture.html").read_bytes())
        if args.video:
            (served / "sample.mp4").symlink_to(args.video.resolve())
        control = directory / "close.fifo"
        os.mkfifo(control, 0o600)
        control_fd = os.open(control, os.O_RDWR | os.O_NONBLOCK | os.O_CLOEXEC)
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(QuietHandler, directory=served))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        url = f"http://127.0.0.1:{server.server_port}/fixture.html?fakeCapture=1"
        lifetime = max(60, (args.duration + 5) * args.samples + 40)
        command = ["flatpak", "run", f"--filesystem={args.probe.resolve().parent}:ro",
                   f"--filesystem={directory}:ro", f"--env=KARERE_FRAME_PROBE_CONTROL={control}",
                   f"--env=KARERE_FRAME_PROBE_URL={url}",
                   f"--env=KARERE_FRAME_PROBE_DURATION={lifetime}",
                   f"--env=KARERE_FRAME_PROBE_WIDTH={args.width}", f"--env=KARERE_FRAME_PROBE_HEIGHT={args.height}",
                   f"--command={args.probe.resolve()}"]
        if args.transfer == "cpu":
            command.append("--env=KARERE_FRAME_PROBE_CPU=1")
        if args.cef_directory:
            command += [f"--filesystem={args.cef_directory}:ro",
                        f"--env=LD_LIBRARY_PATH={args.cef_directory}",
                        f"--env=KARERE_FRAME_PROBE_CEF_DIR={args.cef_directory}"]
        command += [args.app_id, "--no-first-run", "--no-zygote", "--no-sandbox",
                   "--ozone-platform=wayland", "--use-gl=angle", f"--use-angle={args.angle}",
                   "--remote-debugging-port=9333", "--autoplay-policy=no-user-gesture-required",
                   "--enable-webrtc-vea-vda", "--disable-features=PersistentHistograms",
                   "--enable-media-stream", "--use-fake-ui-for-media-stream",
                   f"--use-fake-device-for-media-stream=fps={args.fps}"]
        if args.features:
            command.append(f"--enable-features={args.features}")
        try:
            with args.output.with_suffix(".log").open("x") as log:
                process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    try:
                        with urllib.request.urlopen("http://127.0.0.1:9333/json/list", timeout=1) as reply:
                            targets = json.load(reply)
                        target = next(t for t in targets if t.get("type") == "page" and t.get("url") == url)
                        client = MediaClient(target["webSocketDebuggerUrl"])
                        if client.evaluate("typeof startProbe === 'function'"):
                            break
                        client.close(); client = None
                    except (OSError, StopIteration, RuntimeError):
                        pass
                    if process.poll() is not None:
                        raise RuntimeError("CEF probe exited before readiness")
                    time.sleep(.2)
                if client is None:
                    raise RuntimeError("generated CEF fixture unavailable")
                with urllib.request.urlopen("http://127.0.0.1:9333/json/version", timeout=1) as reply:
                    version = json.load(reply)
                browser = Client(version["webSocketDebuggerUrl"])
                gpu = browser.call("SystemInfo.getInfo", {})["gpu"]
                report["gpu"] = {k: gpu.get(k) for k in ["devices", "featureStatus", "videoDecoding", "videoEncoding"]}
                report["graphics"] = {k: v for k,v in gpu.get("auxAttributes", {}).items() if k in ["glRenderer", "glVendor", "glVersion", "displayType", "glImplementationParts"]}
                client.call("Media.enable", {})
                options = {k: getattr(args, k) for k in ["kind", "width", "height", "fps", "bitrate", "codec"]}
                report["ready"] = client.evaluate(f"startProbe({json.dumps(options)})", await_promise=True)
                for index in range(args.samples):
                    client.evaluate("prepareSample(5)", await_promise=True)
                    before_cpu = {pid: row for pid in descendants(process.pid) if (row := read_process(pid))}
                    sample = client.evaluate(f"sampleProbe({args.duration},0)", await_promise=True)
                    after_cpu = {pid: row for pid in descendants(process.pid) if (row := read_process(pid))}
                    ticks = sum(row["ticks"] - before_cpu.get(pid, {"ticks": 0})["ticks"]
                                for pid, row in after_cpu.items()
                                if pid not in before_cpu or row["start"] == before_cpu[pid]["start"])
                    sample["warmup"] = 5
                    sample["cpu_percent"] = ticks / os.sysconf("SC_CLK_TCK") / sample["duration"] * 100
                    sample["cpu_processes_missing"] = len(before_cpu.keys() - after_cpu.keys())
                    report["samples"].append(sample)
                    report["decoder_properties"] = client.properties.copy()
                    args.output.write_text(json.dumps(report, indent=2) + "\n")
                    print(f"sample {index+1}/{args.samples}: {sample['videoCallbackFps']:.2f} video callbacks/s", flush=True)
                if args.kind == "playback":
                    report["generated_pixel_control"] = client.evaluate("verifyPlaybackOutput()", await_promise=True)
                if args.graphics_check:
                    report["graphics_operations"] = client.evaluate((ROOT / "graphics_fixture.js").read_text(), await_promise=True)
                client.evaluate("stopProbe()")
        finally:
            if client:
                try:
                    client.evaluate("stopProbe()")
                except (OSError, RuntimeError):
                    pass
                client.close()
            if browser:
                browser.close()
            os.write(control_fd, b"q")
            if process and process.poll() is None:
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    report["shutdown_failure"] = "orderly CEF close exceeded ten seconds"
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait(timeout=5)
            os.close(control_fd)
            deadline = time.monotonic() + 5
            while diagnostic_listener_present() and time.monotonic() < deadline:
                time.sleep(.1)
            report["shutdown"] = {"exit_code": process.returncode if process else None,
                                  "listener_closed": not diagnostic_listener_present()}
            log_path = args.output.with_suffix(".log")
            if log_path.exists():
                report["producer_records"] = [json.loads(line) for line in log_path.read_text().splitlines()
                                              if line.startswith('{"kind":')]
            server.shutdown(); server.server_close()
            report["decoder_properties"] = client.properties if client else {}
            args.output.write_text(json.dumps(report, indent=2) + "\n")
            if process and (process.returncode != 0 or not report["shutdown"]["listener_closed"] or report.get("shutdown_failure")):
                raise RuntimeError("CEF probe failed orderly shutdown; see recorded evidence")


if __name__ == "__main__":
    main()
