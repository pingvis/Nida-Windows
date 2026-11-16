# Windows RTMP reconnect qualification

Follow-up to the two reconnect failures recorded during the Windows 0.9.9
merge. Research and native Windows checks: 16 November 2026.

## Findings

The bundled FFmpeg 9 executable takes about **5.39 seconds** to fail a closed
IPv4 localhost port. The result is identical without `-rw_timeout`, with ten
seconds, and with two seconds. A native Windows socket reports connection
refused after about **2.01 seconds** on the same machine.

FFmpeg's [TCP implementation](https://raw.githubusercontent.com/FFmpeg/FFmpeg/n9.0/libavformat/tcp.c)
starts with a five-second connection timeout. Its
[RTMP implementation](https://raw.githubusercontent.com/FFmpeg/FFmpeg/n9.0/libavformat/rtmpproto.c)
opens an inner TCP connection. The measured connection delay is separate from
the sender command's ten-second `-rw_timeout` setting; changing that option
does not shorten this connection attempt. The earlier ten-second connection
timeout explanation was incomplete.

The sender also probes the MPEG-TS stream before opening the destination.
Consequently, an eight-second test limit can end before the first Windows
connection attempt finishes. The backoff test additionally waited for three
failed attempts, then timed the *end* of a fourth failure, although its purpose
was to verify that a network change *starts* a retry promptly.

The SDK's `ucrt/errno.h` assigns `ETIMEDOUT = 138`, `ECONNREFUSED = 107`,
`ECONNABORTED = 106`, and `ECONNRESET = 108`. FFmpeg reports the corresponding
negative errno, but this build prints these as `Error number -138 occurred`
etc. The old text-only explanation reduced that timeout to “The sender stopped.”
See [FFmpeg error conventions](https://ffmpeg.org/doxygen/trunk/error_8h.html)
and [Microsoft errno documentation](https://learn.microsoft.com/en-us/cpp/c-runtime-library/errno-constants?view=msvc-170).

## Changes

- Decode those numeric socket errors on Windows into existing actionable
  timeout, refused-connection and closed-connection advice. Preserve textual
  authentication/network advice precedence and leave unknown codes generic.
  Unix error numbers are not interpreted as Windows CRT numbers.
- Keep the real closed-port test and require successful publishing when its
  RTMP server comes back. Give the initial probe/connection failure up to
  fifteen seconds, and accept either an accurately reported timeout or refusal.
- Give the backoff-interruption test a local TCP server that immediately rejects
  RTMP sessions. This provides deterministic repeated failures independently
  of OS closed-port timing. Wait specifically for the four-second retry pause,
  change the network epoch, and require a **new connection generation within
  one second**, carrying the new epoch. This tests retry startup directly.

The production connection/write timeouts and supervisor retry policy are
unchanged. No preflight TCP connection is made to a real RTMP server.

## Validation

The complete streaming integration suite passes **14 tests, zero failures,
five explicitly ignored platform/hardware workloads**. This includes both
previously failing reconnect tests, closed-port recovery into a real RTMP
server, network changes during live sending, picture/audio on multiple
destinations, upload adaptation, and colour conversion.

All **75 streaming module unit tests pass**, including numeric Windows socket
errors, textual-message precedence, unknown-code fallback, network epochs,
fan-out/backoff decisions, and transport parsing. The production library,
application and streaming integration harness pass Clippy with warnings denied.
The Windows debug executable builds. Existing vendor wgpu build warnings remain.
Only changed Rust sections are formatted; inherited repository-wide formatting
and all-target test lint debt are outside this follow-up.

The one-second backoff-interruption regression also passes three additional
isolated runs (16.87, 16.81 and 16.76 seconds including retry setup). These
repeat the same test and do not add to the distinct test count.

Features:
`--locked --offline --no-default-features --features screen-capture,windows-browser`.
Streaming integration tests require a GPU with `VARDA_REQUIRE_GPU=1`; existing
macOS-native and NVIDIA hardware workloads remain explicitly ignored.

Logs are under `target/rtmp-windows-*` and are not committed. These loopback
checks do not substitute for physical Wi-Fi/cable changes or remote RTMPS
service qualification.
