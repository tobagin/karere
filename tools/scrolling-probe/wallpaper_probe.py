#!/usr/bin/env python3
"""Toggle or restore the wallpaper experiment on one explicitly selected page."""
import argparse
import json
from contextlib import closing, contextmanager
from pathlib import Path

from probe_common import cleanup, connect, interrupt_cleanup, target_argument

SOURCE = Path(__file__).with_suffix('.js')
PROBE = 'window.__karereWallpaperProbe'


def initialize(client, require_production=False):
    """Save the original wallpaper state and select the production/inline mode."""
    client.evaluate(SOURCE.read_text())
    state = client.evaluate(f'({{mode:{PROBE}.mode,id:{PROBE}.id,geometry:{PROBE}.geometry}})')
    if require_production and state['mode'] != 'production':
        raise RuntimeError('--production requires the compiled wallpaper stylesheet')
    return state


def restore(client):
    """Restore an existing experiment even if its wallpaper was disconnected."""
    return client.evaluate(f'{PROBE}?.restore() ?? false')


@contextmanager
def session(client, require_production=False):
    """Restore the saved state on success, setup failure, or interruption."""
    try:
        yield initialize(client, require_production)
    finally:
        cleanup(lambda: restore(client), 'Wallpaper restoration')


def main():
    """Leave a deliberate on/off state active until an explicit restore command."""
    parser = argparse.ArgumentParser()
    parser.add_argument('state', choices=['on', 'off', 'restore'])
    parser.add_argument('--production', action='store_true', help='Require the compiled wallpaper stylesheet')
    target_argument(parser)
    args = parser.parse_args()
    with closing(connect(args.target_id)) as client:
        if args.state == 'restore':
            print(json.dumps({'restored': restore(client), 'target_id': client.target_id}))
            return
        try:
            state = initialize(client, args.production)
            value = client.evaluate(f'{PROBE}.set({json.dumps(args.state == "on")})')
            print(json.dumps(dict(state, will_change=value, target_id=client.target_id)))
        except BaseException:
            cleanup(lambda: restore(client), 'Wallpaper restoration')
            raise


if __name__ == '__main__':
    with interrupt_cleanup():
        main()
