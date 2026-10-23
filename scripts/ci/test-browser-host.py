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
import re
from pathlib import Path
import queue
import struct
import subprocess
import tempfile
import threading
import time

# A white foreground alone cannot distinguish the white-theme fixture from a
# transient all-white navigation frame. This opaque, uniquely colored marker
# proves that the fixture document has actually painted before testing alpha.
READY_BGRA = (73, 37, 19, 255)
READY = b'<div style="position:absolute;left:150px;top:10px;width:2px;height:2px;background:rgb(19,37,73)"></div>'

PAGE = b'''<!doctype html><style>html,body{margin:0;background:transparent}#box{width:40px;height:40px}</style>
<div id="box"></div><div style="position:absolute;left:60px;top:0;width:30px;height:30px;background:rgba(128,64,0,0.5)"></div><script>
let n=+(localStorage.getItem('nida-test')||0)+1;localStorage.setItem('nida-test',n);
let box=document.getElementById('box');
new WebSocket('ws://'+location.host+'/chat').onmessage=()=>{box.style.background=n===1?'red':'lime'};
addEventListener('resize',()=>{box.style.background=n===1?'blue':'yellow'});
</script>'''

CHAT_THEME = b'''<!doctype html><style>
html,body{margin:0;background:transparent}
yt-live-chat-renderer{display:block;width:40px;height:40px;
--yt-live-chat-background-color:rgb(16,24,32);background:white;color:white}
yt-live-chat-renderer[is-overlay-feed]{background:transparent}
</style><yt-live-chat-renderer></yt-live-chat-renderer>
<div style="position:absolute;left:60px;top:0;width:30px;height:30px;background:rgba(128,64,0,0.5)"></div>'''

