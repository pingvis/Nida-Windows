"""Exercise the real GUI executable with and without a launching console.

Run on Windows with runtime DLLs on PATH:
    python scripts/ci/test-windows-console.py target/debug/varda.exe
Each launch uses a temporary workspace and log; no running show's API is used.
"""

import ctypes
from ctypes import wintypes
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time


def check_subsystem(exe):
    with exe.open("rb") as source:
        source.seek(0x3C)
        pe_offset = struct.unpack("<I", source.read(4))[0]
        source.seek(pe_offset + 24 + 68)
        assert struct.unpack("<H", source.read(2))[0] == 2, "EXE must use GUI subsystem"


def isolated(exe):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.AttachConsole.argtypes = [wintypes.DWORD]
    kernel.GetConsoleProcessList.argtypes = [ctypes.POINTER(wintypes.DWORD), wintypes.DWORD]
    pids = (wintypes.DWORD * 64)()
    # Some Python distributions attach to a console during startup themselves.
    kernel.FreeConsole()
    assert kernel.GetConsoleProcessList(pids, len(pids)) == 0, "runner needs no console"

    with tempfile.TemporaryDirectory(prefix="nida-console-") as temporary:
        root = Path(temporary)

        def launch(name, flags, parent_console=False):
            workspace = root / name
            workspace.mkdir()
            log = workspace / "nida.log"
            env = dict(os.environ, VARDA_LOG_FILE=str(log), RUST_LOG="info")
            args = [str(exe), *flags, "--headless", "--workspace", str(workspace),
                    "--port", "0", "--no-camera", "--no-screen-capture",
                    "--no-powerpoint", "--no-ndi", "--no-osc"]
            # Files also check that attaching/allocating doesn't replace inherited
            # output handles (the benchmark and CI use the same mechanism).
            with (workspace / "stdout").open("wb") as out, (workspace / "stderr").open("wb") as err:
                process = subprocess.Popen(args, env=env, stdout=out, stderr=err)
                try:
                    deadline = time.monotonic() + 30
                    while not log.exists() or "Nida - Starting up" not in log.read_text(errors="replace"):
                        assert process.poll() is None, f"{name}: exited early: {process.returncode}"
                        assert time.monotonic() < deadline, f"{name}: startup log timed out"
                        time.sleep(0.05)
                    if parent_console:
                        count = kernel.GetConsoleProcessList(pids, len(pids))
                        assert count <= len(pids) and process.pid in list(pids)[:count], name
                    else:
                        attached = bool(kernel.AttachConsole(process.pid))
                        error = ctypes.get_last_error()
                        if attached:
                            kernel.FreeConsole()
                        assert attached == ("--console" in flags), (name, attached, error)
                        if not attached:
                            assert error == 6, (name, error)  # ERROR_INVALID_HANDLE: no console
                finally:
                    process.terminate()
                    process.wait(timeout=15)
            assert "Nida - Starting up" in (workspace / "stderr").read_text(errors="replace"), name

        launch("default", [])
        launch("console", ["--console"])
        assert kernel.AllocConsole(), "allocate launching terminal"
        try:
            launch("terminal-default", [], parent_console=True)
            launch("terminal-console", ["--console"], parent_console=True)
        finally:
            kernel.FreeConsole()

        env = dict(os.environ, VARDA_LOG_FILE=str(root / "cli.log"))
        for flags, expected, code in [
            (["--help"], "--console", 0),
            (["--version"], "varda ", 0),
            (["--invalid-option"], "unexpected argument", 2),
            (["--console", "--bench-encoders"], "The media engine bench runs on a Mac.", 0),
        ]:
            result = subprocess.run([str(exe), *flags], env=env, capture_output=True, timeout=30)
            assert result.returncode == code, (flags, result.returncode)
            assert expected.encode() in result.stdout + result.stderr, flags
    print("PASS: GUI subsystem, default launch, --console, parent terminal, redirection, CLI output")


if __name__ == "__main__":
    if sys.platform != "win32":
        raise SystemExit("Run this check on Windows.")
    binary = Path(sys.argv[1]).resolve(strict=True)
    check_subsystem(binary)
    if "--isolated" in sys.argv[2:]:
        isolated(binary)
    else:
        # CREATE_NO_WINDOW applies to this console Python supervisor. The GUI
        # app then has a parent without a console, as with an Explorer launch.
        result = subprocess.run(
            [sys.executable, str(Path(__file__).resolve()), str(binary), "--isolated"],
            creationflags=subprocess.CREATE_NO_WINDOW, capture_output=True, text=True,
        )
        print(result.stdout, end="")
        print(result.stderr, end="", file=sys.stderr)
        raise SystemExit(result.returncode)
