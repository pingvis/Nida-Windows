Nida for Windows x64 - EXPERIMENTAL CORE BUILD

1. Extract this whole directory to a writable folder. Keep the EXE, DLLs,
   shaders and other files together. Do not run directly from inside the ZIP.
2. Double-click varda.exe. No Rust, Visual Studio, FFmpeg installation or PATH
   edits are required. Windows and a compatible GPU driver are still required.
3. The binary is unsigned. Check the download's SHA256 and source revision
   before choosing whether to run a Windows security prompt's blocked program.

This package includes the core application and screen/window capture.
HTML/Servo, face detection and Kinect are excluded. NDI is optional and its
runtime must be installed separately from ndi.video; it is not redistributed.
Blackmagic hardware support and macOS-specific app-audio are not provided.
Full 60 FPS capture, hardware encoding, secondary-display color behavior and
live-show stability are not qualified by this build. H.264/AAC is the initial
recording smoke-test profile. FFmpeg software encoding is not NVENC.

Saved workspaces normally live under your user profile in .varda; this is a
portable application folder, not a promise that all user data stays here.
To keep a workspace elsewhere, launch varda.exe --workspace "C:\My show".
To remove the application, delete its extracted directory. Your separately
stored workspace is not deleted.

build-info.json identifies the source revision, features and payload hashes.
licenses/ contains the MIT application license, the actual FFmpeg license,
its build configuration, Rust dependency notices and shader credit guidance.
The supplied FFmpeg GPL build means the combined distribution is under GPLv3;
Nida's own source remains MIT. Development builds are for evaluation; a public
release also needs the exact corresponding third-party source alongside it,
as documented by the project's existing release process.

Manual: https://github.com/pingvis/Nida-Windows/blob/main/docs/README.md
Source/build scripts: https://github.com/pingvis/Nida-Windows
