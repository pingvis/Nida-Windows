"""Offline tests: no GitHub writes or release uploads."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import zipfile

spec = importlib.util.spec_from_file_location("publisher", Path(__file__).with_name("publish-windows-release.py"))
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)
TAG = "0.9.2"
SHA = "a" * 40


class ReleaseTests(unittest.TestCase):
    def assets(self, directory, revision=SHA):
        base = f"Nida-{TAG}-windows-x64"
        archive = directory / (base + "-portable.zip")
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr(base + "-portable/build-info.json", json.dumps({"version": TAG, "revision": revision}))
        installer = directory / (base + "-Setup.exe")
        installer.write_bytes(b"installer fixture")
        for path in (archive, installer):
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            path.with_name(path.name + ".sha256").write_text(f"{digest}  {path.name}\n")
        return publisher.verify_assets(directory, TAG, revision)

    def test_plain_tag_and_matching_numeric_package_version(self):
        publisher.validate_tag(TAG, TAG)
        publisher.validate_tag(TAG + "-beta.1", TAG)
        for tag in ("v0.9.2", " 0.9.2", "0.9.2 ", "0.9", "../0.9.2", "0.9.2;echo", "0.9.2-Beta", "00.9.2"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                publisher.validate_tag(tag, TAG)
        with self.assertRaisesRegex(ValueError, "Cargo.toml"):
            publisher.validate_tag(TAG, "0.9.1")

    def test_four_assets_verified_before_publishing(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            files = self.assets(directory)
            self.assertEqual(len(files), 4)
            installer = next(path for path in files if path.suffix == ".exe")
            installer.write_bytes(b"corrupted")
            with self.assertRaisesRegex(ValueError, "Checksum"):
                publisher.verify_assets(directory, TAG, SHA)

    def test_reject_other_source_commit_and_extra_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.assets(directory)
            with self.assertRaisesRegex(ValueError, "source commit"):
                publisher.verify_assets(directory, TAG, "b" * 40)
            (directory / "unrelated.exe").touch()
            with self.assertRaisesRegex(ValueError, "exactly"):
                publisher.verify_assets(directory, TAG, SHA)

    def test_artifact_preserves_windows_and_installer_directories(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            files = self.assets(directory)
            for path in files:
                child = directory / ("windows" if ".zip" in path.name else "windows-installer")
                child.mkdir(exist_ok=True)
                path.rename(child / path.name)
            verified = publisher.verify_assets(directory, TAG, SHA)
            self.assertEqual(len(verified), 4)
            duplicate = directory / verified[0].name
            duplicate.write_bytes(verified[0].read_bytes())
            with self.assertRaisesRegex(ValueError, "exactly"):
                publisher.verify_assets(directory, TAG, SHA)

    def test_existing_published_or_draft_release_is_never_edited(self):
        for draft in (False, True):
            with self.subTest(draft=draft):
                client = Mock(spec=publisher.GitHub)
                release = {"html_url": "https://example.com/release", "draft": draft,
                           "name": "Existing title", "body": "Existing notes"}
                client.release.return_value = release
                url, created = publisher.publish(client, TAG, SHA, [Path("package.zip")])
                self.assertFalse(created)
                self.assertEqual(url, release["html_url"])
                self.assertEqual([call[0] for call in client.mock_calls], ["release", "upload", "verify_downloads"])
                self.assertEqual(release["draft"], draft)

    def test_new_release_is_created_before_upload_and_verified(self):
        client = Mock(spec=publisher.GitHub)
        client.release.return_value = None
        client.create_draft.return_value = {"html_url": "https://example.com/draft"}
        self.assertTrue(publisher.publish(client, TAG, SHA, [Path("package.zip")])[1])
        self.assertEqual([call[0] for call in client.mock_calls], ["release", "create_draft", "upload", "verify_downloads"])
        client.create_draft.assert_called_once_with(TAG, SHA)

    def test_draft_creation_uses_plain_tag_and_exact_source(self):
        client = publisher.GitHub("owner/repo")
        with patch.object(client, "request") as request:
            client.create_draft(TAG, SHA)
            path, data = request.call_args.args
            self.assertEqual(path, "releases")
            self.assertEqual(data["tag_name"], TAG)
            self.assertEqual(data["target_commitish"], SHA)
            self.assertTrue(data["draft"])

    def test_upload_failure_stops_before_success_verification(self):
        client = Mock(spec=publisher.GitHub)
        client.release.return_value = {"html_url": "https://example.com/release"}
        client.upload.side_effect = RuntimeError("Upload failed")
        with self.assertRaisesRegex(RuntimeError, "Upload failed"):
            publisher.publish(client, TAG, SHA, [])
        client.verify_downloads.assert_not_called()

    def test_download_verification_detects_remote_corruption(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "package.zip"
            source.write_bytes(b"original")

            def download(args, **kwargs):
                destination = Path(args[args.index("--dir") + 1])
                (destination / source.name).write_bytes(b"different")

            with patch.object(publisher.subprocess, "run", side_effect=download), self.assertRaisesRegex(ValueError, "differs"):
                publisher.GitHub("owner/repo").verify_downloads(TAG, [source])


    def test_published_release_lookup_needs_no_listing(self):
        client = publisher.GitHub("owner/repo")
        release = {"tag_name": TAG, "draft": False}
        with patch.object(client, "request", return_value=release) as request:
            self.assertEqual(client.release(TAG), release)
            request.assert_called_once_with("releases/tags/" + TAG)

    def test_existing_pending_draft_on_later_page_is_reused(self):
        client = publisher.GitHub("owner/repo")
        draft = {"tag_name": TAG, "draft": True, "html_url": "https://example.com/draft"}
        first_page = [{"tag_name": f"other-{index}"} for index in range(100)]
        with patch.object(client, "request", side_effect=[None, first_page, [draft]]) as request, \
                patch.object(client, "create_draft") as create, \
                patch.object(client, "upload"), patch.object(client, "verify_downloads"):
            url, created = publisher.publish(client, TAG, SHA, [])
            self.assertEqual(url, draft["html_url"])
            self.assertFalse(created)
            create.assert_not_called()
            self.assertEqual([call.args[0] for call in request.call_args_list],
                             ["releases/tags/" + TAG, "releases?per_page=100&page=1",
                              "releases?per_page=100&page=2"])

    def test_absent_release_checks_all_full_pages_then_creates_draft(self):
        client = publisher.GitHub("owner/repo")
        first_page = [{"tag_name": f"other-{index}"} for index in range(100)]
        with patch.object(client, "request", side_effect=[None, first_page, []]), \
                patch.object(client, "create_draft", return_value={"html_url": "https://example.com/new"}) as create, \
                patch.object(client, "upload"), patch.object(client, "verify_downloads"):
            self.assertTrue(publisher.publish(client, TAG, SHA, [])[1])
            create.assert_called_once_with(TAG, SHA)

    def test_release_listing_failure_never_creates_draft(self):
        client = publisher.GitHub("owner/repo")
        for failure in (RuntimeError("API unavailable"),):
            with self.subTest(failure=failure), \
                    patch.object(client, "request", side_effect=[None, failure]), \
                    patch.object(client, "create_draft") as create, \
                    patch.object(client, "upload"), self.assertRaises(type(failure)):
                publisher.publish(client, TAG, SHA, [])
            create.assert_not_called()

    def test_missing_listing_is_an_error_not_a_missing_release(self):
        client = publisher.GitHub("owner/repo")
        with patch.object(client, "request", side_effect=[None, None]), \
                patch.object(client, "create_draft") as create, \
                patch.object(client, "upload"), self.assertRaises(ValueError):
            publisher.publish(client, TAG, SHA, [])
        create.assert_not_called()


if __name__ == "__main__":
    unittest.main()
