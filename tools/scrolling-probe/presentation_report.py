#!/usr/bin/env python3
"""Join timing-only page samples with GTK/CEF and Wayland presentation records.

The serial correlation requires one visible KarereWebView, no DevTools window,
and an uninterrupted GTK connection. It does not infer fresh content from FPS.
"""
import argparse
import json
import math
from pathlib import Path
import re
import statistics
import time


def percentile(values, fraction):
    """Return the median or an empirical nearest-rank upper percentile."""
    ordered = sorted(values)
    if not ordered:
        return None
    value = statistics.median(ordered) if fraction == .5 else ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]
    return round(value, 3)


def cadence(frames, seconds):
    """Summarize compositor timestamps in nanoseconds over the active window."""
    frames = sorted(frames, key=lambda frame: frame['ns'])
    intervals = [(b['ns'] - a['ns']) / 1e6 for a, b in zip(frames, frames[1:])]
    refresh = statistics.median(f['refresh_ns'] for f in frames) / 1e6 if frames else 0
    return {
        'count': len(frames), 'rate': round(len(frames) / seconds, 3),
        'median_ms': percentile(intervals, .5), 'p95_ms': percentile(intervals, .95),
        'refresh_ms': round(refresh, 6),
        'unused_refresh_opportunities': sum(max(0, round(i / refresh) - 1) for i in intervals) if refresh else None,
    }


def feedback(rows):
    """Associate snapshot serials with committed GTK feedback, preserving order."""
    pending, presented, discarded = {}, [], []
    latest = None
    serial_logs = False
    for row in rows:
        message = row.get('message', '')
        if row['kind'] == 'render':
            draw = re.search(r'J4 draw frame=(\d+)x(\d+) serial=(\d+) clock_frame=(-?\d+)', message)
            if draw:
                serial_logs = True
                latest = (int(draw[3]), int(draw[4])) if int(draw[1]) > 0 and int(draw[2]) > 0 else None
        if row['kind'] != 'wayland' or not re.match(r'^\[\d\d:\d\d:\d\d\.', message):
            continue  # Chromium's other Wayland connection uses a different prefix.
        request = re.search(r'wp_presentation#\d+\.feedback\(wl_surface#(\d+), new id wp_presentation_feedback#(\d+)\)', message)
        if request:
            pending[int(request[2])] = {'surface': int(request[1]), 'snapshot': latest, 'committed': False}
        commit = re.search(r'wl_surface#(\d+)\.commit\(', message)
        if commit:
            for item in pending.values():
                if item['surface'] == int(commit[1]) and not item['committed']:
                    if item['snapshot'] != latest:
                        item['snapshot'] = None  # A second snapshot before commit is ambiguous.
                    item['committed'] = True
        event = re.search(r'wp_presentation_feedback#(\d+)\.(presented|discarded)\(([^)]*)\)', message)
        if not event:
            continue
        item = pending.pop(int(event[1]), None)
        if not item or not item['committed']:
            continue
        item['wall'] = row['wall']
        if event[2] == 'discarded':
            discarded.append(item)
        else:
            data = [int(part.strip()) for part in event[3].split(',')]
            item.update(ns=((data[0] << 32) + data[1]) * 10**9 + data[2], refresh_ns=data[3], flags=data[6])
            presented.append(item)
    # The protocol permits multiple feedback requests for one submission. They
    # are one presentation, even when the compositor sends several callbacks.
    unique = {}
    for item in presented:
        key = item['surface'], item['ns']
        if key in unique:
            previous = unique[key]
            if previous['snapshot'] != item['snapshot']:
                previous['snapshot'] = None
            previous['flags'] &= item['flags']
        else:
            unique[key] = item
    return list(unique.values()), discarded, serial_logs


def presentation_clock(rows, start, end):
    """Use #193's captured clock pairs, never the time of later analysis."""
    clocks = {
        int(match[1]) for row in rows
        if row.get('kind') == 'wayland'
        and re.match(r'^\[\d\d:\d\d:\d\d\.', row.get('message', ''))
        and (match := re.search(r'wp_presentation#\d+\.clock_id\((\d+)\)', row['message']))
    }
    pairs = [r['wall'] - r['monotonic'] for r in rows
             if 'monotonic' in r and start - 1 <= r['wall'] <= end + 1]
    if clocks != {time.CLOCK_MONOTONIC} or not pairs:
        return None, 'missing captured monotonic clock alignment'
    if max(pairs) - min(pairs) > .005:
        return None, 'capture clock changed by more than 5 ms'
    return statistics.median(pairs), None


