#!/usr/bin/env python3
"""Drive the current diagnostic page and save timing/geometry, never content."""
import argparse
import json
import time
import uuid
from contextlib import closing
from pathlib import Path

from probe_common import (
    cancel_probe,
    cleanup,
    connect,
    interrupt_cleanup,
    target_argument,
    validate_label,
)

ROOT = Path(__file__).resolve().parent


def metrics(client):
    """Read only numeric layout, script and task counters from CDP."""
    allow = {'LayoutCount', 'RecalcStyleCount', 'LayoutDuration', 'RecalcStyleDuration', 'ScriptDuration',
             'TaskDuration', 'TaskOtherDuration', 'DevToolsCommandDuration', 'V8CompileDuration', 'Timestamp'}
    return {m['name']: m['value'] for m in client.call('Performance.getMetrics', {})['metrics']
            if m['name'] in allow}


def main():
    """Capture one owned run, cancelling it on failure before closing its page."""
    parser = argparse.ArgumentParser()
    parser.add_argument('label')
    parser.add_argument('--target', choices=['conversation', 'list'], default='conversation')
    parser.add_argument('--mode', choices=['programmatic', 'wheel', 'observe'], default='programmatic')
    parser.add_argument('--duration', type=int, default=20)
    parser.add_argument('--wheel-hz', type=float, default=8)
    parser.add_argument('--anchor-bottom', type=float)
    parser.add_argument('--run-id', default=None, help=argparse.SUPPRESS)
    target_argument(parser)
    args = parser.parse_args()
    validate_label(parser, args.label)
    if not 1 <= args.duration <= 25:
        parser.error('--duration must be 1 to 25 seconds (CDP timeout is 35 seconds)')
    if not 0 < args.wheel_hz <= 120:
        parser.error('--wheel-hz must be greater than 0 and at most 120')
    path = ROOT / 'results' / f'{args.label}_page.json'
    path.parent.mkdir(exist_ok=True)
    if path.exists():
        parser.error('Output already exists')
    run_id = args.run_id or uuid.uuid4().hex
    with closing(connect(args.target_id)) as client:
        try:
            client.call('Performance.enable', {'timeDomain': 'threadTicks'})
            client.evaluate((ROOT / 'conversation_probe.js').read_text())
            before = metrics(client)
            params = {'runId': run_id, 'target': args.target,
                      'mode': 'observe' if args.mode in ('wheel', 'observe') else args.mode,
                      'warmup': 5000, 'duration': args.duration * 1000, 'anchorBottom': args.anchor_bottom}
            geometry = client.evaluate('window.__karereConversationProbe.start(' + json.dumps(params) + ')')
            host_start = time.monotonic()
            deliveries = []
            if args.mode == 'wheel':
                # Browser input injection deliberately bypasses GTK and the physical device.
                interval = 1 / args.wheel_hz
                while True:
                    age = time.monotonic() - host_start
                    if age >= args.duration + 5:
                        break
                    direction = 1 if int(age / 3) % 2 == 0 else -1
                    sent = time.monotonic()
                    client.call('Input.dispatchMouseEvent', {'type': 'mouseWheel',
                                'x': geometry['x'] + geometry['width'] / 2,
                                'y': geometry['y'] + geometry['height'] / 2,
                                'deltaX': 0, 'deltaY': direction * 300 * interval})
                    deliveries.append([sent, time.monotonic()])
                    time.sleep(max(0, interval - (time.monotonic() - sent)))
            result = client.evaluate('window.__karereConversationProbe.done', await_promise=True)
            if not result or result.get('run_id') != run_id or result.get('status') != 'complete':
                raise RuntimeError('Diagnostic run was replaced, cancelled or incomplete')
            after = metrics(client)
            result.update(label=args.label, input_mode=args.mode, target_id=client.target_id,
                          host_start_monotonic=host_start,
                          performance_delta={k: after[k] - before[k] for k in before if k in after},
                          injection_calls=deliveries)
            with path.open('x') as stream:
                json.dump(result, stream)
            summary = {k: v for k, v in result.items()
                       if k not in ['frames', 'wheels', 'scrolls', 'injection_calls', 'long_tasks']}
            summary['wheel_events'] = len(result['wheels'])
            summary['long_task_count'] = len(result['long_tasks'])
            print(json.dumps(summary), flush=True)
        finally:
            cleanup(lambda: cancel_probe(client, run_id), 'Page cancellation')


if __name__ == '__main__':
    with interrupt_cleanup():
        main()
