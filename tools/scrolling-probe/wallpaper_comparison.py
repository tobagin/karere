#!/usr/bin/env python3
"""Run a checked off/on/on/off comparison and restore the original wallpaper."""
import argparse
import json
import sys
import time
import uuid
from contextlib import closing
from functools import partial
from pathlib import Path

from probe_common import (
    cleanup,
    connect,
    interrupt_cleanup,
    run_probe_child,
    target_argument,
    validate_label,
)
from wallpaper_probe import PROBE, session

ROOT = Path(__file__).resolve().parent
CONDITIONS = [(False, 'original_1'), (True, 'layer_1'), (True, 'layer_2'), (False, 'original_2')]


def annotate(path, run_id, status, condition):
    """Attach condition validation only to a result owned by this child run."""
    if not path.exists():
        return
    report = json.loads(path.read_text())
    if report.get('run_id') != run_id:
        raise RuntimeError('Sample output does not belong to this comparison')
    report.update(status=status, wallpaper_condition=condition)
    path.write_text(json.dumps(report))


def compare(client, prefix, mode, require_production=False):
    """Check each settled condition and its document identity around a child run."""
    with session(client, require_production) as state:
        for enabled, suffix in CONDITIONS:
            run_id = uuid.uuid4().hex
            label = prefix + '_' + suffix
            path = ROOT / 'results' / f'{label}_page.json'
            condition = {'mode': state['mode'], 'expected': 'transform' if enabled else 'auto',
                         'before': None, 'after': None, 'verified': False}
            verify = f'{PROBE}.verify({json.dumps(enabled)},{json.dumps(state["id"])})'
            try:
                client.evaluate(f'{PROBE}.set({json.dumps(enabled)})')
                time.sleep(3)
                condition['before'] = client.evaluate(verify)
                print(json.dumps({'condition': condition['before'], 'sample': suffix,
                                  'mode': state['mode'], 'target_id': client.target_id}), flush=True)
                run_probe_child([sys.executable, str(ROOT / 'conversation_probe.py'), label,
                                 '--anchor-bottom', '3452.5', '--mode', mode], client, run_id)
                condition['after'] = client.evaluate(verify)
                condition['verified'] = True
                annotate(path, run_id, 'complete', condition)
            except BaseException:
                # Retain failed evidence, but exclude it from valid aggregate measurements.
                cleanup(partial(annotate, path, run_id, 'invalid_condition', condition), 'Sample invalidation')
                raise


def main():
    """Refuse reused labels before any page change and run one pinned comparison."""
    parser = argparse.ArgumentParser()
    parser.add_argument('prefix')
    parser.add_argument('--mode', default='programmatic', choices=['programmatic', 'wheel'])
    parser.add_argument('--production', action='store_true', help='Require the compiled wallpaper stylesheet')
    target_argument(parser)
    args = parser.parse_args()
    validate_label(parser, args.prefix)
    if any((ROOT / 'results' / f'{args.prefix}_{suffix}_page.json').exists() for _, suffix in CONDITIONS):
        parser.error('Comparison output already exists')
    with closing(connect(args.target_id)) as client:
        compare(client, args.prefix, args.mode, args.production)


if __name__ == '__main__':
    with interrupt_cleanup():
        main()
