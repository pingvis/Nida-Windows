"""Reproduce a navigation frame matching the smoke test's white foreground."""

import queue
import runpy
import unittest
from pathlib import Path

host = runpy.run_path(str(Path(__file__).with_name('test-browser-host.py')))
expect_frame = host['expect_frame']


def frame(*, ready=True, opaque=False, translucent=(0, 32, 64, 128)):
    width, height = 320, 240
    data = bytearray(bytes((255, 255, 255, 255)) * width * height) if opaque else bytearray(width * height * 4)
    for x, y, pixel in [
        (10, 10, (255, 255, 255, 255)),
        (70, 10, translucent),
        (151, 11, host['READY_BGRA'] if ready else (255, 255, 255, 255)),
    ]:
        offset = (y * width + x) * 4
        data[offset:offset + 4] = bytes(pixel)
    return b'NDB1', 11, 2, width, height, data


class FrameReadinessTests(unittest.TestCase):
    def test_white_loading_frame_is_skipped_until_fixture_paints(self):
        messages = queue.Queue()
        messages.put(frame(ready=False, opaque=True))
        messages.put(frame())
        expect_frame(messages, 11, 2, (320, 240), (255, 255, 255, 255))
        self.assertTrue(messages.empty())

    def test_real_opaque_page_still_fails(self):
        messages = queue.Queue()
        messages.put(frame(opaque=True))
        with self.assertRaisesRegex(AssertionError, 'page background lost transparency'):
            expect_frame(messages, 11, 2, (320, 240), (255, 255, 255, 255))

    def test_premultiplied_alpha_is_still_required(self):
        messages = queue.Queue()
        messages.put(frame(translucent=(0, 64, 128, 128)))
        with self.assertRaisesRegex(AssertionError, 'premultiplied alpha contract changed'):
            expect_frame(messages, 11, 2, (320, 240), (255, 255, 255, 255))

    def test_loading_frame_alone_does_not_pass(self):
        messages = queue.Queue()
        messages.put(frame(ready=False, opaque=True))
        with self.assertRaisesRegex(AssertionError, 'readiness marker='):
            expect_frame(messages, 11, 2, (320, 240), (255, 255, 255, 255), timeout=0.01)


if __name__ == '__main__':
    unittest.main()
