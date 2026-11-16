"""Check real MIDI discovery/rescan and clean shutdown in a production build.

python scripts/ci/test-windows-midi-startup.py target/debug/varda.exe --output target/midi-native
Runtime DLLs must be on PATH. Uses fresh scratch shows and its own ephemeral API.
Unlike integration tests, this deliberately runs normal native MIDI discovery.
"""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def request(api, path, method="GET"):
    deadline = time.monotonic() + 10
    while True:
        try:
            with urllib.request.urlopen(urllib.request.Request(api + path, method=method), timeout=10) as reply:
                return json.load(reply)
        except urllib.error.HTTPError as error:
            # The listener starts before the first engine snapshot is published.
            if method != "GET" or error.code != 503 or time.monotonic() >= deadline:
                raise
            time.sleep(0.05)


def check(executable, output, shader_library, iteration):
    directory = Path(tempfile.mkdtemp(prefix=f"run-{iteration}-", dir=output))
    log_file = directory / "nida.log"
    environment = dict(os.environ, VARDA_LOG_FILE=str(log_file), VARDA_NO_CLI_INSTALL="1", RUST_LOG="info")
    process = subprocess.Popen(
        [str(executable), "--headless", "--workspace", str(directory), "--port", "0",
         "--no-osc", "--no-ndi", "--no-camera", "--no-screen-capture", "--no-powerpoint",
         "--no-html", "--shader-dir", str(shader_library)],
        cwd=directory, env=environment, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    api = None
    try:
        deadline = time.monotonic() + 60
        while True:
            assert process.poll() is None, f"startup exited with {process.returncode}: {log_file}"
            text = log_file.read_text(encoding="utf-8", errors="replace") if log_file.exists() else ""
            assert "A test build" not in text, "Use a normal application build without test-fixtures"
            assert "PANIC" not in text, text
            port = re.search(r"HTTP API server listening on .*:(\d+)", text)
            if port:
                api = "http://127.0.0.1:" + port.group(1)
            if api and "[STARTUP] MIDI ready:" in text and "[STARTUP] Initialization complete" in text:
                break
            assert time.monotonic() < deadline, f"MIDI discovery did not complete: {log_file}"
            time.sleep(0.05)

        for rescan in [False, True]:
            if rescan:
                request(api, "/api/devices/midi/scan", "POST")
            state = request(api, "/api/state/midi")
            names = [device["name"] for device in state["devices"]]
            assert not any(name.casefold() == "microsoft gs wavetable synth" for name in names), names
            (directory / ("rescan.json" if rescan else "discovery.json")).write_text(
                json.dumps(state, indent=2), encoding="utf-8",
            )
        request(api, "/api/shutdown", "POST")
        process.wait(timeout=20)
        assert process.returncode == 0, f"shutdown exited with {process.returncode}: {log_file}"
        text = log_file.read_text(encoding="utf-8", errors="replace")
        assert "PANIC" not in text and re.search(r"\bERROR\s+[^\]]*\]", text) is None, text
        print(f"PASS {iteration}: production discovery, rescan, shutdown; {len(names)} control port(s)", flush=True)
    finally:
        if process.poll() is None:
            if api:
                try:
                    request(api, "/api/shutdown", "POST")
                    process.wait(timeout=10)
                except (OSError, subprocess.TimeoutExpired):
                    pass
            if process.poll() is None:
                process.kill()
                process.wait(timeout=10)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--iterations", type=int, default=3)
    args = parser.parse_args()
    executable = args.executable.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    shaders = Path(__file__).resolve().parents[2] / "shaders"
    for iteration in range(1, args.iterations + 1):
        check(executable, output, shaders, iteration)
