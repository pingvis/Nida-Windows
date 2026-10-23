"""Small archive fixtures for the CEF extraction path; no SDK download needed."""

import contextlib
import io
import runpy
import tarfile
import tempfile
import unittest
from pathlib import Path

extract_sdk = runpy.run_path(str(Path(__file__).with_name("extract-windows-cef.py")))[
    "extract_sdk"
]


class CefExtractionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "sdk.tar.bz2"
        self.output = self.root / "output with spaces"

    def make_archive(self, members):
        with tarfile.open(self.archive, "w:bz2") as archive:
            for name, link in members:
                entry = tarfile.TarInfo(name)
                if link:
                    entry.type = tarfile.SYMTYPE
                    entry.linkname = link
                    archive.addfile(entry)
                else:
                    entry.size = len(b"fixture")
                    archive.addfile(entry, io.BytesIO(b"fixture"))

    def test_extracts_files_and_reports_real_progress(self):
        self.make_archive([("sdk/include/header.h", None), ("sdk/Release/library.dll", None)])
        messages = io.StringIO()
        with contextlib.redirect_stdout(messages):
            extract_sdk(self.archive, self.output, "sdk", progress_seconds=0)
        self.assertEqual((self.output / "sdk/include/header.h").read_bytes(), b"fixture")
        self.assertIn("Extracted 2 files", messages.getvalue())
        self.assertIn("Extraction complete: 2 files", messages.getvalue())

    def test_rejects_unexpected_paths_and_links(self):
        for name, link in [
            ("other/header.h", None),
            ("sdk/../../escaped.txt", None),
            ("sdk/link", "../../escaped.txt"),
        ]:
            with self.subTest(name=name):
                self.make_archive([(name, link)])
                with self.assertRaises(ValueError), contextlib.redirect_stdout(io.StringIO()):
                    extract_sdk(self.archive, self.output, "sdk")
                self.assertFalse((self.root / "escaped.txt").exists())

    def test_corrupt_archive_does_not_report_success(self):
        self.archive.write_bytes(b"not a bzip2 archive")
        messages = io.StringIO()
        with self.assertRaises(tarfile.ReadError), contextlib.redirect_stdout(messages):
            extract_sdk(self.archive, self.output, "sdk")
        self.assertNotIn("Extraction complete", messages.getvalue())


if __name__ == "__main__":
    unittest.main()
