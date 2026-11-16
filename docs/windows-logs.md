# Windows application logs

Windows launches open only the Nida window by default. To see diagnostics from
startup, launch `varda.exe --console` (or add `--console` to a shortcut's Target).
This reuses the launching terminal if available, otherwise it opens a console.
`RUST_LOG` still controls verbosity; for example, set `$env:RUST_LOG = 'debug'`
in PowerShell before launching `./varda.exe --console`.

Launching from an existing terminal still prints there without `--console`,
including `--help`, `--version` and argument errors. Redirected stdout/stderr
remain available to scripts and startup benchmarks. When invoking the GUI
executable from a shell, use `Start-Process -Wait` (PowerShell) or `start /wait`
(cmd) when the caller must wait for it to exit.

The optional console belongs to the application: closing it can exit Nida.
Close Nida through its application window when finished. There is no in-app
console control; the flag applies only to that launch.

Native launch regression check (with the runtime DLLs on `PATH`):
`python scripts/ci/test-windows-console.py target/debug/varda.exe`. This checks
the GUI executable subsystem, console-free launch, explicit console creation,
existing-terminal reuse, redirected output and CLI help/version/errors, using
temporary workspaces without contacting an existing application's API.

Windows builds now write application diagnostics to
`%LOCALAPPDATA%\Nida\logs\nida.log`, for portable and installed launches alike.
The folder is created automatically. Set `VARDA_LOG_FILE` to use an explicit
path instead; its parent directory is also created when needed. File-open
failure falls back to stderr and does not prevent Nida starting.

Messages still go to stderr as well, preserving terminal/CI redirection and
the startup benchmark's output capture. `RUST_LOG` continues to select the
logging level. The existing panic hook records panic information through the
same logger. The file contains application log records; it does not redirect
arbitrary stdout/stderr writes from native libraries or child processes.

Before a new write would take a nonempty file over 20 MiB, the logger closes
it, replaces `nida.log.old`, and starts a new file. An oversized log from a
previous run rotates on the next write too. A single record can exceed the
limit; records are not split or truncated. Rotation happens during a long
show, not only on the next launch. Logs from earlier launches are appended
until rotation. The active file and one previous file are retained.

`VARDA_LOG_FILE` also gives existing presentation tracing code the chosen log
directory. Unix/macOS logging selection and stdout/stderr redirection remain
unchanged. This Windows-only logger is a separate fork feature from the
shared MIDI/startup work. No PR has been opened.

Tests cover the default path, explicit Unicode paths, append across opens,
repeated in-run rotation/replacement, and an invalid destination.

Native verification on 24 October 2026 confirmed two complete startup/save/
shutdown cycles append at the default user log location. A third complete
cycle wrote to `target/startup-logs/Žurnalai/nida-clean-fixture.log` through an explicit
`VARDA_LOG_FILE`. The corrected runs had zero warnings and error records;
all three exited with code 0 and no panic. Giving an existing
directory as the log destination reported the open failure to stderr while
`--version` still completed successfully. Application regression tests include
all three rotating-log/path unit tests. Production Clippy and the
browser-enabled debug build passed.
