"""Range serving must make generated video seeks reproducible."""
import functools
import http.server
from pathlib import Path
import tempfile
import threading
import unittest
import urllib.error
import urllib.request

from media_probe import QuietHandler


class GeneratedVideoRanges(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory()
        cls.data = bytes(range(256))
        (Path(cls.directory.name) / "sample.mp4").write_bytes(cls.data)
        cls.server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(QuietHandler, directory=cls.directory.name))
        cls.thread = threading.Thread(target=cls.server.serve_forever)
        cls.thread.start()
        cls.url = f"http://127.0.0.1:{cls.server.server_port}/sample.mp4"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.thread.join()
        cls.server.server_close()
        cls.directory.cleanup()

    def test_whole_file_advertises_seek_support(self):
        with urllib.request.urlopen(self.url) as reply:
            self.assertEqual(reply.status, 200)
            self.assertEqual(reply.headers['Accept-Ranges'], 'bytes')
            self.assertEqual(reply.read(), self.data)

    def test_bounded_open_and_suffix_ranges(self):
        for value, start, end in [('bytes=5-10', 5, 10), ('bytes=250-', 250, 255), ('bytes=-3', 253, 255)]:
            with self.subTest(value=value):
                request = urllib.request.Request(self.url, headers={'Range': value})
                with urllib.request.urlopen(request) as reply:
                    self.assertEqual(reply.status, 206)
                    self.assertEqual(reply.headers['Content-Range'], f'bytes {start}-{end}/256')
                    self.assertEqual(int(reply.headers['Content-Length']), end - start + 1)
                    self.assertEqual(reply.read(), self.data[start:end+1])

    def test_invalid_ranges_do_not_return_unrelated_frames(self):
        for value in ['bytes=256-', 'bytes=20-10', 'bytes=0-1,5-6', 'bytes=-0', 'bytes=-']:
            with self.subTest(value=value):
                request = urllib.request.Request(self.url, headers={'Range': value})
                with self.assertRaises(urllib.error.HTTPError) as error:
                    urllib.request.urlopen(request)
                self.assertEqual(error.exception.code, 416)
                error.exception.close()


if __name__ == '__main__':
    unittest.main()