INTERACT = '''<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;background:transparent}
button{position:absolute;left:0;top:0;width:40px;height:40px;padding:0;border:0}
input{position:absolute;left:0;top:50px;width:120px;height:24px}
</style><button id="accept" aria-label="Accept test cookies"></button><input id="entry" aria-label="Test text">
<div style="position:absolute;left:60px;top:0;width:30px;height:30px;background:rgba(128,64,0,0.5)"></div>
<script>
const accept=document.getElementById('accept'),entry=document.getElementById('entry');
accept.style.background=document.cookie.includes('nida-consent=yes')?'lime':'red';
accept.onclick=()=>{document.cookie='nida-consent=yes; path=/; max-age=86400';accept.style.background='lime'};
entry.oninput=()=>{accept.style.background=entry.value==='ą'?'purple':entry.value==='ą你'?'cyan':'orange'};
addEventListener('wheel',()=>{accept.style.background='blue'});
</script>'''.encode('utf-8')


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
            page = CHAT_THEME if self.path.startswith('/live_chat') else PAGE
            if self.path.startswith('/interact'):
                page = INTERACT
            if 'overlay=1' in self.path:
                page = page.replace(b'<yt-live-chat-renderer>', b'<yt-live-chat-renderer is-overlay-feed>')
            if self.path.startswith('/live_chat'):
                # Test the exact embedded script without external HTTPS/DNS or
                # bypassing YouTube's HSTS policy. Only its location parameter
                # is supplied by the fixture; the production guard is intact.
                source = (Path(__file__).resolve().parents[2] / 'browser/cef/main.cpp').read_text(encoding='utf-8')
                script = re.search(r'R"JS\((.*?)\)JS"', source, re.S).group(1)
                host = 'www.youtube.com.evil.invalid' if 'other=1' in self.path else 'www.youtube.com'
                fixture = 'new Function("location", ' + json.dumps(script) + ')(' + json.dumps({'hostname': host, 'pathname': '/live_chat'}) + ');'
                page += ('<script>' + fixture + '</script>').encode()
            page += READY
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.send_header('Content-Length', str(len(page)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            self.wfile.write(page)


def exact(stream, length):
    data = bytearray()
    while len(data) < length:
        block = stream.read(length - len(data))
        if not block:
            raise EOFError('browser pipe closed')
        data.extend(block)
    return data


def expect_frame(messages, source, generation, size, bgra, timeout=30):
    deadline = time.monotonic() + timeout
    last = 'no matching frame received'
    while time.monotonic() < deadline:
        try:
            message = messages.get(timeout=max(0.01, deadline - time.monotonic()))
        except queue.Empty:
            break
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
        ready = (11 * width + 151) * 4
        foreground = data[pixel:pixel + 4]
        marker = data[ready:ready + 4]
        last = f'foreground={tuple(foreground)}, readiness marker={tuple(marker)}'
        if foreground != bytes(bgra) or marker != bytes(READY_BGRA):
            continue
        # Readiness is independent of transparency: once the fixture paints,
        # opaque backgrounds and broken premultiplied alpha still fail here.
        clear = (100 * width + 100) * 4
        assert data[clear + 3] == 0, (
            f'page background lost transparency: source={source}, generation={generation}, '
            f'background BGRA={tuple(data[clear:clear + 4])}'
        )
        translucent = (10 * width + 70) * 4
        actual = data[translucent:translucent + 4]
        assert all(abs(a - b) <= 2 for a, b in zip(actual, (0, 32, 64, 128))), (
            f'CEF premultiplied alpha contract changed: BGRA={tuple(actual)}'
        )
        return
    raise AssertionError(
        f'expected browser pixels did not arrive: source={source}, generation={generation}, '
        f'expected foreground={bgra}; {last}'
    )


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
            expect_frame(messages, source, generation, size, bgra)

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
            # Exercise the real compatibility script with a local reproduction
            # of YouTube's mismatched theme; never depend on a live stream.
            chat = f'http://127.0.0.1:{server.server_port}/live_chat'
            send(op='configure', id=11, generation=0, url=chat, width=320, height=240, fps=24)
            expect(11, 0, (320, 240), (32, 24, 16, 255))
            send(op='configure', id=11, generation=1, url=chat + '?overlay=1', width=320, height=240, fps=24)
            expect(11, 1, (320, 240), (0, 0, 0, 0))
            # The identical markup on another host keeps its white background.
            send(op='configure', id=11, generation=2,
                 url=chat + '?other=1', width=320, height=240, fps=24)
            expect(11, 2, (320, 240), (255, 255, 255, 255))
            interactive = f'http://127.0.0.1:{server.server_port}/interact'
            send(op='configure', id=12, generation=0, url=interactive, width=320, height=240, fps=30)
            expect(12, 0, (320, 240), (0, 0, 255, 255))

            def input_event(type, **values):
                send(op='input', id=12, generation=0, type=type, **values)

            def click(x, y):
                input_event('move', x=x, y=y)
                input_event('button', x=x, y=y, button=0, pressed=True)
                input_event('button', x=x, y=y, button=0, pressed=False)

            input_event('focus', focused=True)
            click(10, 10)  # Consent is a real DOM click, not JavaScript injection.
            expect(12, 0, (320, 240), (0, 255, 0, 255))
            send(op='reload', id=12)
            expect(12, 0, (320, 240), (0, 255, 0, 255))  # Cookie survives navigation.
            send(op='configure', id=13, generation=0, url=interactive, width=320, height=240, fps=30)
            expect(13, 0, (320, 240), (0, 255, 0, 255))  # Same cookie profile, independent page.
            click(10, 60)
            input_event('key', code='KeyA', key='ą', text='ą', pressed=True, modifiers=0)
            input_event('key', code='KeyA', key='ą', text='ą', pressed=False, modifiers=0)
            expect(12, 0, (320, 240), (128, 0, 128, 255))
            input_event('ime', text='你', commit=False)
            input_event('ime', text='你', commit=True)
            expect(12, 0, (320, 240), (255, 255, 0, 255))
            input_event('key', code='KeyA', key='a', text='a', pressed=True, modifiers=4)
            input_event('key', code='KeyA', key='a', text='a', pressed=False, modifiers=4)
            input_event('key', code='Backspace', key='Backspace', text='', pressed=True, modifiers=0)
            input_event('key', code='Backspace', key='Backspace', text='', pressed=False, modifiers=0)
            expect(12, 0, (320, 240), (0, 165, 255, 255))
            input_event('wheel', x=10, y=10, dx=0, dy=-120)
            expect(12, 0, (320, 240), (255, 0, 0, 255))
            input_event('focus', focused=False)
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
    print('CEF host passed: live WebSocket, transparency, independent sources, resize, refresh, profile storage, scoped YouTube theme repair, cookie consent clicks, Unicode keyboard, shortcuts, IME and wheel input.')


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
