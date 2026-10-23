"""Measure process launch and logged startup phases in isolated workspaces."""

import argparse
import json
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import threading
import time
import urllib.request


def measure(executable, directory, flags, shader_library=None):
    directory.mkdir(parents=True, exist_ok=True)
    if shader_library:
        shutil.copytree(shader_library, directory / '.varda' / 'shaders', dirs_exist_ok=True)
    environment = os.environ.copy()
    environment['VARDA_NO_CLI_INSTALL'] = '1'
    environment['RUST_LOG'] = 'info'
    start = time.perf_counter()
    process = subprocess.Popen(
        [str(executable), '--workspace', str(directory), '--port', '0', '--no-osc', *flags],
        cwd=directory, env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding='utf-8', errors='replace', creationflags=subprocess.CREATE_NO_WINDOW,
    )
    received = queue.Queue()
    events = []
    api = None
    initialized = False

    def read():
        for line in process.stdout:
            received.put((round(time.perf_counter() - start, 4), line.rstrip()))
        received.put(None)

    threading.Thread(target=read, daemon=True).start()
    try:
        deadline = time.perf_counter() + 60
        while time.perf_counter() < deadline:
            try:
                event = received.get(timeout=0.1)
            except queue.Empty:
                if initialized and api:
                    break
                continue
            if event is None:
                break
            at, line = event
            events.append({'seconds': at, 'message': line})
            if '[STARTUP]' in line:
                print(f'{directory.name}: {at:.3f}s {line}', flush=True)
            if 'HTTP API server listening on' in line:
                port = re.search(r':(\d+)\s*$', line).group(1)
                api = 'http://127.0.0.1:' + port
            if '[STARTUP] Initialization complete' in line:
                initialized = True
        if not initialized:
            raise RuntimeError(f'Startup did not complete (exit={process.poll()}); see ' + str(directory / 'startup-log.json'))
        # Let the first real frames run before requesting a graceful exit.
        time.sleep(0.5)
    finally:
        (directory / 'startup-log.json').write_text(json.dumps(events, indent=2), encoding='utf-8')
        if api and process.poll() is None:
            try:
                request = urllib.request.Request(api + '/api/shutdown', method='POST')
                urllib.request.urlopen(request, timeout=3).close()
            except OSError:
                pass
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        process.stdout.close()
    phases = [event for event in events if '[STARTUP]' in event['message']]
    return {'flags': flags, 'phases': phases, 'log': str(directory / 'startup-log.json')}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('executable', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--iterations', type=int, default=2)
    parser.add_argument('--shader-library', type=Path, help='Copy this shader library into each isolated workspace')
    parser.add_argument('--case', action='append', choices=['normal', 'no-browser', 'no-devices'], help='Run only these cases (repeatable)')
    args = parser.parse_args()
    executable = args.executable.resolve(strict=True)
    args.output = args.output.resolve()
    results = []
    cases = {
        'normal': [],
        'no-browser': ['--no-html'],
        'no-devices': ['--no-camera', '--no-screen-capture', '--no-ndi'],
    }
    for name, flags in cases.items():
        if args.case and name not in args.case:
            continue
        for iteration in range(args.iterations):
            result = measure(executable, args.output / f'{name}-{iteration + 1}', flags, args.shader_library)
            result['case'] = name
            results.append(result)
            print(json.dumps(result), flush=True)
    (args.output / 'results.json').write_text(json.dumps(results, indent=2), encoding='utf-8')
