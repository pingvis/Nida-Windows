"""Measure source-add acknowledgements separately from browser/camera readiness.

Uses a fresh show and local inline HTML/image fixtures, without network pages.
Example: python scripts/ci/benchmark-windows-source-load.py target/debug/varda.exe --output target/source-load --camera-id first
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request


def request(api, path, method="GET", body=None):
    data = None if body is None else json.dumps(body).encode()
    message = urllib.request.Request(api + path, data=data, method=method, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(message, timeout=60) as reply:
        return json.load(reply)


def run(args):
    executable = args.executable.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix="show-", dir=output))
    log = directory / "nida.log"
    fixture = directory / "source.bmp"
    pixels = (b"\0\0\xff" * 2 + b"\0\0") * 2
    fixture.write_bytes(struct.pack("<2sIHHI", b"BM", 70, 0, 0, 54) +
                        struct.pack("<IiiHHIIiiII", 40, 2, 2, 1, 24, 0, 16, 2835, 2835, 0, 0) + pixels)
    env = dict(os.environ, VARDA_LOG_FILE=str(log), VARDA_NO_CLI_INSTALL="1",
               NIDA_BROWSER_DEBUG_PROFILE=str(directory / "cef-profile"),
               NIDA_BROWSER_PROFILE=str(directory / "cef-profile"),
               RUST_LOG="info,varda::app::engine_impl=debug,varda::internal::deck::source=debug,varda::internal::deck::loading=debug,varda::internal::html::cef_backend=debug,varda::internal::camera=debug")
    if args.backend:
        env["WGPU_BACKEND"] = args.backend
    flags = ["--headless", "--workspace", str(directory), "--port", "0", "--no-osc",
             "--no-ndi", "--no-screen-capture", "--no-powerpoint", "--shader-dir",
             str(Path(__file__).resolve().parents[2] / "shaders")]
    if args.camera_id is None:
        flags.append("--no-camera")
    if args.no_browser:
        flags.append("--no-html")
    api = None
    rows = []
    with executable.open("rb") as binary:
        digest = hashlib.file_digest(binary, "sha256").hexdigest()
    version = subprocess.run([str(executable), "--version"], env=env, capture_output=True, text=True, check=True, timeout=15).stdout.strip()
    result = {"executable": str(executable), "version": version, "sha256": digest,
              "windows": platform.platform(), "workspace": str(directory), "measurements": rows}
    process = subprocess.Popen([str(executable), *flags], cwd=directory, env=env,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        deadline = time.perf_counter() + 90
        while True:
            assert process.poll() is None, f"Nida exited during startup: {process.returncode}; {log}"
            text = log.read_text(encoding="utf-8", errors="replace") if log.exists() else ""
            port = re.search(r"HTTP API server listening on .*:(\d+)", text)
            if port:
                api = "http://127.0.0.1:" + port.group(1)
                try:
                    request(api, "/api/state/mixer")
                    break
                except urllib.error.HTTPError as error:
                    if error.code != 503:
                        raise
            assert time.perf_counter() < deadline, f"Startup timed out: {log}"
            time.sleep(0.05)
        result["gpu"] = [line for line in text.splitlines() if any(marker in line for marker in ("Using GPU:", "Backend:", "Adapter type:"))]
        cameras = request(api, "/api/state/cameras")
        result["cameras"] = cameras
        cases = [("solid", {"color": [1, 0, 0, 1]}), ("image", {"path": str(fixture)}),
                 ("shader", {"shader_name": "solid_color"})]
        if not args.no_browser:
            html = "<!doctype html><style>html,body{margin:0;background:#e52}</style><h1>Nida startup probe</h1>"
            cases.append(("html", {"url": "data:text/html;charset=utf-8," + urllib.parse.quote(html),
                                   "width": 1920, "height": 1080, "fps": 30}))
        if args.camera_id is not None:
            devices = cameras["devices"]
            assert devices, "No webcam was enumerated; see cameras in results.json"
            camera = devices[0][1] if args.camera_id == "first" else int(args.camera_id)
            cases.append(("camera", {"camera_id": camera}))
        for kind, body in cases:
            for iteration in range(1, args.iterations + 1):
                channel = request(api, "/api/channels", "POST")["uuid"]
                began = time.perf_counter()
                added = request(api, f"/api/channels/{channel}/decks/{kind}", "POST", body)
                row = {"kind": kind, "iteration": iteration, "api_add_ms": round((time.perf_counter() - began) * 1000, 2),
                       "backend_ready_ms": None, "effects_ready_ms": None,
                       "ready_definition": "not measured for this source type"}
                rows.append(row)
                deadline = began + 60
                while True:
                    state = request(api, "/api/state/mixer")
                    deck = next((deck for channel in state["channels"] for deck in channel["decks"] if deck["uuid"] == added["uuid"]), None)
                    if deck is not None:
                        effects = deck["effects"]
                        states = [effect.get("load_state", {"state": "ready"}) for effect in effects]
                        assert not any(state["state"] == "failed" for state in states), states
                        effects_ready = len(effects) >= 2 and all(state["state"] == "ready" for state in states)
                        if effects_ready and row["effects_ready_ms"] is None:
                            row["effects_ready_ms"] = round((time.perf_counter() - began) * 1000, 2)
                        if kind == "html":
                            browser = deck.get("browser") or {}
                            assert not browser.get("error"), browser["error"]
                            ready = browser.get("ready", False)
                        elif kind == "camera":
                            ready = deck.get("camera_signal") is not None
                        else:
                            ready = True
                        if ready and kind in ("html", "camera") and row["backend_ready_ms"] is None:
                            row["backend_ready_ms"] = round((time.perf_counter() - began) * 1000, 2)
                            row["ready_definition"] = "browser frame uploaded" if kind == "html" else "camera picture reported"
                        if effects_ready and ready:
                            break
                    assert time.perf_counter() < deadline, f"{kind} first picture timed out: {log}"
                    time.sleep(0.02)
                print(json.dumps(row), flush=True)
                request(api, f"/api/channels/{channel}", "DELETE")
                time.sleep(0.2)
        request(api, "/api/shutdown", "POST")
        process.wait(timeout=20)
        assert process.returncode == 0, f"Nida exit {process.returncode}"
    finally:
        if process.poll() is None and api:
            try:
                request(api, "/api/shutdown", "POST")
                process.wait(timeout=15)
            except (OSError, subprocess.TimeoutExpired):
                pass
        if process.poll() is None:
            process.kill()
            process.wait(timeout=10)
        result["exit_code"] = process.returncode
        result["log"] = str(log)
        (directory / "results.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
        print("Results: " + str(directory / "results.json"), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("executable", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--iterations", type=int, default=3)
    parser.add_argument("--camera-id", help="Numeric camera id or 'first'; omission leaves webcams unopened")
    parser.add_argument("--no-browser", action="store_true")
    parser.add_argument("--backend", choices=["dx12", "vulkan"])
    args = parser.parse_args()
    if args.iterations < 1:
        parser.error("--iterations must be positive")
    run(args)
