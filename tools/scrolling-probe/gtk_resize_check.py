#!/usr/bin/env python3
"""Resize an already running, isolated generated Devel fixture through KWin."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import time

from probe_common import connect

APP = 'io.github.tobagin.karere.Devel'
PLUGIN = 'karere-cef156-generated-resizes'


def read_integrity(stream):
    """Leave a concurrently written final JSONL record unread until complete."""
    while True:
        position = stream.tell()
        line = stream.readline()
        if not line or not line.endswith('\n'):
            stream.seek(position)
            return
        row = json.loads(line)
        if row.get('kind') == 'integrity':
            yield row


def dbus(path, method, *arguments):
    """Call KWin only; temporary scripts target the generated Devel window."""
    return subprocess.check_output(['gdbus', 'call', '--session', '--dest', 'org.kde.KWin',
        '--object-path', path, '--method', method, *map(str, arguments)], text=True, timeout=5)


def geometry(client):
    """Reject account navigation before every generated-page geometry read."""
    value = client.evaluate('({generated:location.protocol==="data:" && '
        'document.title.includes("CEF frame integrity fixture"),'
        'width:innerWidth,height:innerHeight,dpr:devicePixelRatio})')
    if not value.get('generated'):
        raise RuntimeError('expected only the generated integrity fixture')
    return value


def set_maximized(script, maximized):
    """Load/run/unload one targeted action; no KWin plugin remains installed."""
    script.write_text('for (const window of workspace.windowList()) {'
        'if (String(window.resourceClass).toLowerCase().includes("karere.devel")) {'
        f'window.setMaximize({str(maximized).lower()}, {str(maximized).lower()});'
        '}}\n')
    loaded = dbus('/Scripting', 'org.kde.kwin.Scripting.loadScript', script, PLUGIN)
    match = re.fullmatch(r'\((-?\d+),\)\s*', loaded)
    if not match or int(match[1]) < 0:
        raise RuntimeError('could not load targeted resize script')
    try:
        dbus('/Scripting/Script' + match[1], 'org.kde.kwin.Script.run')
    finally:
        dbus('/Scripting', 'org.kde.kwin.Scripting.unloadScript', PLUGIN)


def main():
    """Collect image checks across 72 real maximize/restore transitions."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--script-dir', required=True, type=Path)
    args = parser.parse_args()
    args.script_dir = args.script_dir.resolve()
    if args.output.exists():
        parser.error('refusing to overwrite resize evidence')
    args.script_dir.mkdir(parents=True, exist_ok=True)
    script = args.script_dir / 'kwin-generated-resize.js'
    result = {'generated_only': True, 'performance_sample': False,
              'steps': [], 'complete': False, 'passed': False}
    client = None
    try:
        client = connect()
        result['initial_geometry'] = geometry(client)
        set_maximized(script, True)
        time.sleep(6)
        current = geometry(client)
        with args.capture.open() as stream:
            stream.seek(0, 2)
            for index in range(72):
                started = time.monotonic()
                # The initial fixture may already be maximized by a previous
                # calibration action. Start with restore, then alternate.
                maximize = index % 2 == 1
                step = {'index': index + 1, 'maximized': maximize, 'checks': []}
                set_maximized(script, maximize)
                deadline = time.monotonic() + 3
                while time.monotonic() < deadline:
                    now = geometry(client)
                    step['checks'].extend(read_integrity(stream))
                    changed = (now['width'], now['height']) != (current['width'], current['height'])
                    aligned = [row for row in step['checks']
                        if row['width'] == round(now['width'] * now['dpr'])
                        and row['height'] == round(now['height'] * now['dpr'])]
                    if changed and aligned and time.monotonic() - started >= .35:
                        step['geometry'] = now
                        step['aligned_checks'] = len(aligned)
                        current = now
                        break
                    time.sleep(.02)
                else:
                    raise RuntimeError(f'resize {index + 1} lacked changed geometry and image coverage')
                result['steps'].append(step)
                args.output.write_text(json.dumps(result, indent=2) + '\n')
        result['complete'] = True
        checks = [row for step in result['steps'] for row in step['checks']]
        result['checked_frames'] = len(checks)
        result['bad_frames'] = sum(row['unequal'] > 0 or not row['palette_valid'] for row in checks)
        result['transfers'] = sorted({row['transfer'] for row in checks})
        result['passed'] = not result['bad_frames']
    except BaseException as error:
        result['error'] = str(error)
    finally:
        if client:
            client.close()
        try:
            dbus('/Scripting', 'org.kde.kwin.Scripting.unloadScript', PLUGIN)
        except subprocess.SubprocessError:
            pass
        subprocess.run(['gapplication', 'action', APP, 'quit'], check=False, timeout=10)
        args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({key: value for key, value in result.items() if key != 'steps'}))
    if not result['complete'] or not result['passed']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
