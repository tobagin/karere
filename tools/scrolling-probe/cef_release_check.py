#!/usr/bin/env python3
"""Check a released CEF using generated content, separate from account/FPS tests."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
from urllib.parse import quote
from urllib.request import urlopen

from cdp_local import Client
from karere_probe import descendants


def sha256(path):
    """Identify the library that the fixture must actually load."""
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def listener_closed(port):
    """Check the exact temporary diagnostic port."""
    with socket.socket() as sock:
        sock.settimeout(.2)
        return sock.connect_ex(('127.0.0.1', port)) != 0


def producer_processes(pid, executable):
    """Find only this launched fixture, excluding its CEF child processes."""
    found = []
    for child in descendants(pid):
        try:
            command = Path(f'/proc/{child}/cmdline').read_bytes().split(b'\0')
            if (command and Path(os.fsdecode(command[0])).name == executable.name
                    and not any(arg.startswith(b'--type=') for arg in command)):
                found.append(child)
        except OSError:
            pass
    return found


def main():
    """Run one immutable, identified producer or image-integrity sample."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--library', required=True, type=Path)
    parser.add_argument('--probe', required=True, type=Path)
    parser.add_argument('--angle', choices=['gl-egl', 'vulkan'], default='gl-egl')
    parser.add_argument('--cpu', action='store_true')
    parser.add_argument('--integrity', action='store_true')
    parser.add_argument('--workload', choices=['canvas', 'webgl'], default='canvas')
    parser.add_argument('--rate', type=int, default=240)
    parser.add_argument('--resizes', type=int, default=72)
    parser.add_argument('--port', type=int, default=9337)
    args = parser.parse_args()
    if args.workload != 'canvas' and not args.integrity:
        parser.error('the WebGL integrity workload requires --integrity')
    if not 1024 <= args.port <= 65535 or not 1 <= args.resizes <= 72:
        parser.error('invalid diagnostic port or resize count')
    if not 1 <= args.rate <= 1000:
        parser.error('invalid producer rate')
    args.library = args.library.resolve(strict=True)
    args.probe = args.probe.resolve(strict=True)
    args.output = args.output.resolve()
    log_path = args.output.with_suffix('.log')
    if args.output.exists() or log_path.exists():
        parser.error('refusing to overwrite evidence')
    if not listener_closed(args.port):
        parser.error('diagnostic port already in use')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    command = ['flatpak', 'run', f'--filesystem={args.probe.parent}:ro',
               f'--filesystem={args.library}:ro', f'--command={args.probe}',
               f'--env=LD_LIBRARY_PATH={args.library}',
               f'--env=KARERE_FRAME_PROBE_CEF_DIR={args.library}',
               f'--env=KARERE_FRAME_PROBE_RATE={args.rate}']
    if args.cpu:
        command.append('--env=KARERE_FRAME_PROBE_CPU=1')
    if args.integrity:
        fixture = 'frame_integrity_webgl.html' if args.workload == 'webgl' else 'frame_integrity.html'
        html = Path(__file__).with_name(fixture).read_text()
        command += ['--env=KARERE_FRAME_PROBE_INTEGRITY=1',
                    f'--env=KARERE_FRAME_PROBE_RESIZES={args.resizes}',
                    '--env=KARERE_FRAME_PROBE_URL=data:text/html,' + quote(html)]
    command += ['io.github.tobagin.karere.Devel', '--no-first-run', '--no-zygote',
                '--no-sandbox', '--ozone-platform=wayland', '--use-gl=angle',
                f'--use-angle={args.angle}', '--disable-features=PersistentHistograms',
                f'--remote-debugging-port={args.port}',
                f'--resources-dir-path={args.library}',
                f'--locales-dir-path={args.library / "locales"}']
    report = {'generated_only': True, 'presentation_verified': False,
              'performance_sample': not args.integrity,
              'integrity_requested': args.integrity, 'cpu_requested': args.cpu,
              'generated_workload': args.workload,
              'angle_requested': args.angle, 'requested_rate': args.rate,
              'libcef_sha256': sha256(args.library / 'libcef.so'),
              'probe_sha256': sha256(args.probe), 'command': command}
    child = None
    error = None
    with log_path.open('x') as log:
        try:
            child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
            deadline = time.monotonic() + 15
            while child.poll() is None and time.monotonic() < deadline:
                try:
                    with urlopen(f'http://127.0.0.1:{args.port}/json/version', timeout=.5) as reply:
                        version = json.load(reply)
                    client = Client(version['webSocketDebuggerUrl'], port=args.port)
                    try:
                        gpu = client.call('SystemInfo.getInfo', {})['gpu']
                        aux = gpu.get('auxAttributes', {})
                        report['gpu'] = {'devices': gpu.get('devices', []),
                            'features': gpu.get('featureStatus', {}),
                            'aux': {key: aux[key] for key in ['displayType', 'glImplementationParts',
                                'glRenderer', 'glVendor', 'glVersion'] if key in aux}}
                    finally:
                        client.close()
                    loaded = set()
                    for pid in descendants(child.pid):
                        try:
                            for line in Path(f'/proc/{pid}/maps').read_text().splitlines():
                                if line.endswith('/libcef.so'):
                                    loaded.add(line.split(None, 5)[5])
                        except OSError:
                            pass
                    report['observed_library_paths'] = sorted(loaded)
                    expected = str(args.library / 'libcef.so')
                    report['library_verified'] = report['observed_library_paths'] == [expected]
                    if not report['library_verified']:
                        raise RuntimeError('fixture did not map exactly the selected CEF library')
                    break
                except (OSError, RuntimeError, ValueError) as exc:
                    error = str(exc)
                    time.sleep(.2)
            if not report.get('library_verified'):
                raise RuntimeError('could not verify loaded engine: ' + str(error))
            child.wait(timeout=65)
        except BaseException as exc:
            report['runner_error'] = str(exc)
        finally:
            if child and child.poll() is None:
                for pid in producer_processes(child.pid, args.probe):
                    try:
                        os.kill(pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    for pid in producer_processes(child.pid, args.probe):
                        try:
                            os.kill(pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    child.wait(timeout=10)
            report['exit_code'] = child.returncode if child else None
            report['records'] = [json.loads(line) for line in log_path.read_text().splitlines()
                                 if line.startswith('{"kind":')]
            report['listener_closed'] = listener_closed(args.port)
            report['valid'] = (report['exit_code'] == 0 and report['listener_closed']
                               and 'runner_error' not in report)
            args.output.write_text(json.dumps(report, indent=2) + '\n')
    summary = {key: report[key] for key in ['exit_code', 'valid', 'listener_closed']}
    summary['records'] = [row for row in report['records'] if row.get('kind') != 'integrity_bad_frame']
    summary['first_bad_frames'] = [row for row in report['records']
                                   if row.get('kind') == 'integrity_bad_frame'][:5]
    print(json.dumps(summary))
    if not report['valid']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
