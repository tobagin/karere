"""Regression checks for fresh-content/presentation correlation."""
import unittest

from presentation_report import feedback


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


if __name__ == '__main__':
    unittest.main()
