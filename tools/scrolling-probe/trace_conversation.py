#!/usr/bin/env python3
"""Collect Chrome stage names/times only; drop all trace arguments and page data."""
import argparse
import json
import sys
import urllib.request
import uuid
from collections import Counter
from contextlib import closing
from pathlib import Path

from cdp_local import Client
from probe_common import (
    cleanup,
    connect,
    interrupt_cleanup,
    run_probe_child,
    target_argument,
    validate_label,
)

ROOT = Path(__file__).resolve().parent


class TraceClient(Client):
    """Accept arbitrarily sized trace buckets while retaining only timing metadata."""

    def __init__(self, url):
        """Disable the frame cap only for this browser-wide trace connection."""
        super().__init__(url, max_frame_bytes=None)
        self.events = []
        self.finished = False
        self.started = False
        self.end_sent = False

    def receive(self):
        """Strip trace arguments immediately, including events received during calls."""
        reply = super().receive()
        if reply.get('method') == 'Tracing.dataCollected':
            for event in reply['params']['value']:
                kept = {key: event[key] for key in ['cat', 'name', 'ph', 'ts', 'dur', 'tdur', 'pid', 'tid']
                        if key in event}
                if event.get('ph') == 'M' and event.get('name') in ['process_name', 'thread_name']:
                    name = event.get('args', {}).get('name', '')
                    if name in ['Browser', 'Renderer', 'Gpu', 'CrGpuMain', 'CrRendererMain',
                                'VizCompositorThread', 'Compositor'] or name.startswith('CompositorTileWorker'):
                        kept['known_thread_name'] = name
                self.events.append(kept)
        elif reply.get('method') == 'Tracing.tracingComplete':
            self.finished = True
        return reply

    def start(self):
        """Begin a ReportEvents trace, claiming ownership only after success."""
        self.call('Tracing.start', {
            'categories': 'devtools.timeline,cc,gpu,viz,blink,disabled-by-default-devtools.timeline,disabled-by-default-gpu.service',
            'options': 'record-continuously', 'transferMode': 'ReportEvents'})
        self.started = True

    def finish(self):
        """End an owned trace once and drain until explicit completion."""
        if not self.started or self.finished:
            return
        if not self.end_sent:
            self.end_sent = True
            self.call('Tracing.end', {})
        while not self.finished:
            self.receive()


def write_trace(path, client):
    """Write only a completed, sanitized trace and remove interrupted output."""
    if not client.finished:
        raise RuntimeError('Trace incomplete; refusing to write a successful capture')
    with path.open('x') as stream:
        try:
            json.dump(client.events, stream)
        except BaseException:
            path.unlink()
            raise


def collect(client, page, command, path, run_id):
    """Keep child sampling pinned and stop/drain tracing even when it fails."""
    try:
        client.start()
        run_probe_child(command, page, run_id)
        client.finish()
        write_trace(path, client)
    finally:
        cleanup(client.finish, 'Trace shutdown')


def main():
    """Capture one selected pane and print inclusive durations with thread IDs."""
    parser = argparse.ArgumentParser()
    parser.add_argument('label')
    parser.add_argument('--target', default='conversation', choices=['conversation', 'list'])
    target_argument(parser)
    args = parser.parse_args()
    validate_label(parser, args.label)
    path = ROOT / 'results' / f'{args.label}_trace.json'
    path.parent.mkdir(exist_ok=True)
    if path.exists() or (ROOT / 'results' / f'{args.label}_page.json').exists():
        parser.error('Output already exists')
    with closing(connect(args.target_id)) as page:
        with urllib.request.urlopen('http://127.0.0.1:9333/json/version', timeout=3) as response:
            version = json.load(response)
        with closing(TraceClient(version['webSocketDebuggerUrl'])) as client:
            command = [sys.executable, str(ROOT / 'conversation_probe.py'), args.label, '--target', args.target]
            collect(client, page, command, path, uuid.uuid4().hex)
            totals, counts, metadata = Counter(), Counter(), []
            for event in client.events:
                if event.get('ph') == 'X' and event.get('dur'):
                    key = (event['pid'], event['tid'], event['name'])
                    totals[key] += event['dur']
                    counts[key] += 1
                if 'known_thread_name' in event:
                    metadata.append(event)
            print(json.dumps({'trace': path.name, 'events': len(client.events), 'threads': metadata,
                              'top_inclusive_durations': [
                                  {'pid': k[0], 'tid': k[1], 'name': k[2], 'total_ms': round(v / 1000, 1),
                                   'n': counts[k]} for k, v in totals.most_common(25)]}), flush=True)


if __name__ == '__main__':
    with interrupt_cleanup():
        main()
