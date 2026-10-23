# Windows: exclude Nida from display capture

The existing **Exclude Nida** source setting now controls Windows display capture.
It temporarily marks the current process's control, output and browser interaction
windows with `WDA_EXCLUDEFROMCAPTURE`. New windows receive the policy when they
open; native dialogs are checked at most five times per second. Closing the last
display source requesting exclusion restores each surviving window's prior policy.
Changing the checkbox applies without recreating the source. Saved shows use the
existing field and default; there is no show migration.

Windows applies display affinity globally to a window, rather than per capture.
If any active display source requests exclusion, it takes effect across captures.
An explicit window source selecting a Nida window takes precedence for that window,
so direct window capture remains usable; that window will also appear in display
captures while the window source is active. Other running copies of Nida belong to
other processes and are not changed.

True omission requires Windows 10 version 2004 or later. Earlier Windows versions
interpret this flag as `WDA_MONITOR`, which can produce a black rectangle instead.
This is a capture behaviour setting, not a security boundary. See Microsoft's
[SetWindowDisplayAffinity documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity).

## Validation

Production Clippy passes for the Windows browser/capture profile. A native test
creates owned windows and verifies capture-worker activation, overlapping display
sources, a newly created window, direct window capture priority and restoration of
both default and pre-existing display affinity. The pixel test uses a small owned
colour fixture and only reads a four-by-four crop; it does not save desktop images.
The native pixel test passed the live include/exclude/include sequence on this
HDR desktop, using a hue check that tolerates Windows colour conversion. The
51 existing capture tests passed (one desktop-only test is ignored by default).
Manual application clicks require the desktop computer-use app approval, which timed
out during this session; API/native checks do not substitute for that UI verification.
