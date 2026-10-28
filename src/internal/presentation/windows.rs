//! Windows PowerPoint automation in one persistent STA PowerShell process.
//!
//! Office COM calls can block on a dialog. The helper owns all COM objects;
//! bounded reads keep them off the render thread and kill only the helper on
//! timeout. The next request reconnects to PowerPoint. User decks are JSON
//! data on stdin, never PowerShell source or command-line arguments.

use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt as _;
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Mutex};
use std::time::Duration;

use base64::Engine as _;

use super::powerpoint::{Runner, ScriptError, COMMAND_TIMEOUT};

const HELPER: &str = include_str!("windows-powerpoint.ps1");
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Default)]
pub struct WindowsCom {
    bridge: Mutex<Option<Bridge>>,
}

impl WindowsCom {
    fn send(&self, command: &serde_json::Value, timeout: Duration) -> Result<String, ScriptError> {
        let mut bridge = self
            .bridge
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if bridge.is_none() {
            *bridge = Some(Bridge::start(HELPER)?);
        }
        let result = bridge
            .as_mut()
            .expect("helper started")
            .exchange(command, timeout);
        match result {
            Ok(reply) => reply.result(),
            Err(error) => {
                // A reply arriving after a timeout must never answer the next command.
                *bridge = None;
                Err(error)
            }
        }
    }
}

impl Runner for WindowsCom {
    fn run(&self, _script: &str, _timeout: Duration) -> Result<String, ScriptError> {
        Err(ScriptError::Failed(
            "Windows requires a structured PowerPoint command".into(),
        ))
    }

    fn request(
        &self,
        command: &serde_json::Value,
        _script: &str,
        timeout: Duration,
    ) -> Result<String, ScriptError> {
        self.send(command, timeout)
    }

    fn open(&self, path: &Path) -> Result<(), ScriptError> {
        self.send(
            &serde_json::json!({"op": "open", "path": path}),
            COMMAND_TIMEOUT,
        )
        .map(|_| ())
    }

    fn window_id(&self, path: &Path) -> Result<Option<u64>, ScriptError> {
        let reply = self.send(
            &serde_json::json!({"op": "window", "path": path}),
            COMMAND_TIMEOUT,
        )?;
        reply
            .parse::<u64>()
            .map(Some)
            .map_err(|e| ScriptError::Failed(format!("invalid PowerPoint HWND: {e}")))
    }

    fn refocus(&self) {
        let command = serde_json::json!({"op": "refocus", "pid": std::process::id()});
        if let Err(error) = self.send(&command, COMMAND_TIMEOUT) {
            log::debug!("PowerPoint: could not return focus to Nida: {error}");
        }
    }
}

#[derive(serde::Deserialize)]
#[allow(clippy::struct_field_names, reason = "reply matches the bridge wire format")]
struct Reply {
    #[serde(default)]
    reply: String,
    error: Option<String>,
    message: Option<String>,
}

impl Reply {
    fn result(self) -> Result<String, ScriptError> {
        match self.error.as_deref() {
            None => Ok(self.reply),
            Some("notinstalled") => Err(ScriptError::NotInstalled),
            Some("notrunning") => Err(ScriptError::NotRunning),
            Some("nopres") => Err(ScriptError::NotOpen),
            Some("noshow") => Err(ScriptError::NoShow),
            Some(_) => Err(ScriptError::Failed(
                self.message
                    .unwrap_or_else(|| "Windows automation failed".into()),
            )),
        }
    }
}

struct Bridge {
    child: Child,
    stdin: ChildStdin,
    replies: mpsc::Receiver<Result<Reply, String>>,
}

impl Bridge {
    fn start(helper: &str) -> Result<Self, ScriptError> {
        // A small bootstrap avoids Windows' command-line length limit. The
        // embedded helper is sent as UTF-8 base64, then reads JSON lines.
        let bootstrap = r"[Console]::InputEncoding = New-Object Text.UTF8Encoding; [Console]::OutputEncoding = New-Object Text.UTF8Encoding; $code = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String([Console]::ReadLine())); & ([ScriptBlock]::Create($code))";
        let encoded = base64::engine::general_purpose::STANDARD.encode(
            bootstrap
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        );
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        let executable = Path::new(&system).join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = Command::new(executable)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Sta",
                "-EncodedCommand",
                &encoded,
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| ScriptError::Failed(format!("starting Windows PowerPoint helper: {e}")))?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (sender, replies) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let reply = line.map_err(|e| e.to_string()).and_then(|line| {
                    serde_json::from_str::<Reply>(&line)
                        .map_err(|e| format!("invalid helper reply: {e}"))
                });
                if sender.send(reply).is_err() {
                    break;
                }
            }
        });
        let mut bridge = Self {
            child,
            stdin,
            replies,
        };
        let code = base64::engine::general_purpose::STANDARD.encode(helper.as_bytes());
        writeln!(bridge.stdin, "{code}")
            .and_then(|()| bridge.stdin.flush())
            .map_err(|e| ScriptError::Failed(format!("loading PowerPoint helper: {e}")))?;
        Ok(bridge)
    }

    fn exchange(
        &mut self,
        command: &serde_json::Value,
        timeout: Duration,
    ) -> Result<Reply, ScriptError> {
        writeln!(self.stdin, "{command}")
            .and_then(|()| self.stdin.flush())
            .map_err(|e| ScriptError::Failed(format!("sending PowerPoint command: {e}")))?;
        match self.replies.recv_timeout(timeout) {
            Ok(Ok(reply)) => Ok(reply),
            Ok(Err(error)) => Err(ScriptError::Failed(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(ScriptError::TimedOut),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(ScriptError::Failed(
                "Windows PowerPoint helper stopped".into(),
            )),
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_helper_initializes_without_office() {
        let runner = WindowsCom::default();
        assert_eq!(
            runner.send(&serde_json::json!({"op": "ping"}), Duration::from_secs(20)),
            Ok("ready".into())
        );
    }

    #[test]
    fn helper_transport_keeps_unicode_paths_as_data() {
        let script = r#"
while ($null -ne ($line = [Console]::ReadLine())) {
    $request = $line | ConvertFrom-Json
    @{ reply = $request.path } | ConvertTo-Json -Compress | ForEach-Object { [Console]::WriteLine($_) }
}
"#;
        let mut bridge = Bridge::start(script).unwrap();
        let path = "C:/Talks/Čiurlionis ' $() ` deck.pptx";
        let reply = bridge
            .exchange(&serde_json::json!({"path": path}), Duration::from_secs(20))
            .unwrap()
            .result()
            .unwrap();
        assert_eq!(reply, path);
        let reply = bridge
            .exchange(
                &serde_json::json!({"path": "another deck"}),
                Duration::from_secs(5),
            )
            .unwrap()
            .result()
            .unwrap();
        assert_eq!(reply, "another deck");
    }

    #[test]
    fn a_stalled_helper_times_out() {
        let mut bridge = Bridge::start("Start-Sleep -Seconds 30").unwrap();
        assert!(matches!(
            bridge.exchange(&serde_json::json!({}), Duration::from_millis(100)),
            Err(ScriptError::TimedOut)
        ));
    }

    #[test]
    fn automation_errors_are_distinct() {
        for (code, expected) in [
            ("notinstalled", ScriptError::NotInstalled),
            ("notrunning", ScriptError::NotRunning),
            ("nopres", ScriptError::NotOpen),
            ("noshow", ScriptError::NoShow),
        ] {
            let reply = Reply {
                reply: String::new(),
                error: Some(code.into()),
                message: None,
            };
            assert_eq!(reply.result(), Err(expected));
        }
    }
}
