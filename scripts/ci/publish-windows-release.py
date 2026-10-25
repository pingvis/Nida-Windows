"""Publish only verified Windows packages; existing release metadata is untouched."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import zipfile


def validate_tag(tag, package_version):
    number = r"(?:0|[1-9][0-9]*)"
    if not re.fullmatch(rf"{number}\.{number}\.{number}(?:-[a-z0-9-]+(?:\.[a-z0-9-]+)*)?", tag):
        raise ValueError("Use a plain release tag such as 0.9.2, without a leading v or spaces.")
    if tag.split("-")[0] != package_version.split("-")[0]:
        raise ValueError("The numeric release tag must match Cargo.toml on the selected source branch.")


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_assets(directory, tag, source_sha):
    if not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("A full source commit SHA is required.")
    names = [f"Nida-{tag}-windows-x64-portable.zip", f"Nida-{tag}-windows-x64-Setup.exe"]
    expected = {name for base in names for name in (base, base + ".sha256")}
    # upload-artifact preserves the common dist/ root, so ZIP and EXE arrive
    # under windows/ and windows-installer/. Also allow a flat local directory.
    paths = [path for path in directory.rglob("*") if path.is_file()]
    files = {path.name: path for path in paths}
    if len(paths) != 4 or set(files) != expected:
        raise ValueError("Expected exactly the versioned portable ZIP, Setup EXE and their two checksums.")
    for name in names:
        checksum = files[name + ".sha256"].read_text(encoding="utf-8-sig").strip()
        if checksum != f"{sha256(files[name])}  {name}":
            raise ValueError(f"Checksum does not match the downloadable file: {name}")
    with zipfile.ZipFile(files[names[0]]) as archive:
        manifest = f"Nida-{tag}-windows-x64-portable/build-info.json"
        metadata = json.loads(archive.read(manifest))
        if metadata["version"] != tag or metadata["revision"] != source_sha:
            raise ValueError("Portable manifest does not match the requested version and source commit.")
        if archive.testzip() is not None:
            raise ValueError("Portable ZIP failed its integrity check.")
    return sorted(files.values())


class GitHub:
    def __init__(self, repository):
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
            raise ValueError("Expected repository owner/name.")
        self.repository = repository

    def request(self, path, data=None):
        body = json.dumps(data).encode() if data is not None else None
        request = urllib.request.Request(
            f"https://api.github.com/repos/{self.repository}/{path}", data=body,
            headers={"Authorization": f"Bearer {os.environ['GH_TOKEN']}",
                     "Accept": "application/vnd.github+json", "Content-Type": "application/json",
                     "X-GitHub-Api-Version": "2022-11-28", "User-Agent": "Nida-Windows-release"},
        )
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if error.code == 404 and data is None:
                return None
            raise

    def release(self, tag):
        return self.request("releases/tags/" + urllib.parse.quote(tag, safe=""))

    def create_draft(self, tag, source_sha):
        return self.request("releases", {
            "tag_name": tag, "target_commitish": source_sha, "name": tag, "draft": True,
            "prerelease": "-" in tag,
            "body": f"Windows portable and installer built from commit `{source_sha}`.\n\n"
                    "This draft is ready for download and testing before publication.",
        })

    def upload(self, tag, files):
        # gh streams large uploads; do not buffer the Chromium runtime in Python.
        subprocess.run(["gh", "release", "upload", tag, "--repo", self.repository,
                        "--clobber", *map(str, files)], check=True)

    def verify_downloads(self, tag, files):
        with tempfile.TemporaryDirectory(prefix="nida-release-check-") as temporary:
            args = ["gh", "release", "download", tag, "--repo", self.repository, "--dir", temporary]
            for path in files:
                args += ["--pattern", path.name]
            subprocess.run(args, check=True)
            for path in files:
                if sha256(Path(temporary) / path.name) != sha256(path):
                    raise ValueError(f"Uploaded release download differs from the build: {path.name}")


def publish(client, tag, source_sha, files):
    release = client.release(tag)
    created = release is None
    if created:
        release = client.create_draft(tag, source_sha)
    # No release edits or Git ref updates: preserve existing notes, status and tag.
    client.upload(tag, files)
    client.verify_downloads(tag, files)
    return release["html_url"], created


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--source-sha")
    parser.add_argument("--repository")
    parser.add_argument("--assets", type=Path)
    args = parser.parse_args()
    package = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
    validate_tag(args.tag, package)
    if args.validate_only:
        print(f"Release tag {args.tag} matches source package {package}.")
        return
    if not all((args.assets, args.repository, args.source_sha)):
        parser.error("Publishing requires --assets, --repository and --source-sha.")
    files = verify_assets(args.assets, args.tag, args.source_sha)
    url, created = publish(GitHub(args.repository), args.tag, args.source_sha, files)
    summary = (f"{'Created draft release' if created else 'Replaced Windows assets in existing release'}: {url}\n\n"
               f"Source commit: `{args.source_sha}`. All four uploaded downloads match the build SHA256 hashes.\n")
    print(summary)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as stream:
            stream.write(summary)


if __name__ == "__main__":
    main()
