#!/usr/bin/env python3
"""Start/finish an observation-only physical-input capture on one pinned page."""
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


def main():
    """Save target/run identities at start and reject a mismatched finish."""
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=['start', 'finish'])
    parser.add_argument('label')
    target_argument(parser)
    args = parser.parse_args()
    validate_label(parser, args.label)
    start_path = ROOT / 'results' / f'{args.label}_manual_start.json'
    output = ROOT / 'results' / f'{args.label}_page.json'
    output.parent.mkdir(exist_ok=True)
    if output.exists() or (args.command == 'start' and start_path.exists()):
        parser.error('Output already exists')
    record = None
    if args.command == 'finish':
        if not start_path.exists():
            parser.error('Manual sample has not been started with this label')
        record = json.loads(start_path.read_text())
        if not record.get('target_id') or not record.get('run_id'):
            parser.error('Legacy manual start lacks target/run identity; start a new capture')
        if args.target_id and args.target_id != record['target_id']:
            parser.error('--target-id does not match the recorded manual target')
        args.target_id = record['target_id']
    run_id = record['run_id'] if record else uuid.uuid4().hex
    with closing(connect(args.target_id)) as client:
        leave_running = False
        try:
            if args.command == 'start':
                client.evaluate((ROOT / 'conversation_probe.js').read_text())
                params = {'runId': run_id, 'mode': 'observe', 'warmup': 0, 'duration': 600000}
                geometry = client.evaluate('window.__karereConversationProbe.start(' + json.dumps(params) + ')')
                with start_path.open('x') as stream:
                    json.dump({'wall': time.time(), 'monotonic': time.monotonic(), 'geometry': geometry,
                               'target_id': client.target_id, 'run_id': run_id}, stream)
                print(json.dumps({'armed': args.label, 'geometry': geometry, 'target_id': client.target_id}))
                leave_running = True
            else:
                report = client.evaluate('window.__karereConversationProbe?.finish(' + json.dumps(run_id) + ')')
                if not report or report.get('run_id') != run_id or report.get('status') != 'complete':
                    raise RuntimeError('Manual run is missing, replaced, cancelled or incomplete')
                report.update(label=args.label, target_id=client.target_id)
                with output.open('x') as stream:
                    json.dump(report, stream)
                print(json.dumps({'finished': args.label, 'wheel_events': len(report['wheels']),
                                  'frames': report['observed_frames']}))
        finally:
            if not leave_running:
                cleanup(lambda: cancel_probe(client, run_id), 'Page cancellation')


if __name__ == '__main__':
    with interrupt_cleanup():
        main()
