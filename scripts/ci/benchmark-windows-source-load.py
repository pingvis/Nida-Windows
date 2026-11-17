"""Measure source-add acknowledgements separately from browser/camera readiness.

Uses a fresh show and local inline HTML/image fixtures, without network pages.
Example: python scripts/ci/benchmark-windows-source-load.py target/debug/varda.exe --output target/source-load --camera-id first
"""

import argparse
import ctypes
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import statistics
import struct
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request


# Keep these aligned with app::PACING_FRAMES and render_headless publication.
FRAME_HISTORY_FRAMES = 300
STATE_PUBLICATION_FRAMES = 10


def distribution(values):
    ordered = sorted(values)
    assert ordered
    return {"samples": len(ordered), "min": ordered[0],
            "median": statistics.median(ordered),
            "p95": ordered[math.ceil(len(ordered) * 0.95) - 1],
            "max": ordered[-1]}


def process_cpu_seconds(process):
    """Windows engine-process CPU; excludes the separate CEF process tree."""
    if os.name != "nt":
        return None
    from ctypes import wintypes
    get_times = ctypes.WinDLL("kernel32", use_last_error=True).GetProcessTimes
    get_times.argtypes = [wintypes.HANDLE, *([ctypes.POINTER(wintypes.FILETIME)] * 4)]
    get_times.restype = wintypes.BOOL
    times = [wintypes.FILETIME() for _ in range(4)]
    if not get_times(int(process._handle), *(ctypes.byref(value) for value in times)):
        raise ctypes.WinError(ctypes.get_last_error())
    return sum((value.dwHighDateTime << 32) | value.dwLowDateTime for value in times[2:]) / 1e7


def wait_ready(api, uuid, kind):
    deadline = time.perf_counter() + 60
    while time.perf_counter() < deadline:
        mixer = request(api, "/api/state/mixer")
        deck = next((deck for channel in mixer["channels"] for deck in channel["decks"]
                     if deck["uuid"] == uuid), None)
        if deck:
            states = [effect.get("load_state", {"state": "ready"}) for effect in deck["effects"]]
            assert not any(state["state"] == "failed" for state in states), states
            browser = deck.get("browser") or {}
            assert not browser.get("error"), browser
            if (len(states) >= 2 and all(state["state"] == "ready" for state in states)
                    and (kind != "html" or browser.get("ready", False))):
                return deck
        time.sleep(0.02)
    raise TimeoutError(f"{kind} {uuid} did not become ready")


