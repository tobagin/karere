"""Live-tail regression: an unfinished write must not invalidate a resize."""
import io
import unittest

from gtk_resize_check import read_integrity


class LiveCaptureTests(unittest.TestCase):
    """Follow a writer that publishes records across multiple writes."""

    def test_partial_record_is_retained_for_the_next_read(self):
        stream = io.StringIO('{"kind":"integrity","checked":1}\n{"kind":"integrity","checked":')
        self.assertEqual([row['checked'] for row in read_integrity(stream)], [1])
        position = stream.tell()
        stream.seek(0, 2)
        stream.write('2}\n')
        stream.seek(position)
        self.assertEqual([row['checked'] for row in read_integrity(stream)], [2])
        self.assertEqual(list(read_integrity(stream)), [])
