# Per-app audio on Windows

The Audio Mixer’s **+ Input → App sound** menu lists applications with Windows
audio sessions. Select an app to feed the existing mixer, buses, meters and
recording/streaming audio routing. No virtual cable is required.

This backend requires Windows build 20348 or later (including Windows 11) and
the normal Windows build’s `screen-capture` feature. Disabling screen capture
at runtime does not disable app audio. Windows process-loopback capture works
independently of the app’s selected playback endpoint. It includes the selected
process and its children; unrelated processes are excluded.

The saved app identifier uses its executable path, so quitting and reopening an
app does not require selecting its new process ID. WebView/CEF helpers are
associated with the owning app where possible. Nida and its child processes are
excluded from discovery to avoid taking its own mixer output back as an input.
Multiple independent instances of the same executable are mixed against their
Windows timestamps, rather than concatenating their PCM packets.

Discovery, activation and capture run outside the render thread. PCM is stereo
float at 48 kHz, delivered through a bounded queue. Missing packets become
silence on the shared timeline; queue overflow reports the exact lost sample
count to the existing mixer. Capture stops when its input is removed or the
selected process set changes.

Unlike the macOS process tap, this does not mute the captured app’s ordinary
playback. Nida does not change Windows output routing or volume. Choose mixer
bus destinations as appropriate for the setup.

## Validation

Offline tests cover instance mixing, gaps, bounded storage, helper ownership and
launcher boundaries. A separate native lifecycle test targets a process with no
render stream, checks that it supplies silence and stops promptly, and produces
no playback. Native signal-isolation testing needs an explicitly approved
non-speaker playback endpoint. The optional local tone fixture refuses to use
the default output and requires an explicit device index.

On the Windows test machine: two backend tests, the explicitly silent native
test, five app-worker lifecycle tests, one app-isolation mixer test, and 28 Audio
Mixer UI tests passed (one existing UI test remained ignored). Production Clippy
passed and the debug app was built and launched.

The running app discovered a separate, silent Nida test process, took its audio
as a live mixer input, marked it unavailable when that process exited, and
reconnected the same input UUID when it restarted with a new PID. The executable
identifier was retained in `.varda/stage.json`. No bus or monitor output device
was configured. Nonzero live signal isolation was not verified: speaker tone
fixtures were stopped at the user's request, and further validation used no
speaker playback. Desktop UI clicking remains unverified because the
computer-use app-approval request timed out.

API reference: [Microsoft’s process-loopback sample](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/).
