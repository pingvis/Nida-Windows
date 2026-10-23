"""Extract the pinned CEF SDK with visible progress, independent of PATH's tar."""

import argparse
import time
import tarfile
from pathlib import Path, PurePosixPath


def extract_sdk(archive, destination, sdk_name, progress_seconds=30):
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    started = last_report = time.monotonic()
    files = total_bytes = 0
    print("[CEF] Python streaming bzip2/tar extraction started", flush=True)
    with tarfile.open(archive, "r|bz2") as source:
        for member in source:
            parts = PurePosixPath(member.name).parts
            if not parts or parts[0] != sdk_name or ".." in parts:
                raise ValueError(f"Unexpected CEF archive path: {member.name}")
            # The SDK contains ordinary files/directories; reject links and
            # device entries rather than letting them escape the staging tree.
            if not (member.isfile() or member.isdir()):
                raise ValueError(f"Unexpected CEF archive entry: {member.name}")
            source.extract(member, destination, filter="data")
            if member.isfile():
                files += 1
                total_bytes += member.size
            now = time.monotonic()
            if now - last_report >= progress_seconds:
                print(
                    f"[CEF] Extracted {files} files, {total_bytes / 2**20:.1f} MiB "
                    f"in {now - started:.0f}s; latest: {member.name}",
                    flush=True,
                )
                last_report = now
    if not files:
        raise ValueError("CEF archive contained no SDK files")
    print(
        f"[CEF] Extraction complete: {files} files, {total_bytes / 2**20:.1f} MiB "
        f"in {time.monotonic() - started:.1f}s",
        flush=True,
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive")
    parser.add_argument("destination")
    parser.add_argument("sdk_name")
    args = parser.parse_args()
    extract_sdk(args.archive, args.destination, args.sdk_name)
