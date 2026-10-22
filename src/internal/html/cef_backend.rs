//! Windows CEF host: all process and pipe operations run off the render thread.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
type Frames = Arc<Mutex<BTreeMap<(u64, bool), Message>>>;
use super::protocol::{read_message, Message};
use serde_json::{json, Value};
use std::thread;
use std::time::Duration;

pub struct CefEngine {
    commands: SyncSender<Value>,
    frames: Frames,
    alive: Arc<AtomicBool>,
}

pub fn helper_path() -> Option<PathBuf> {
    #[cfg(test)]
    if let Some(path) = std::env::var_os("NIDA_BROWSER_TEST_HELPER") {
        let path = PathBuf::from(path);
        return path.is_file().then_some(path);
    }
    let path = std::env::current_exe()
        .ok()?
        .parent()?
        .join("browser/nida-browser-host.exe");
    path.is_file().then_some(path)
}

impl CefEngine {
    pub fn new() -> Self {
        let (commands, incoming) = mpsc::sync_channel(64);
        let frames = Arc::new(Mutex::new(BTreeMap::new()));
        let alive = Arc::new(AtomicBool::new(true));
        let worker_frames = frames.clone();
        let worker_alive = alive.clone();
        if thread::Builder::new()
            .name("html-cef-control".into())
            .spawn(move || {
                let _ = run(incoming, &worker_frames);
                worker_alive.store(false, Ordering::Release);
            })
            .is_err()
        {
            alive.store(false, Ordering::Release);
        }
        Self {
            commands,
            frames,
            alive,
        }
    }

    pub fn send(&self, command: Value) -> Result<(), &'static str> {
        self.commands
            .try_send(command)
            .map_err(|_| "Browser command queue is full or the host has stopped")
    }

    pub fn configure(
        &self,
        id: u64,
        generation: u64,
        settings: &super::BrowserSettings,
    ) -> Result<(), &'static str> {
        self.send(json!({"op":"configure", "id":id, "generation":generation, "url":settings.url, "width":settings.width, "height":settings.height, "fps":settings.fps}))
    }

    pub fn reload(&self, id: u64) -> Result<(), &'static str> {
        self.send(json!({"op":"reload", "id":id}))
    }

    pub fn stop(&self, id: u64) -> Result<(), &'static str> {
        self.send(json!({"op":"stop", "id":id}))
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }

    pub fn drain(&self) -> Vec<Message> {
        self.frames
            .try_lock()
            .map(|mut frames| std::mem::take(&mut *frames).into_values().collect())
            .unwrap_or_default()
    }
}

