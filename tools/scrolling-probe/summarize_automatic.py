#!/usr/bin/env python3
"""Compare generated-page captures over their exact 15-second active window.

Paint/draw timestamps are log receipt times, not compositor presentation times.
CEF pump entry/exit and scheduling requests carry their own monotonic timestamps.
"""
import argparse
from bisect import bisect_left
from collections import Counter
import json
from pathlib import Path
import re
import statistics


def percentile(values, fraction):
    ordered = sorted(values)
    return round(ordered[int((len(ordered) - 1) * fraction)], 3) if ordered else None


def summarize(path):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    report = next(row for row in rows if row['kind'] in ('synthetic_result', 'chat_list_result'))
    start, end = report['start_epoch_ms'] / 1000, report['end_epoch_ms'] / 1000
    active = [row for row in rows if start <= row['wall'] < end]
    pumps, requests = [], []
    for row in rows:
        message = row.get('message', '')
        match = re.search(r'call mono_ns=(\d+) gap_us=(\d+) duration_us=(\d+)', message)
        if match:
            pumps.append({'ns': int(match[1]), 'gap_ms': int(match[2]) / 1000,
                          'duration_ms': int(match[3]) / 1000, 'wall': row['wall']})
        match = re.search(r'request mono_ns=(\d+) n=\d+ delay_ms=(-?\d+) timer_added=(\d+) pending_us=(\d+)', message)
        if match:
            requests.append({'ns': int(match[1]), 'delay_ms': int(match[2]),
                             'added': int(match[3]), 'pending_ms': int(match[4]) / 1000,
                             'wall': row['wall']})
    pumps.sort(key=lambda p: p['ns'])
    requests.sort(key=lambda p: p['ns'])
    selected = [p for p in pumps if start <= p['wall'] < end]
    active_requests = [p for p in requests if start <= p['wall'] < end]
    pump_times = [p['ns'] for p in pumps]
    request_times = [p['ns'] for p in requests]
    request_to_next_pump = []
    for request in active_requests:
        if request['added'] and request['delay_ms'] <= 0:
            index = bisect_left(pump_times, request['ns'])
            if index < len(pump_times):
                request_to_next_pump.append((pump_times[index] - request['ns']) / 1e6)
    long_gaps = []
    for previous, current in zip(pumps, pumps[1:]):
        if start <= current['wall'] < end and current['gap_ms'] > 50:
            previous_exit = previous['ns'] + previous['duration_ms'] * 1e6
            first = bisect_left(request_times, previous_exit)
            last = bisect_left(request_times, current['ns'])
            long_gaps.append({'gap_ms': current['gap_ms'],
                              'preceding_pump_duration_ms': previous['duration_ms'],
                              'requests_between_pumps': last - first})
    messages = [row.get('message', '') for row in active]
    samples = [row for row in active if row['kind'] == 'sample']
    paint = sum('on_paint delivered=' in m or 'on_accelerated_paint delivered=' in m for m in messages)
    draw = sum('J4 draw frame=' in m for m in messages)
    rendering_logged = any('J4 draw ' in row.get('message', '') for row in rows)
    seconds = (end - start)
    return {
        'file': path.name,
        'configuration': {key: rows[0].get(key) for key in ('settings', 'fast_backstop', 'synthetic_gpu', 'chat_list', 'gsk_renderer', 'schedule_probe')},
        'page': {key: value for key, value in report.items() if key not in ('kind', 't', 'wall', 'phase')},
        'rendering_callbacks_logged': rendering_logged,
        'paint_callbacks': paint if rendering_logged else None,
        'draw_callbacks': draw if rendering_logged else None,
        'paint_callbacks_per_second': round(paint / seconds, 2) if rendering_logged else None,
        'draw_callbacks_per_second': round(draw / seconds, 2) if rendering_logged else None,
        'cef_pump': {
            'calls': len(selected),
            'gap_median_ms': percentile([p['gap_ms'] for p in selected], .5),
            'gap_p95_ms': percentile([p['gap_ms'] for p in selected], .95),
            'gap_max_ms': max(p['gap_ms'] for p in selected),
            'duration_p95_ms': percentile([p['duration_ms'] for p in selected], .95),
            'duration_max_ms': max(p['duration_ms'] for p in selected),
            'gaps_over_50ms': len(long_gaps),
            'long_gaps_without_between_pump_requests': sum(g['requests_between_pumps'] == 0 for g in long_gaps),
        },
        'schedule_requests': {
            'count': len(active_requests),
            'delay_ms_counts': dict(Counter(r['delay_ms'] for r in active_requests)),
            'new_timers': sum(r['added'] for r in active_requests),
            'immediate_request_to_next_pump_p95_ms': percentile(request_to_next_pump, .95),
            'immediate_request_to_next_pump_max_ms': max(request_to_next_pump, default=None),
            'urgent_postponed_events': sum('urgent_postponed' in m for m in messages),
        },
        'average_process_cpu_percent': round(statistics.mean(s['cpu_pct'] for s in samples), 1) if samples else None,
        'cpu_percent_by_role': {
            role: round(statistics.mean(sum(p['cpu_pct'] for p in s['processes'] if p['role'] == role) for s in samples), 1)
            for role in ('main', 'renderer', 'gpu-process')
        } if samples else {},
        'note': '100% CPU is one core. Callback rates are not display FPS. Active-window alignment uses page epoch timestamps and host log receipt times.'
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('captures', nargs='+', type=Path)
    args = parser.parse_args()
    print(json.dumps([summarize(path) for path in args.captures], indent=2))
