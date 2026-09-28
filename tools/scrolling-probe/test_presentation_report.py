"""Regression checks for fresh-content/presentation correlation."""
import unittest
import json
from pathlib import Path
import tempfile

from presentation_report import feedback, percentile, report


def event(message, kind='wayland'):
    """Build a timing-only record with the GTK connection's log prefix."""
    return {'kind': kind, 'wall': 10.0, 'message': '[12:00:00.000] ' + message}


def draw(serial):
    """Emit the serial captured by the production widget snapshot."""
    return event(f'J4 draw frame=100x100 serial={serial} clock_frame={serial}', 'render')


def request(identifier):
    """Emit a feedback request for the only visible test surface."""
    return event(f'wp_presentation#1.feedback(wl_surface#47, new id wp_presentation_feedback#{identifier})')


class CorrelationTests(unittest.TestCase):
    """Do not turn repeated, discarded, or ambiguously ordered buffers into FPS."""

    def test_repeated_content_keeps_the_same_serial_and_discard_is_separate(self):
        """Compositor refreshes can repeat content without another CEF frame."""
        rows = [draw(1)]
        for identifier in [10, 11]:
            rows += [request(identifier), event('wl_surface#47.commit()'),
                     event(f'wp_presentation_feedback#{identifier}.presented(0, 1, {identifier * 4166666}, 4166666, 0, 1, 7)')]
        rows += [draw(2), request(12), event('wl_surface#47.commit()'),
                 event('wp_presentation_feedback#12.discarded()')]
        frames, discards, serials = feedback(rows)
        self.assertTrue(serials)
        self.assertEqual([f['snapshot'][0] for f in frames], [1, 1])
        self.assertEqual(len(discards), 1)
        self.assertEqual(discards[0]['snapshot'][0], 2)

    def test_another_snapshot_before_commit_is_ambiguous(self):
        """An uncertain association must remain unverified."""
        rows = [draw(1), request(10), draw(2), event('wl_surface#47.commit()'),
                event('wp_presentation_feedback#10.presented(0, 1, 0, 4166666, 0, 1, 7)')]
        frames, _, _ = feedback(rows)
        self.assertIsNone(frames[0]['snapshot'])

    def test_missing_commit_and_chromium_connection_are_not_counted(self):
        """Only committed buffers on the known GTK connection are eligible."""
        rows = [draw(1), request(10),
                event('wp_presentation_feedback#10.presented(0, 1, 0, 4166666, 0, 1, 7)'),
                {'kind': 'wayland', 'wall': 10.0, 'message': '[1234.555] wp_presentation_feedback#99.presented(0, 1, 0, 4166666, 0, 1, 7)'}]
        self.assertEqual(feedback(rows)[0], [])

    def test_interval_statistics_do_not_hide_a_long_tail(self):
        """Small captures must not understate their p95 or even-count median."""
        self.assertEqual(percentile([4, 8, 12, 16], .5), 10)
        self.assertEqual(percentile([4, 4, 12], .95), 12)

    def test_unstable_samples_remain_unverified(self):
        """History loading, interruption and hiding cannot satisfy acceptance."""
        rows = [draw(1), request(10), event('wl_surface#47.commit()'),
                event('wp_presentation_feedback#10.presented(0, 1, 0, 4166666, 0, 1, 7)')]
        with tempfile.TemporaryDirectory() as directory:
            capture = Path(directory) / 'capture.jsonl'
            capture.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            for invalid in [{'scroll_height_changes': 1}, {'status': 'cancelled'}, {'visibility': 'hidden'}]:
                page = dict(start_epoch_ms=9000, end_epoch_ms=11000, **invalid)
                result = report(capture, page, single_view=True)
                self.assertFalse(result['sample_valid'])
                self.assertEqual(result['verdict'], 'presentation unverified')


if __name__ == '__main__':
    unittest.main()