fn run(commands: Receiver<Value>, outgoing: &Frames) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    let helper = helper_path().ok_or_else(|| std::io::Error::other("missing browser host"))?;
    #[cfg(test)]
    let profile =
        std::env::temp_dir().join(format!("Nida CEF transport test {}", std::process::id()));
    #[cfg(not(test))]
    let profile = dirs::data_local_dir()
        .ok_or_else(|| std::io::Error::other("missing user data directory"))?
        .join("Nida/BrowserCEF");
    std::fs::create_dir_all(&profile)?;
    // Current CEF requires LPAC read/execute access to its public runtime files.
    // Apply only to the browser runtime, never to show files or profile data.
    let icacls = PathBuf::from(
        std::env::var_os("SystemRoot")
            .ok_or_else(|| std::io::Error::other("missing Windows directory"))?,
    )
    .join("System32/icacls.exe");
    if !Command::new(icacls)
        .arg(helper.parent().unwrap())
        .args(["/grant", "*S-1-15-2-2:(OI)(CI)(RX)", "/T", "/Q"])
        .creation_flags(0x0800_0000)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success()
    {
        return Err(std::io::Error::other(
            "could not grant CEF sandbox read access",
        ));
    }
    let mut child = Command::new(&helper)
        .arg(format!("--cache-path={}", profile.display()))
        .current_dir(helper.parent().unwrap())
        .creation_flags(0x0800_0000)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let job = super::cef_job::attach(&mut child)?;
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let frames = outgoing.clone();
    let reader_alive = Arc::new(AtomicBool::new(true));
    let reading = reader_alive.clone();
    let reader = thread::Builder::new()
        .name("html-cef-frames".into())
        .spawn(move || {
            while let Ok(frame) = read_message(&mut stdout) {
                if let Ok(mut pending) = frames.lock() {
                    let key = (frame.id, frame.error.is_some());
                    if pending.len() < 64 || pending.contains_key(&key) {
                        pending.insert(key, frame);
                    }
                }
            }
            reading.store(false, Ordering::Release);
        })?;
    let clock = std::time::Instant::now();
    let writer_alive = Arc::new(AtomicBool::new(true));
    let writing = writer_alive.clone();
    let write_started = Arc::new(AtomicU64::new(0));
    let started = write_started.clone();
    let writer = thread::Builder::new()
        .name("html-cef-commands".into())
        .spawn(move || {
            while let Ok(command) = commands.recv() {
                started.store(clock.elapsed().as_millis() as u64 + 1, Ordering::Release);
                let result = (|| -> std::io::Result<()> {
                    let bytes = serde_json::to_vec(&command)?;
                    stdin.write_all(&(bytes.len() as u32).to_le_bytes())?;
                    stdin.write_all(&bytes)?;
                    stdin.flush()
                })();
                started.store(0, Ordering::Release);
                if result.is_err() {
                    break;
                }
            }
            writing.store(false, Ordering::Release);
        })?;
    // Monitor independently of blocking pipe writes. A wedged browser can fill
    // stdin, but cannot hold this worker, the render loop, or shutdown forever.
    let result = (|| loop {
        if child.try_wait()?.is_some() || !reader_alive.load(Ordering::Acquire) {
            return Err(std::io::Error::other("browser host stopped"));
        }
        if !writer_alive.load(Ordering::Acquire) {
            return Ok(());
        }
        let began = write_started.load(Ordering::Acquire);
        if began != 0 && clock.elapsed().as_millis() as u64 + 1 > began + 5000 {
            return Err(std::io::Error::other("browser command pipe stalled"));
        }
        thread::sleep(Duration::from_millis(50));
    })();
    // On normal teardown stdin is closed by the writer. Allow a short graceful
    // close, then close the job to terminate every remaining Chromium process.
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while child.try_wait()?.is_none() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    drop(job);
    let _ = child.wait();
    // No worker is joined by the render thread. Closing the process tree breaks
    // pending pipe I/O. The command receiver may still be owned by HtmlManager;
    // leave that writer detached until the manager drops/restarts its sender.
    drop(writer);
    let _ = reader.join();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_windows_transport_renders_and_closes_its_process_tree() {
        // The Windows browser workflow supplies the freshly built runtime.
        // Ordinary unit runs remain independent of an installed CEF SDK.
        if std::env::var_os("NIDA_BROWSER_TEST_HELPER").is_none() {
            return;
        }
        let engine = CefEngine::new();
        let alive = engine.alive.clone();
        engine
            .configure(
                1,
                3,
                &super::super::BrowserSettings {
                    url: "data:text/html,<html style='background:red'><body></body></html>".into(),
                    width: 64,
                    height: 32,
                    fps: 30,
                },
            )
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let mut rendered = false;
        while std::time::Instant::now() < deadline && engine.is_alive() {
            for message in engine.drain() {
                if message.id == 1
                    && message.generation == 3
                    && message.width == 64
                    && message.height == 32
                {
                    let center = ((16 * 64 + 32) * 4) as usize;
                    rendered |= message.pixels.get(center..center + 4) == Some(&[255, 0, 0, 255]);
                }
            }
            if rendered {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        drop(engine);
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while alive.load(Ordering::Acquire) && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !alive.load(Ordering::Acquire),
            "browser process tree did not close"
        );
        assert!(
            rendered,
            "no real CEF frame arrived through the Windows job and Rust transport"
        );
        let profile =
            std::env::temp_dir().join(format!("Nida CEF transport test {}", std::process::id()));
        std::fs::remove_dir_all(profile).expect("CEF job should release its profile files");
    }
}
