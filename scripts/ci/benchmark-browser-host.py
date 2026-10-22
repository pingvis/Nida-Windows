#!/usr/bin/env python3
"""Measure actual CEF -> pipe -> production decoder throughput locally.

Supply a compiled benchmark-browser-conversion executable as --decoder.
Uses an isolated temporary profile and an animated inline page, never a show.
"""
import argparse
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from urllib.parse import quote


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("helper", type=Path)
    parser.add_argument("--decoder", required=True, type=Path)
    parser.add_argument("--width", type=int, default=1920)
    parser.add_argument("--height", type=int, default=1080)
    parser.add_argument("--fps", type=int, default=30)
    args = parser.parse_args()
    helper = args.helper.resolve()
    with tempfile.TemporaryDirectory(prefix="Nida browser benchmark ") as work:
        host = subprocess.Popen([str(helper), "--cache-path=" + work + "/profile"],
                                stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, cwd=helper.parent)
        decoder = subprocess.Popen([str(args.decoder.resolve()), "--stream"],
                                   stdin=host.stdout, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE)
        # Parent must not retain a reader after the decoder finishes.
        host.stdout.close()

        def send(command):
            payload = json.dumps(command).encode()
            host.stdin.write(struct.pack("<I", len(payload)) + payload)
            host.stdin.flush()

        page = """<!doctype html><style>html,body{margin:0;background:#15213b;color:white;font:32px sans-serif}</style>
<div id='clock'></div><script>function tick(t){clock.textContent=t.toFixed(0);
document.body.style.borderLeft=`${(t/20)%100}px solid cyan`;requestAnimationFrame(tick)}requestAnimationFrame(tick)</script>"""
        try:
            send(dict(op="configure", id=1, generation=1,
                      url="data:text/html," + quote(page), width=args.width,
                      height=args.height, fps=args.fps))
            out, error = decoder.communicate(timeout=35)
            if decoder.returncode:
                raise RuntimeError(error.decode(errors="replace"))
            result = json.loads(out)
            result.update(width=args.width, height=args.height, requested_fps=args.fps)
            print(json.dumps(result))
        finally:
            if decoder.poll() is None:
                decoder.kill()
                decoder.wait()
            try:
                send(dict(op="shutdown"))
                host.stdin.close()
                host.wait(timeout=10)
            except (OSError, subprocess.TimeoutExpired):
                host.kill()
                host.wait()


if __name__ == "__main__":
    main()