def measure_workflows(api, process, args, result):
    """Measure cadence/CPU diagnostics without intrusive GPU-drain profiling."""
    result["workflows"] = []

    def add(channel, kind, body):
        uuid = request(api, f"/api/channels/{channel}/decks/{kind}", "POST", body)["uuid"]
        wait_ready(api, uuid, kind)
        return uuid

    def wait_window(start, began, minimum_seconds, flush_control=False):
        # Histories contain 300 frames, not a fixed duration. The publication
        # margin also flushes frames between the starting snapshot and now.
        minimum_frames = FRAME_HISTORY_FRAMES + STATE_PUBLICATION_FRAMES
        deadline = began + max(120, minimum_seconds + 120)
        # Avoid repeatedly sampling the API during the requested measurement
        # duration; poll only if a slow scene has not replaced its history yet.
        if minimum_seconds:
            time.sleep(minimum_seconds)
        while True:
            elapsed = time.perf_counter() - began
            end = request(api, "/api/state/performance")
            frames = end["frame_count"] - start["frame_count"]
            control_frames = (end["pacing"]["control_monitors"]["main"]["shown"] -
                              start["pacing"]["control_monitors"]["main"]["shown"])
            if (elapsed >= minimum_seconds and frames >= minimum_frames
                    and (not flush_control or control_frames >= minimum_frames)):
                return end
            if time.perf_counter() >= deadline:
                raise TimeoutError(f"Only {frames} published frames in {elapsed:.1f}s; "
                                   f"need {minimum_frames}; control picks={control_frames}, "
                                   f"control flush required={flush_control}")
            time.sleep(min(0.2, max(0.02, minimum_seconds - elapsed)))

    def validate_fixture(channel, uuid, kind):
        expected = {"Channel": channel}
        deadline = time.perf_counter() + 15
        first_position = None
        while time.perf_counter() < deadline:
            mixer = request(api, "/api/state/mixer")
            main = mixer["main"]
            deck = next((deck for ch in mixer["channels"] for deck in ch["decks"]
                         if deck["uuid"] == uuid), None)
            if main["mode"] == "program" and main["program"] == expected and deck:
                browser = deck.get("browser") or {}
                assert not browser.get("error"), browser
                if kind == "html" and not browser.get("ready", False):
                    time.sleep(0.05)
                    continue
                if kind == "video":
                    playback = deck.get("video_playback") or {}
                    if playback.get("playing"):
                        position = playback["position"]
                        if first_position is None:
                            first_position = position
                        elif position != first_position:
                            return {"main": main, "video_playing": True,
                                    "video_positions": [first_position, position]}
                else:
                    return {"main": main, "browser_ready": browser.get("ready")}
            time.sleep(0.05)
        raise TimeoutError(f"{kind} fixture {uuid} is not active on Main; "
                           "video validation requires playing and a changing playhead")

    def observe(name, channel, uuid, kind):
        validation = validate_fixture(channel, uuid, kind)
        flush_start = request(api, "/api/state/performance")
        wait_window(flush_start, time.perf_counter(), 0, flush_control=args.windowed)
        for iteration in range(1, args.workflow_samples + 1):
            start = request(api, "/api/state/performance")
            cpu_start = process_cpu_seconds(process)
            began = time.perf_counter()
            end = wait_window(start, began, args.workflow_seconds)
            elapsed = time.perf_counter() - began
            cpu_end = process_cpu_seconds(process)
            frames = end["frame_count"] - start["frame_count"]
            row = {"name": name, "iteration": iteration, "seconds": elapsed,
                   "frames": frames, "fps": frames / elapsed,
                   "engine_cpu_ms_per_frame": None if cpu_start is None or not frames else
                       (cpu_end - cpu_start) * 1000 / frames,
                   "performance": end, "fixture_validation": validation}
            result["workflows"].append(row)
            print(json.dumps({key: value for key, value in row.items() if key != "performance"}), flush=True)
            validation = validate_fixture(channel, uuid, kind)

    def observe_loading(channel, uuid):
        validation = validate_fixture(channel, uuid, "shader")
        loading_channel = request(api, "/api/channels", "POST")["uuid"]
        row = {"name": "generator_with_background_defaults",
               "definition": "Sequential off-air solid additions while the loaded bars generator stays on Main; each pair of defaults becomes API-observed ready before its deck is deleted",
               "limitation": "Warm repeated construction with at most one added source outstanding; not a saturated-queue or cold-shader test, and no driver-contention attribution",
               "requested_adds": args.loading_adds, "adds": [],
               "fixture_validation_before": validation}
        result["loading_contention"] = row
        try:
            start = request(api, "/api/state/performance")
            cpu_start = process_cpu_seconds(process)
            began = time.perf_counter()
            row["performance_start"] = start
            for iteration in range(1, args.loading_adds + 1):
                add_began = time.perf_counter()
                added = request(api, f"/api/channels/{loading_channel}/decks/solid", "POST",
                                {"color": [1, 0, 0, 1]})["uuid"]
                admitted = time.perf_counter()
                wait_ready(api, added, "solid")
                ready = time.perf_counter()
                row["adds"].append({"iteration": iteration,
                                    "api_add_ms": (admitted - add_began) * 1000,
                                    "effects_ready_ms": (ready - add_began) * 1000})
                request(api, f"/api/decks/{added}", "DELETE")
            added_until = time.perf_counter()
            after_adds = request(api, "/api/state/performance")
            # If the loading phase is short, retain it and explicitly identify
            # the quiet tail needed to replace the 300-frame rolling history.
            end = wait_window(start, began, 0)
            elapsed = time.perf_counter() - began
            cpu_end = process_cpu_seconds(process)
            frames = end["frame_count"] - start["frame_count"]
            row.update({"completed_adds": len(row["adds"]), "seconds": elapsed,
                        "loading_seconds": added_until - began,
                        "tail_seconds": max(0, elapsed - (added_until - began)),
                        "frames": frames, "fps": frames / elapsed,
                        "loading_frames": after_adds["frame_count"] - start["frame_count"],
                        "tail_frames": end["frame_count"] - after_adds["frame_count"],
                        "engine_cpu_ms_per_frame": None if cpu_start is None or not frames else
                            (cpu_end - cpu_start) * 1000 / frames,
                        "api_add_ms": distribution([add["api_add_ms"] for add in row["adds"]]),
                        "effects_ready_ms": distribution([add["effects_ready_ms"] for add in row["adds"]]),
                        "performance_after_adds": after_adds, "performance_end": end,
                        "performance_window": "Snapshots describe their latest 300 frames; whole-interval CPU/frame and FPS include the explicitly recorded quiet tail",
                        "fixture_validation_after": validate_fixture(channel, uuid, "shader")})
            print(json.dumps({key: value for key, value in row.items()
                              if not key.startswith("performance_") and key != "adds"}), flush=True)
        finally:
            request(api, f"/api/channels/{loading_channel}", "DELETE")

    cases = [("solid_idle", "solid", {"color": [1, 0, 0, 1]}),
             ("generator_active", "shader", {"shader_name": "bars"})]
    if args.video:
        cases.append(("video_active", "video", {"path": str(args.video.resolve(strict=True))}))
    if not args.no_browser:
        html = ("<!doctype html><style>html,body{margin:0;background:#e52}"
                "@keyframes move{to{transform:translateX(900px)}}"
                "h1{animation:move 2s linear infinite alternate}</style><h1>Nida active probe</h1>")
        cases.append(("browser_active", "html", {"url": "data:text/html;charset=utf-8," + urllib.parse.quote(html),
                                                 "width": 1920, "height": 1080, "fps": 30}))
    for name, kind, body in cases:
        channel = request(api, "/api/channels", "POST")["uuid"]
        try:
            first = add(channel, kind, body)
            request(api, "/api/main/mode", "PUT", {"mode": "program"})
            request(api, "/api/main/take", "POST", {"source": {"Channel": channel}})
            observe(name, channel, first, kind)
            if name == "generator_active":
                observe_loading(channel, first)
            if kind == "solid":
                second = add(channel, "solid", {"color": [0, 0, 1, 1]})
                request(api, "/api/main/take", "POST", {"source": {"Deck": first}})
                time.sleep(1)
                switches = []
                for index in range(args.switches):
                    new = second if index % 2 == 0 else first
                    began = time.perf_counter()
                    request(api, "/api/main/take", "POST", {"source": {"Deck": new}, "fade": False})
                    switches.append((time.perf_counter() - began) * 1000)
                    time.sleep(0.05)
                result["switch_loaded"] = {"definition": "Main cut command acknowledgement between two loaded solids; not physical display latency",
                                            "ms": distribution(switches), "raw_ms": switches,
                                            "performance": request(api, "/api/state/performance")}
        finally:
            request(api, f"/api/channels/{channel}", "DELETE")


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
    flags = ["--workspace", str(directory), "--port", "0", "--no-osc",
             "--no-ndi", "--no-screen-capture", "--no-powerpoint", "--shader-dir",
             str(Path(__file__).resolve().parents[2] / "shaders")]
    if not args.windowed:
        flags.append("--headless")
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
              "probe_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
              "windows": platform.platform(), "workspace": str(directory), "measurements": rows,
              "build_label": args.build_label, "cache_label": args.cache_label,
              "scene": "1920x1080, fresh workspace/profile, sequential local fixtures; actual cadence in performance snapshots",
              "runner": "windowed control UI, fresh default layout" if args.windowed else "headless",
              "first_visible_frame": "physical presentation not measured; readiness signals resource milestones only",
              "readiness_measurement": "API-observed upper bound: state is published every 10 show frames, plus polling/request delay",
              "frame_parts": "engine/control parts available in pacing snapshots" if args.windowed else
                  "unavailable: headless runner does not collect frame_begin/frame_end CPU service times; cadence and process CPU remain available",
              "frame_history_frames": FRAME_HISTORY_FRAMES,
              "state_publication_frames": STATE_PUBLICATION_FRAMES,
              "control_history_flush": "310 Main monitor picks before each steady scene when windowed; includes rare abandoned window frames, so keep the window visible",
              "counter_precision": "Frame-count differences are published in batches of 10; short-window FPS and CPU/frame have corresponding boundary quantization",
              "percentiles": "median averages the middle pair; p95 uses nearest rank",
              "loading_workflow": "Warm sequential off-air solid/default-effect construction while the bars generator remains on Main; excludes cold shader and queue saturation, with no driver attribution",
              "settings": {"iterations": args.iterations, "workflows": args.workflows,
                           "windowed": args.windowed, "workflow_samples": args.workflow_samples,
                           "workflow_seconds": args.workflow_seconds, "switches": args.switches,
                           "loading_adds": args.loading_adds,
                           "video": None if args.video is None else str(args.video.resolve())},
              "backend_requested": args.backend or "default"}
    process = subprocess.Popen([str(executable), *flags], cwd=directory, env=env,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    result["pid"] = process.pid
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
        if args.video:
            cases.append(("video", {"path": str(args.video.resolve(strict=True))}))
        if args.camera_id is not None:
            devices = cameras["devices"]
            assert devices, "No webcam was enumerated; see cameras in results.json"
            camera = devices[0][1] if args.camera_id == "first" else int(args.camera_id)
            cases.append(("camera", {"camera_id": camera}))
        for kind, body in cases:
            for iteration in range(1, args.iterations + 1):
                channel_began = time.perf_counter()
                channel = request(api, "/api/channels", "POST")["uuid"]
                channel_ms = (time.perf_counter() - channel_began) * 1000
                performance_before = request(api, "/api/state/performance")
                began = time.perf_counter()
                added = request(api, f"/api/channels/{channel}/decks/{kind}", "POST", body)
                row = {"kind": kind, "iteration": iteration, "api_add_ms": round((time.perf_counter() - began) * 1000, 2),
                       "channel_add_ms": round(channel_ms, 2),
                       "resource_condition": "first of type in process" if iteration == 1 else "warm repeated type",
                       "backend_ready_ms": None, "effects_ready_ms": None,
                       "ready_definition": "not measured for this source type"}
                row["performance_before"] = performance_before
                row["performance_at_admission"] = request(api, "/api/state/performance")
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
                            row["performance_at_ready"] = request(api, "/api/state/performance")
                            break
                    assert time.perf_counter() < deadline, f"{kind} first picture timed out: {log}"
                    time.sleep(0.02)
                print(json.dumps({key: value for key, value in row.items() if not key.startswith("performance_")}), flush=True)
                request(api, f"/api/channels/{channel}", "DELETE")
                time.sleep(0.2)
        if args.workflows:
            measure_workflows(api, process, args, result)
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
    parser.add_argument("--build-label", default="unspecified", help="Record profile/features/revision; does not change the executable")
    parser.add_argument("--cache-label", default="OS/driver disk caches uncontrolled; fresh process and CEF profile")
    parser.add_argument("--video", type=Path, help="Optional local video fixture for admission and active playback")
    parser.add_argument("--workflows", action="store_true", help="Also measure loaded-source switching and unchanged scenes")
    parser.add_argument("--windowed", action="store_true", help="Show the control UI and collect its frame-part diagnostics; use a separate run from headless")
    parser.add_argument("--workflow-samples", type=int, default=3)
    parser.add_argument("--workflow-seconds", type=float, default=6, help="Minimum sample duration; also waits for 300 frames plus publication margin")
    parser.add_argument("--switches", type=int, default=30)
    parser.add_argument("--loading-adds", type=int, default=40, help="Sequential off-air solid/default-effect additions during the active-generator workflow")
    args = parser.parse_args()
    if args.iterations < 1:
        parser.error("--iterations must be positive")
    if args.workflow_samples < 1 or args.switches < 1 or args.loading_adds < 1 or args.workflow_seconds < 6:
        parser.error("workflow samples/switches/loading-adds must be positive and workflow seconds at least 6")
    run(args)