def report(capture, page=None, single_view=False):
    """Summarize a generated sample or one reused PR #193 conversation sample."""
    with Path(capture).open() as stream:
        rows = [json.loads(line) for line in stream]
    if page is None:
        page = next(row for row in rows if row['kind'] in ('synthetic_result', 'chat_list_result'))
    start, end = page['start_epoch_ms'] / 1000, page['end_epoch_ms'] / 1000
    seconds = end - start
    if not math.isfinite(seconds) or seconds <= 0:
        raise ValueError('sample must have a finite, positive duration')
    active = [r for r in rows if start <= r['wall'] < end]
    samples = [r for r in active if r['kind'] == 'sample']
    render = [r.get('message', '') for r in active if r['kind'] == 'render']
    has_serials = any('clock_frame=' in m for m in render)
    draws = [m for m in render if 'J4 draw frame=' in m and (not has_serials or 'clock_frame=' in m)]
    paint = [m for m in render if 'on_paint delivered=' in m]
    accelerated = [m for m in render if 'on_accelerated_paint delivered=' in m]
    frames, discards, serials = feedback(rows)
    offset, clock_error = presentation_clock(rows, start, end)
    def in_sample(frame):
        """Arrival times are only a labelled fallback for legacy captures."""
        stamp = frame['ns'] / 1e9 + offset if offset is not None else frame['wall']
        return start <= stamp < end
    selected = [f for f in frames if in_sample(f)]
    by_surface = {}
    for frame in selected:
        by_surface.setdefault(frame['surface'], []).append(frame)
    surface, window = max(by_surface.items(), key=lambda item: len(item[1]), default=(None, []))
    fresh, seen = [], set()
    for frame in sorted(frames, key=lambda f: f['ns']):
        snapshot = frame['snapshot']
        if frame['surface'] != surface or snapshot is None or snapshot[0] <= 0 or snapshot[0] in seen:
            continue
        seen.add(snapshot[0])
        if in_sample(frame):
            fresh.append(frame)
    intervals = page.get('frame_intervals_ms', {})
    result = {
        'label': page.get('label', Path(capture).stem), 'capture': Path(capture).name,
        'geometry': page.get('geometry', {'width': page.get('inner_width'), 'height': page.get('inner_height'), 'dpr': page.get('device_pixel_ratio')}),
        'duration_seconds': seconds,
        'page_rate': round(page.get('observed_frames', page.get('raf_frames', 0)) / seconds, 3),
        'page_median_ms': intervals.get('p50', page.get('raf_median_ms')),
        'page_p95_ms': intervals.get('p95', page.get('raf_p95_ms')),
        'history_changes': page.get('scroll_height_changes'),
        'scroll_base': page.get('base'), 'scroll_amplitude': page.get('amplitude'),
        'cpu_paints_s': round(len(paint) / seconds, 3),
        'accelerated_callbacks_s': round(len(accelerated) / seconds, 3),
        'gtk_snapshots_s': round(len(draws) / seconds, 3),
        'cpu_percent': round(statistics.mean(s['cpu_pct'] for s in samples), 1) if samples else None,
        'cpu_by_role': {role: round(statistics.mean(sum(p['cpu_pct'] for p in s['processes'] if p['role'] == role) for s in samples), 1)
                        for role in ('main', 'renderer', 'gpu-process')} if samples else {},
        'window_presentation': dict(surface=surface, **cadence(window, seconds)),
        'discarded_feedbacks': sum(f['surface'] == surface and start <= f['wall'] < end for f in discards),
        'fresh_content_presentation': cadence(fresh, seconds) if single_view and serials and window else None,
        'presentation_clock': {'epoch_minus_monotonic': offset, 'error': clock_error},
        'transfer_observed': 'mixed' if paint and accelerated else 'accelerated' if accelerated else 'cpu' if paint else 'unobserved',
        'note': 'Captured monotonic alignment selects presentation boundaries; legacy receipt-time fallback cannot verify acceptance. Serial/commit correlation requires one visible web view. Unused refresh opportunities are not necessarily dropped frames.',
    }
    fresh_stats = result['fresh_content_presentation']
    invalid = []
    if page.get('status', 'complete') != 'complete':
        invalid.append('page sample incomplete')
    if page.get('scroll_height_changes', 0):
        invalid.append('history or layout changed')
    if page.get('visibility', 'visible') != 'visible':
        invalid.append('page not visible')
    if seconds < 14.999:
        invalid.append('less than 15 seconds measured')
    result['sample_invalid_reasons'] = invalid
    result['sample_valid'] = not invalid
    reliable = (fresh_stats and not clock_error and fresh
                and all(f['flags'] & 7 == 7 for f in window)
                and all(f['snapshot'] is not None for f in window))
    target_met = (reliable and fresh_stats['rate'] >= 235
                  and fresh_stats['median_ms'] is not None
                  and abs(fresh_stats['median_ms'] - 1000 / 240) <= .25
                  and abs(fresh_stats['refresh_ms'] - 1000 / 240) <= .25
                  and fresh_stats['p95_ms'] <= 8.33)
    result['verdict'] = 'presentation unverified' if not reliable or invalid else (
        '240 FPS verified' if target_met else 'lower ceiling measured')
    result['accelerated_sample_accepted'] = (
        result['verdict'] == '240 FPS verified' and result['transfer_observed'] == 'accelerated')
    return result


def main():
    """Write only numeric summaries; never overwrite an earlier evidence file."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    parser.add_argument('--page', type=Path, action='append')
    parser.add_argument('--single-view', action='store_true')
    args = parser.parse_args()
    pages = [json.loads(p.read_text()) for p in args.page] if args.page else [None]
    print(json.dumps([report(args.capture, p, args.single_view) for p in pages], indent=2))


if __name__ == '__main__':
    main()
