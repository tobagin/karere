"""Offline regression tests for capture integrity, transport and owned cleanup.

Run: python3 -m unittest discover -s tools/scrolling-probe -p 'test_*.py' -v
"""
import io
import json
import selectors
import signal
import struct
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest.mock import Mock, patch

import conversation_probe
import manual_probe
import prepare_conversation
import probe_common
import trace_conversation
import wallpaper_comparison
import wallpaper_probe
from cdp_local import Client
from summarize_conversation import summarize
from trace_conversation import TraceClient, write_trace


def observe_sigterm_cleanup():
    """Send real SIGTERM only after the production cleanup handler is installed."""
    source = """
import signal
from probe_common import interrupt_cleanup

with interrupt_cleanup():
    try:
        print('handler-ready', flush=True)
        signal.pause()
    finally:
        print('cleanup-ran', flush=True)
"""
    with subprocess.Popen([sys.executable, '-c', source],
                          cwd=Path(probe_common.__file__).resolve().parent,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as child:
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(child.stdout, selectors.EVENT_READ)
                if not selector.select(timeout=5) or child.stdout.readline() != 'handler-ready\n':
                    raise RuntimeError('Signal handler readiness was not confirmed')
            child.terminate()
            stdout, stderr = child.communicate(timeout=5)
            return {'signal_sent': signal.SIGTERM.name, 'signal_number': int(signal.SIGTERM),
                    'delivery_method': 'subprocess.Popen.terminate()', 'handler_ready': True,
                    'raw_subprocess_returncode': child.returncode,
                    'cleanup_ran': stdout == 'cleanup-ran\n',
                    'keyboard_interrupt_reported': 'KeyboardInterrupt' in stderr}
        finally:
            if child.poll() is None:
                child.kill()
                child.communicate(timeout=5)


def frame(payload, opcode=1, final=True):
    """Encode an unmasked server frame, including extended lengths."""
    data = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
    first = (128 if final else 0) | opcode
    if len(data) < 126:
        return bytes([first, len(data)]) + data
    if len(data) < 65536:
        return bytes([first, 126]) + struct.pack('!H', len(data)) + data
    return bytes([first, 127]) + struct.pack('!Q', len(data)) + data


def wire_client(kind, wire):
    """Exercise the actual parser against buffered frames without opening a socket."""
    client = object.__new__(kind)
    client.buffer = wire
    client.sequence = 0
    client.socket = Mock()
    client.socket.recv.side_effect = ConnectionError('Synthetic disconnect')
    client.max_frame_bytes = None if kind is TraceClient else 1_000_000
    client.events = []
    client.started = True
    client.end_sent = False
    client.finished = False
    client.send = Mock()
    return client


class ClockTests(unittest.TestCase):
    """Capture clock pairs must survive later analysis under a different clock."""

    def capture(self, root, label, wall, monotonic, legacy=False):
        """Write a complete numeric fixture with one pump and one paint event."""
        start = {'kind': 'start', 'wall': wall, 't': 0}
        if not legacy:
            start['monotonic'] = monotonic
        rows = [start, {'kind': 'render', 'wall': wall + 1,
                        'message': f'call mono_ns={int((monotonic + 1) * 1e9)} gap_us=16670 duration_us=50'},
                {'kind': 'exit', 'wall': wall + 5}]
        (root / f'{label}.jsonl').write_text('\n'.join(json.dumps(row) for row in rows))
        stages = [{'stage': 'paint', 'start_us': (monotonic + 1) * 1e6,
                   'end_us': (monotonic + 1) * 1e6 + 200, 'data': [0, 0, 0, 0, 20, 30]}]
        (root / f'{label}_stages.json').write_text(json.dumps(stages))
        page = {'start_epoch_ms': (wall + .5) * 1000, 'end_epoch_ms': (wall + 3) * 1000,
                'frames': [], 'wheels': [], 'long_tasks': [], 'status': 'complete'}
        (root / f'{label}_page.json').write_text(json.dumps(page))

    def test_delayed_analysis_uses_capture_clock(self):
        """A reboot/suspend before analysis cannot drop new capture events."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.capture(root, 'new', 1000, 10)
            report = summarize(root, fallback_offset=900_000)
        run = report['runs'][0]
        self.assertNotIn('clock_offset_epoch_minus_monotonic', report)
        self.assertEqual(run['clock_offset_epoch_minus_monotonic'], 990)
        self.assertEqual(run['clock_alignment_source'], 'capture_start')
        self.assertEqual(run['pump_gap_ms']['n'], 1)
        self.assertEqual(run['stage_durations_ms']['paint']['n'], 1)
        self.assertEqual(run['paint_damage_bytes']['p50'], 2400)

    def test_multiple_captures_use_independent_offsets(self):
        """Captures from separate boots must not share a global offset."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.capture(root, 'a', 1000, 10)
            self.capture(root, 'b', 2000, 30)
            runs = summarize(root, fallback_offset=-1)['runs']
        self.assertEqual([r['clock_offset_epoch_minus_monotonic'] for r in runs], [990, 1970])
        self.assertEqual([r['pump_gap_ms']['n'] for r in runs], [1, 1])

    def test_legacy_fallback_is_reported_and_warned(self):
        """Legacy files remain readable without silently claiming capture alignment."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.capture(root, 'old', 1000, 10, legacy=True)
            with self.assertWarnsRegex(UserWarning, 'legacy summary-time clock fallback'):
                run = summarize(root, fallback_offset=990)['runs'][0]
        self.assertEqual(run['clock_alignment_source'], 'summary_time_fallback')
        self.assertEqual(run['pump_gap_ms']['n'], 1)

    def test_incomplete_and_cancelled_samples_are_excluded(self):
        """Interrupted launcher or page runs must not become completed summaries."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.capture(root, 'a', 1000, 10)
            page_path = root / 'a_page.json'
            page = json.loads(page_path.read_text())
            for status in ['cancelled', 'incomplete', 'invalid_condition']:
                page['status'] = status
                page_path.write_text(json.dumps(page))
                self.assertEqual(summarize(root)['runs'], [])
            page['status'] = 'complete'
            page_path.write_text(json.dumps(page))
            capture_path = root / 'a.jsonl'
            capture_path.write_text('\n'.join(capture_path.read_text().splitlines()[:-1]))
            self.assertEqual(summarize(root)['runs'], [])


class TransportTests(unittest.TestCase):
    """Trace transport accepts large frames without weakening ordinary clients."""

    def test_trace_constructor_alone_opts_out_of_limit(self):
        """The trace subclass explicitly disables the cap at connection setup."""
        with patch.object(Client, '__init__', return_value=None) as initialize:
            TraceClient('ws://127.0.0.1:9333/devtools/browser/test')
        initialize.assert_called_once_with('ws://127.0.0.1:9333/devtools/browser/test', max_frame_bytes=None)

    def test_large_trace_frame_is_sanitized(self):
        """An unfragmented bucket over 1 MB succeeds and discards private arguments."""
        event = {'name': 'Raster', 'cat': 'gpu', 'ph': 'X', 'ts': 1, 'dur': 2, 'pid': 3, 'tid': 4,
                 'args': {'private_sentinel': 'x' * 1_000_001}, 'unknown': 'not retained'}
        wire = frame({'method': 'Tracing.dataCollected', 'params': {'value': [event]}})
        ordinary = wire_client(Client, wire)
        with self.assertRaisesRegex(ValueError, 'Unexpected CDP frame'):
            ordinary.receive()
        trace = wire_client(TraceClient, wire)
        trace.receive()
        self.assertEqual(trace.events, [{k: v for k, v in event.items() if k not in ['args', 'unknown']}])
        self.assertNotIn('private_sentinel', json.dumps(trace.events))

    def test_fragmented_trace_with_ping_is_reassembled(self):
        """Control frames interleaved with a fragmented trace do not corrupt JSON."""
        payload = json.dumps({'method': 'Tracing.dataCollected', 'params': {'value': [{'name': 'Raster'}]}}).encode()
        wire = frame(payload[:20], final=False) + frame(b'ping', opcode=9) + frame(payload[20:], opcode=0)
        client = wire_client(TraceClient, wire)
        client.receive()
        client.send.assert_called_once_with(b'ping', 10)
        self.assertEqual(client.events, [{'name': 'Raster'}])

    def test_masked_server_frames_still_rejected(self):
        """Opting out of trace size limits does not opt out of server mask checks."""
        for kind in [Client, TraceClient]:
            with self.subTest(kind=kind.__name__), self.assertRaises(ValueError):
                wire_client(kind, bytes([129, 128])).receive()

    def test_finish_drains_events_after_end_response_once(self):
        """A trace is complete only after the completion event, not the end reply."""
        wire = (frame({'id': 1, 'result': {}}) +
                frame({'method': 'Tracing.dataCollected', 'params': {'value': [{'name': 'Raster'}]}}) +
                frame({'method': 'Tracing.tracingComplete', 'params': {}}))
        client = wire_client(TraceClient, wire)
        client.finish()
        client.finish()
        self.assertTrue(client.finished)
        self.assertEqual(client.events, [{'name': 'Raster'}])
        self.assertEqual(client.send.call_count, 1)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'trace.json'
            write_trace(path, client)
            self.assertEqual(json.loads(path.read_text()), client.events)

    def test_completion_before_end_reply_is_supported(self):
        """Events consumed while waiting for a command response still count."""
        wire = frame({'method': 'Tracing.tracingComplete'}) + frame({'id': 1, 'result': {}})
        client = wire_client(TraceClient, wire)
        client.finish()
        self.assertTrue(client.finished)

    def test_disconnect_prevents_successful_trace_file(self):
        """A missing completion event cannot produce a success-looking output."""
        client = wire_client(TraceClient, frame({'id': 1, 'result': {}}))
        with self.assertRaises(ConnectionError):
            client.finish()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'trace.json'
            with self.assertRaisesRegex(RuntimeError, 'Trace incomplete'):
                write_trace(path, client)
            self.assertFalse(path.exists())

    def test_only_allowlisted_thread_metadata_is_retained(self):
        """Thread metadata cannot retain arbitrary page strings from args."""
        events = [{'name': 'thread_name', 'ph': 'M', 'args': {'name': name, 'secret': 'sentinel'}}
                  for name in ['CrGpuMain', 'private page string']]
        client = wire_client(TraceClient, frame({'method': 'Tracing.dataCollected', 'params': {'value': events}}))
        client.receive()
        self.assertEqual(client.events[0]['known_thread_name'], 'CrGpuMain')
        self.assertNotIn('known_thread_name', client.events[1])
        self.assertNotIn('sentinel', json.dumps(client.events))


class TargetTests(unittest.TestCase):
    """Visible target selection must be unique or explicitly pinned."""

    def select(self, visible, target_id=None):
        """Supply eligible targets and retain their fake clients for close assertions."""
        targets = [{'id': key, 'type': 'page', 'url': 'https://web.whatsapp.com/',
                    'webSocketDebuggerUrl': key} for key in visible]
        self.clients = {key: Mock(evaluate=Mock(return_value=value)) for key, value in visible.items()}
        with patch('probe_common.urllib.request.urlopen', return_value=io.StringIO(json.dumps(targets))), \
                patch('probe_common.Client', side_effect=lambda url: self.clients[url]):
            return probe_common.connect(target_id)

    def test_unique_visible_target_is_selected(self):
        """Hidden candidates close while the sole visible page remains connected."""
        client = self.select({'a': False, 'b': True})
        self.assertEqual(client.target_id, 'b')
        self.clients['a'].close.assert_called_once()
        client.close.assert_not_called()

    def test_ambiguous_targets_are_closed_and_rejected(self):
        """Never silently pick the first of two eligible visible pages."""
        with self.assertRaisesRegex(RuntimeError, 'Multiple visible diagnostic targets'):
            self.select({'a': True, 'b': True})
        for client in self.clients.values():
            client.close.assert_called_once()

    def test_explicit_target_is_used(self):
        """A pinned ID selects the same page even when another page is visible."""
        client = self.select({'a': True, 'b': True}, 'b')
        self.assertEqual(client.target_id, 'b')
        self.clients['a'].evaluate.assert_not_called()

    def test_missing_or_hidden_target_does_not_fall_back(self):
        """Losing a requested target cannot silently move a run to another page."""
        for target_id in ['missing', 'b']:
            with self.subTest(target_id=target_id), self.assertRaises(RuntimeError):
                self.select({'a': True, 'b': False}, target_id)
            self.clients['a'].evaluate.assert_not_called()

    def test_visibility_failure_closes_all_connections(self):
        """Selection errors must release both selected and current candidates."""
        clients = [Mock(evaluate=Mock(return_value=True)), Mock(evaluate=Mock(side_effect=OSError))]
        targets = [{'id': str(i), 'type': 'page', 'url': 'data:text/html,test',
                    'webSocketDebuggerUrl': str(i)} for i in range(2)]
        with patch('probe_common.urllib.request.urlopen', return_value=io.StringIO(json.dumps(targets))), \
                patch('probe_common.Client', side_effect=clients), self.assertRaises(OSError):
            probe_common.connect()
        for client in clients:
            client.close.assert_called_once()


class CleanupTests(unittest.TestCase):
    """Child and page cleanup must happen before the parent restores its state."""

    @unittest.skipUnless(sys.platform.startswith('linux'), 'Linux diagnostic signal handling')
    def test_real_sigterm_runs_cleanup_after_handler_is_ready(self):
        """The sent signal and resulting exit code are distinct observations."""
        observed = observe_sigterm_cleanup()
        self.assertEqual(observed['signal_sent'], 'SIGTERM')
        self.assertEqual(observed['signal_number'], signal.SIGTERM)
        self.assertTrue(observed['handler_ready'])
        self.assertTrue(observed['cleanup_ran'])
        self.assertTrue(observed['keyboard_interrupt_reported'])
        self.assertNotEqual(observed['raw_subprocess_returncode'], 0)

    def test_child_receives_target_and_run_identity(self):
        """The child receives the parent's target and a shared cancellation ID."""
        page = Mock(target_id='page-b')
        child = Mock(wait=Mock(return_value=0), poll=Mock(return_value=0))
        with patch('probe_common.subprocess.Popen', return_value=child) as popen, \
                patch('probe_common.cancel_probe') as cancel:
            probe_common.run_probe_child(['python3', 'probe.py'], page, 'owned')
        self.assertEqual(popen.call_args.args[0][-4:], ['--target-id', 'page-b', '--run-id', 'owned'])
        cancel.assert_called_once_with(page, 'owned')

    def test_child_failure_cancels_owned_page(self):
        """A nonzero child exit is reported after cancellation is attempted."""
        page = Mock(target_id='a')
        child = Mock(wait=Mock(return_value=7), poll=Mock(return_value=7))
        with patch('probe_common.subprocess.Popen', return_value=child), \
                patch('probe_common.cancel_probe') as cancel, self.assertRaises(subprocess.CalledProcessError):
            probe_common.run_probe_child(['python3'], page, 'owned')
        cancel.assert_called_once_with(page, 'owned')

    def test_interrupt_stops_and_reaps_child_before_cancellation(self):
        """An interrupt cannot leave the child injecting input during restoration."""
        page = Mock(target_id='a')
        events = []
        child = Mock(poll=Mock(return_value=None))
        child.wait.side_effect = [KeyboardInterrupt(), 0]
        child.terminate.side_effect = lambda: events.append('terminate')
        with patch('probe_common.subprocess.Popen', return_value=child), \
                patch('probe_common.cancel_probe', side_effect=lambda *_: events.append('cancel')), \
                self.assertRaises(KeyboardInterrupt):
            probe_common.run_probe_child(['python3'], page, 'owned')
        self.assertEqual(events, ['terminate', 'cancel'])
        self.assertEqual(child.wait.call_count, 2)
        child.wait.assert_called_with(timeout=5)

    def test_unresponsive_child_is_killed_and_reaped(self):
        """Cleanup has a bounded graceful wait before escalating to a kill."""
        child = Mock(poll=Mock(return_value=None))
        child.wait.side_effect = [subprocess.TimeoutExpired('child', 5), 0]
        probe_common.stop_child(child)
        child.kill.assert_called_once()
        self.assertEqual(child.wait.call_count, 2)

    def test_cleanup_failure_does_not_mask_original_error(self):
        """Failure reporting retains the cause even when the page disappeared."""
        original = ValueError('original failure')
        with redirect_stderr(io.StringIO()) as stderr:
            with self.assertRaises(ValueError) as caught:
                try:
                    raise original
                finally:
                    probe_common.cleanup(Mock(side_effect=ConnectionError), 'Cancellation')
        self.assertIs(caught.exception, original)
        self.assertIn('Cancellation failed', stderr.getvalue())
        with self.assertRaises(ConnectionError):
            probe_common.cleanup(Mock(side_effect=ConnectionError), 'Cancellation')

    def test_trace_child_failure_still_ends_trace(self):
        """Failed/interrupted child runs drain tracing but write no successful trace."""
        for error in [KeyboardInterrupt(), subprocess.CalledProcessError(1, ['probe'])]:
            with self.subTest(error=type(error).__name__), tempfile.TemporaryDirectory() as directory:
                client = Mock()
                path = Path(directory) / 'trace.json'
                with patch('trace_conversation.run_probe_child', side_effect=error), \
                        self.assertRaises(type(error)):
                    trace_conversation.collect(client, Mock(), ['probe'], path, 'owned')
                client.finish.assert_called_once()
                self.assertFalse(path.exists())

    def test_conversation_failure_cancels_its_run_and_closes(self):
        """An incomplete JS result must not be written as a successful page sample."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'conversation_probe.js').write_text('synthetic test source')
            client = Mock(target_id='a')
            client.evaluate.side_effect = [True, {}, {'run_id': 'owned', 'status': 'cancelled'}, True]
            with patch.object(conversation_probe, 'ROOT', root), \
                    patch.object(conversation_probe, 'connect', return_value=client), \
                    patch.object(conversation_probe, 'metrics', return_value={}), \
                    patch.object(sys, 'argv', ['conversation_probe.py', 'sample', '--run-id', 'owned']), \
                    self.assertRaisesRegex(RuntimeError, 'incomplete'):
                conversation_probe.main()
            self.assertFalse((root / 'results/sample_page.json').exists())
            self.assertIn('cancel("owned")', client.evaluate.call_args.args[0])
            client.close.assert_called_once()


class WallpaperTests(unittest.TestCase):
    """The Python driver verifies both ends of a sample and always restores state."""

    def test_missing_production_stylesheet_restores_setup(self):
        """Strict production mode cannot silently fall back to inline promotion."""
        client = Mock()
        client.evaluate.side_effect = [True, {'mode': 'inline', 'id': 's', 'geometry': {}}, True]
        with self.assertRaisesRegex(RuntimeError, 'compiled wallpaper stylesheet'):
            with wallpaper_probe.session(client, require_production=True):
                self.fail('Invalid production session entered')
        self.assertIn('restore()', client.evaluate.call_args.args[0])

    def test_failed_child_restores_wallpaper(self):
        """An interrupted comparison restores state even before a result exists."""
        for error in [KeyboardInterrupt(), subprocess.CalledProcessError(1, ['probe'])]:
            with self.subTest(error=type(error).__name__), tempfile.TemporaryDirectory() as directory:
                client = Mock(target_id='a', evaluate=Mock(return_value='auto'))
                with patch.object(wallpaper_comparison, 'ROOT', Path(directory)), \
                        patch('wallpaper_probe.initialize', return_value={'mode': 'production', 'id': 's'}), \
                        patch('wallpaper_probe.restore') as restore, \
                        patch('wallpaper_comparison.time.sleep'), \
                        patch('wallpaper_comparison.run_probe_child', side_effect=error), \
                        patch('wallpaper_comparison.print'), self.assertRaises(type(error)):
                    wallpaper_comparison.compare(client, 'sample', 'programmatic')
                restore.assert_called_once_with(client)

    def test_condition_change_invalidates_completed_child_sample(self):
        """A changed condition after sampling is retained as invalid evidence."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'results').mkdir()
            path = root / 'results/sample_original_1_page.json'
            client = Mock(target_id='a')
            client.evaluate.side_effect = ['auto', 'auto', RuntimeError('condition replaced')]

            def child(_command, _client, run_id):
                """Simulate a complete child capture before the post-check fails."""
                path.write_text(json.dumps({'run_id': run_id, 'status': 'complete'}))

            with patch.object(wallpaper_comparison, 'ROOT', root), \
                    patch('wallpaper_probe.initialize', return_value={'mode': 'production', 'id': 's'}), \
                    patch('wallpaper_probe.restore') as restore, patch('wallpaper_comparison.time.sleep'), \
                    patch('wallpaper_comparison.run_probe_child', side_effect=child), \
                    patch('wallpaper_comparison.print'), self.assertRaisesRegex(RuntimeError, 'condition replaced'):
                wallpaper_comparison.compare(client, 'sample', 'programmatic')
            report = json.loads(path.read_text())
            self.assertEqual(report['status'], 'invalid_condition')
            self.assertFalse(report['wallpaper_condition']['verified'])
            restore.assert_called_once_with(client)

    def test_annotation_cannot_overwrite_another_run(self):
        """Only the child belonging to this comparison may be marked or updated."""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'sample.json'
            original = json.dumps({'run_id': 'different', 'status': 'complete'})
            path.write_text(original)
            with self.assertRaises(RuntimeError):
                wallpaper_comparison.annotate(path, 'owned', 'invalid_condition', {})
            self.assertEqual(path.read_text(), original)


class PreparationTests(unittest.TestCase):
    """Conversation preparation must fail before accessing an unavailable pane."""

    def test_missing_pane_reports_specific_error_and_closes(self):
        """An open chat without a scroller must not trigger state reads or input."""
        client = Mock(evaluate=Mock(side_effect=[True, False]))
        with patch.object(prepare_conversation, 'connect', return_value=client) as connect, \
                patch.object(prepare_conversation.time, 'sleep'), \
                patch.object(sys, 'argv', ['prepare_conversation.py', '--target-id', 'page-a']), \
                self.assertRaisesRegex(RuntimeError, '^Scrollable conversation pane unavailable$'):
            prepare_conversation.main()
        connect.assert_called_once_with('page-a')
        self.assertEqual(client.evaluate.call_count, 2)
        client.call.assert_not_called()
        client.close.assert_called_once()

    def test_available_pane_keeps_existing_preparation_flow(self):
        """A discovered pane still reaches the loaded-history anchor and report."""
        state = {'height': 1000, 'scroll_height': 8000, 'top': 2000}
        report = dict(state, top=3547.5, images=0)
        client = Mock(evaluate=Mock(side_effect=[True, True, state, True, report]))
        with patch.object(prepare_conversation, 'connect', return_value=client), \
                patch.object(prepare_conversation.time, 'sleep'), \
                patch.object(sys, 'argv', ['prepare_conversation.py']), \
                patch('prepare_conversation.print') as output:
            prepare_conversation.main()
        self.assertEqual(json.loads(output.call_args.args[0]), report)
        self.assertIn('scrollHeight-window.__karerePane.clientHeight-3452.5',
                      client.evaluate.call_args_list[3].args[0])
        client.call.assert_not_called()
        client.close.assert_called_once()


class ManualTests(unittest.TestCase):
    """Manual start/finish must retain the page and run selected at start."""

    def test_finish_uses_saved_target_and_run(self):
        """A delayed finish attaches to the recorded page rather than the first page."""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'results').mkdir()
            (root / 'results/sample_manual_start.json').write_text(json.dumps({'target_id': 'page-b', 'run_id': 'owned'}))
            report = {'run_id': 'owned', 'status': 'complete', 'wheels': [], 'observed_frames': 10}
            client = Mock(target_id='page-b', evaluate=Mock(side_effect=[report, True]))
            with patch.object(manual_probe, 'ROOT', root), \
                    patch.object(manual_probe, 'connect', return_value=client) as connect, \
                    patch.object(sys, 'argv', ['manual_probe.py', 'finish', 'sample']), patch('manual_probe.print'):
                manual_probe.main()
            connect.assert_called_once_with('page-b')
            self.assertIn('finish("owned")', client.evaluate.call_args_list[0].args[0])
            self.assertEqual(json.loads((root / 'results/sample_page.json').read_text())['target_id'], 'page-b')
            client.close.assert_called_once()

    def test_mismatched_target_or_legacy_record_is_rejected(self):
        """Finishing cannot migrate an old or explicitly mismatched manual capture."""
        for record, extra in [({'target_id': 'a', 'run_id': 'owned'}, ['--target-id', 'b']), ({'wall': 1}, [])]:
            with self.subTest(record=record), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / 'results').mkdir()
                (root / 'results/sample_manual_start.json').write_text(json.dumps(record))
                with patch.object(manual_probe, 'ROOT', root), patch.object(manual_probe, 'connect') as connect, \
                        patch.object(sys, 'argv', ['manual_probe.py', 'finish', 'sample'] + extra), \
                        redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    manual_probe.main()
                connect.assert_not_called()


if __name__ == '__main__':
    unittest.main()
