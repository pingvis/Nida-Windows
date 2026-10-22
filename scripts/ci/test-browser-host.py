#!/usr/bin/env python3
"""Exercise real CEF pixels, WebSocket updates, alpha, resize and refresh.

Runs against the extracted package with an unrelated CWD. No provider tokens,
external websites, desktop windows, or OBS-specific APIs are required.
"""
import argparse
import base64
import hashlib
import http.server
import json
from pathlib import Path
import queue
import struct
import subprocess
import tempfile
import threading
import time

PAGE = b'''<!doctype html><style>html,body{margin:0;background:transparent}#box{width:40px;height:40px}</style>
<div id="box"></div><div style="position:absolute;left:60px;top:0;width:30px;height:30px;background:rgba(128,64,0,0.5)"></div><script>
let n=+(localStorage.getItem('nida-test')||0)+1;localStorage.setItem('nida-test',n);
let box=document.getElementById('box');
new WebSocket('ws://'+location.host+'/chat').onmessage=()=>{box.style.background=n===1?'red':'lime'};
addEventListener('resize',()=>{box.style.background=n===1?'blue':'yellow'});
</script>'''


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_):
        pass

    def do_GET(self):
        if self.path == '/chat':
            key = self.headers['Sec-WebSocket-Key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11'
            accept = base64.b64encode(hashlib.sha1(key.encode()).digest()).decode()
            self.send_response(101)
            self.send_header('Upgrade', 'websocket')
            self.send_header('Connection', 'Upgrade')
            self.send_header('Sec-WebSocket-Accept', accept)
            self.end_headers()
            time.sleep(0.5)
            self.wfile.write(b'\x81\x04chat')
            self.wfile.flush()
            # Keep the connection open until the test closes the browser.
            time.sleep(15)
        else:
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.send_header('Content-Length', str(len(PAGE)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(PAGE)


def exact(stream, length):
    data = bytearray()
    while len(data) < length:
        block = stream.read(length - len(data))
        if not block:
            raise EOFError('browser pipe closed')
        data.extend(block)
    return data


def exercise(helper):
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    server.daemon_threads = True
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory(prefix='Nida browser test ') as work:
        process = subprocess.Popen([str(helper), '--cache-path=' + work + '/profile', '--disable-gpu'],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, cwd=work)
        messages = queue.Queue(maxsize=32)

        def read():
            try:
                while True:
                    magic, source, generation, width, height, length = struct.unpack('<4sQQIII', exact(process.stdout, 32))
                    if magic not in (b'NDB1', b'NDE1') or length > 4096 * 4096 * 4:
                        raise ValueError('invalid frame header')
                    message = magic, source, generation, width, height, exact(process.stdout, length)
                    try:
                        messages.put_nowait(message)
                    except queue.Full:
                        pass
            except (EOFError, OSError, ValueError) as error:
                try:
                    messages.put_nowait(error)
                except queue.Full:
                    pass

        threading.Thread(target=read, daemon=True).start()

        def send(**command):
            data = json.dumps(command).encode()
            process.stdin.write(struct.pack('<I', len(data)) + data)
            process.stdin.flush()

        def expect(source, generation, size, bgra):
            deadline = time.monotonic() + 30
            while time.monotonic() < deadline:
                message = messages.get(timeout=max(0.01, deadline - time.monotonic()))
                if isinstance(message, Exception):
                    raise message
                magic, actual_source, actual_generation, width, height, data = message
                if magic == b'NDE1':
                    if data:
                        raise AssertionError(data.decode(errors='replace'))
                    continue
                if (actual_source, actual_generation, width, height) != (source, generation, *size):
                    continue
                pixel = (10 * width + 10) * 4
                if data[pixel:pixel + 4] != bytes(bgra):
                    continue
                # An actual painted foreground establishes this is not a blank
                # startup placeholder before checking transparent background.
                clear = (100 * width + 100) * 4
                assert data[clear + 3] == 0, 'page background lost transparency'
                translucent = (10 * width + 70) * 4
                actual = data[translucent:translucent + 4]
                assert all(abs(a - b) <= 2 for a, b in zip(actual, (0, 32, 64, 128))), 'CEF premultiplied alpha contract changed'
                return
            raise AssertionError('expected browser pixels did not arrive')

        url = f'http://127.0.0.1:{server.server_port}/overlay'
        try:
            send(op='configure', id=7, generation=0, url=url, width=320, height=240, fps=30)
            expect(7, 0, (320, 240), (0, 0, 255, 255))  # WebSocket painted red.
            send(op='configure', id=7, generation=1, url=url, width=640, height=360, fps=30)
            expect(7, 1, (640, 360), (255, 0, 0, 255))  # Blue: resized, did not reload.
            send(op='reload', id=7)
            expect(7, 1, (640, 360), (0, 255, 0, 255))  # Green: JS ran again; cookie/profile storage survived.
            send(op='configure', id=9, generation=0, url=url, width=320, height=240, fps=24)
            expect(9, 0, (320, 240), (0, 255, 0, 255))
            send(op='stop', id=7)
            send(op='configure', id=9, generation=1, url=url, width=480, height=270, fps=24)
            expect(9, 1, (480, 270), (0, 255, 255, 255))  # Yellow: second source remains independent.
        finally:
            process.stdin.close()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            process.stdout.close()
            server.shutdown()
        assert process.returncode == 0, f'CEF host exited with {process.returncode}'
    print('CEF host passed: live WebSocket, transparency, independent sources, resize, refresh and profile storage.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('helper', type=Path)
    parser.add_argument('--credits-output', type=Path)
    args = parser.parse_args()
    helper = args.helper.resolve(strict=True)
    if args.credits_output:
        destination = args.credits_output.resolve()
        with tempfile.TemporaryDirectory(prefix='Nida Chromium credits ') as work:
            subprocess.run([str(helper), '--cache-path=' + work + '/profile',
                            '--credits-output=' + str(destination), '--disable-gpu'],
                           cwd=work, check=True, timeout=60,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        text = destination.read_text(encoding='utf-8').lower()
        assert len(text) > 10000 and 'license' in text, 'Chromium credits were not exported'
    else:
        exercise(helper)
