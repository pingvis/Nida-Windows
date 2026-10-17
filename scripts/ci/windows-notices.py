#!/usr/bin/env python3
"""Collect notices for the core Windows runtime crates and bundled shaders."""
import argparse
import importlib.util
import json
import re
import shutil
import subprocess
from pathlib import Path


def run(*args):
    return subprocess.check_output(args, text=True, encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    # Reuse the existing, platform-independent notice discovery rules, including
    # the special font notices. Importing this module does not run the Mac build.
    path = Path(__file__).with_name("bundle-macos-libs.py")
    spec = importlib.util.spec_from_file_location("mac_notices", path)
    notices = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(notices)
    features = ["--no-default-features", "--features", "screen-capture"]
    target = "x86_64-pc-windows-msvc"
    metadata = json.loads(run("cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", target, *features))
    tree = run("cargo", "tree", "--locked", "--target", target, "-e", "normal", "--prefix", "none", "--format", "{p}", *features)
    used = set(re.findall(r"^(\S+) v(\S+)", tree, re.MULTILINE))
    args.output.mkdir(parents=True, exist_ok=True)
    rows = ["# Rust runtime dependencies", "", "Names, versions and declared licenses follow. Original notices are in rust/.", ""]
    bare = set()
    for crate in metadata["packages"]:
        if (crate["name"], crate["version"]) not in used or crate["id"] == metadata["resolve"]["root"]:
            continue
        root = Path(crate["manifest_path"]).parent
        declared = notices.EXTRA_NOTICES.get(crate["name"], []) + ([crate["license_file"]] if crate.get("license_file") else [])
        rows.append(f"- {crate['name']} {crate['version']}: {crate.get('license') or 'see license file'}; {crate.get('repository') or crate.get('source') or 'vendored source'}")
        files = notices.notice_files(root, declared)
        if not files and crate.get("license"):
            bare.update(notices.spdx_ids(crate["license"]))
        for file in files:
            dest = args.output / "rust" / f"{crate['name']}-{crate['version']}" / file.relative_to(root)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(file, dest)
    if bare:
        notices.spdx_texts(sorted(bare), args.output / "rust" / "texts")
    (args.output / "RUST-CRATES.md").write_text("\n".join(rows) + "\n", encoding="utf-8")
    (args.output / "SHADERS.txt").write_text(
        "Shaders retain their embedded CREDIT/LICENSE notices. The portable package\n"
        "uses the same exclusion list as the Mac distribution: scripts/ci/shaders-left-out.txt.\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
