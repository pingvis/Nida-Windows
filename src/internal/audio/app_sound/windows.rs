//! WASAPI process loopback. The existing app-sound worker owns discovery;
//! each selected app has a bounded, stoppable capture thread of its own.
use anyhow::{bail, Context as _, Result};
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};
use windows::core::{Interface, PWSTR};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, HWND, LPARAM},
    Media::Audio::{
        eRender, ActivateAudioInterfaceAsync, AudioSessionStateActive,
        IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
        IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient,
        IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator,
        AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
        AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_LOOPBACK,
        AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS,
        AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, DEVICE_STATE_ACTIVE,
        PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
        WAVEFORMATEX,
    },
    System::{
        Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize,
            StructuredStorage::{PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0},
            BLOB, CLSCTX_ALL, COINIT_MULTITHREADED,
        },
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        Variant::VT_BLOB,
    },
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, IsWindowVisible},
};

const RATE: u32 = 48_000;
const BLOCK: u64 = 480;
const LATENCY: u64 = 1440;
const PREFIX: &str = "windows-exe:";

struct Apartment;
impl Apartment {
    fn new() -> Result<Self> {
        // SAFETY: each discovery/capture call balances its own MTA entry.
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn process_path(pid: u32) -> Option<String> {
    // SAFETY: queried handle is owned here, buffer capacity is supplied, and
    // only the initialized path prefix is read.
    unsafe {
        let handle = Handle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?);
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &raw mut len,
        )
        .ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

type Processes = HashMap<u32, (u32, String)>;
fn process_tree() -> Processes {
    let mut tree = HashMap::new();
    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return tree;
        };
        let snapshot = Handle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot.0, &raw mut entry).is_ok() {
            loop {
                tree.insert(
                    entry.th32ProcessID,
                    (entry.th32ParentProcessID, String::new()),
                );
                if Process32NextW(snapshot.0, &raw mut entry).is_err() {
                    break;
                }
            }
        }
    }
    tree
}

// Resolve only audio processes and their ancestors, not every executable on
// the PC on every discovery poll.
fn resolve_paths(mut pid: u32, tree: &mut Processes) {
    let mut seen = HashSet::new();
    while seen.insert(pid) {
        let Some((parent, path)) = tree.get_mut(&pid) else {
            break;
        };
        if path.is_empty() {
            *path = process_path(pid).unwrap_or_default();
        }
        pid = *parent;
    }
}

unsafe extern "system" fn visible_process(hwnd: HWND, data: LPARAM) -> windows::core::BOOL {
    // SAFETY: EnumWindows is synchronous; data points at its caller's set.
    unsafe {
        if IsWindowVisible(hwnd).as_bool() {
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
            (&mut *(data.0 as *mut HashSet<u32>)).insert(pid);
        }
    }
    true.into()
}

fn descendant(pid: u32, ancestor: u32, tree: &Processes) -> bool {
    let mut pid = pid;
    let mut seen = HashSet::new();
    while seen.insert(pid) {
        if pid == ancestor {
            return true;
        }
        let Some((parent, _)) = tree.get(&pid) else {
            break;
        };
        pid = *parent;
    }
    false
}

fn root_of(pid: u32, tree: &Processes, visible: &HashSet<u32>) -> u32 {
    let mut root = pid;
    let mut seen = HashSet::new();
    while seen.insert(root) {
        let Some((parent, path)) = tree.get(&root) else {
            break;
        };
        let Some((_, parent_path)) = tree.get(parent) else {
            break;
        };
        if path.eq_ignore_ascii_case(parent_path) {
            root = *parent;
            continue;
        }
        let helper = std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        if !visible.contains(&root)
            && (helper.contains("helper")
                || helper.contains("subprocess")
                || helper == "msedgewebview2.exe")
        {
            // WebView/CEF audio helpers belong to the nearest app window,
            // rather than becoming a separate audio input themselves.
            let mut candidate = *parent;
            let mut ancestors = HashSet::new();
            while ancestors.insert(candidate) {
                let Some((next, _)) = tree.get(&candidate) else {
                    break;
                };
                let ancestor = std::path::Path::new(&tree[&candidate].1)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                if matches!(
                    ancestor.as_str(),
                    "explorer.exe"
                        | "powershell.exe"
                        | "pwsh.exe"
                        | "cmd.exe"
                        | "windowsterminal.exe"
                        | "conhost.exe"
                        | "openconsole.exe"
                        | "svchost.exe"
                        | "services.exe"
                ) {
                    break;
                }
                if visible.contains(&candidate) {
                    return candidate;
                }
                candidate = *next;
            }
        }
        break;
    }
    root
}

pub fn processes() -> Vec<(u32, String, bool)> {
    let Ok(_apartment) = Apartment::new() else {
        return vec![];
    };
    let mut tree = process_tree();
    let mut visible = HashSet::<u32>::new();
    unsafe {
        let _ = EnumWindows(Some(visible_process), LPARAM((&raw mut visible) as isize));
    }
    let mut found: HashMap<u32, (String, bool)> = HashMap::new();
    // SAFETY: COM objects stay on this MTA thread. Every index is bounded by
    // the collection's own count; failed/disappearing sessions are skipped.
    unsafe {
        let Ok(enumerator) =
            CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
        else {
            return vec![];
        };
        let Ok(devices) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
            return vec![];
        };
        for device_index in 0..devices.GetCount().unwrap_or(0) {
            let sessions = devices
                .Item(device_index)
                .and_then(|device| device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None))
                .and_then(|manager| manager.GetSessionEnumerator());
            let Ok(sessions) = sessions else {
                continue;
            };
            for session_index in 0..sessions.GetCount().unwrap_or(0) {
                let Ok(session) = sessions.GetSession(session_index) else {
                    continue;
                };
                let Ok(control) = session.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(pid) = control.GetProcessId() else {
                    continue;
                };
                if pid == 0 || descendant(pid, std::process::id(), &tree) {
                    continue;
                }
                resolve_paths(pid, &mut tree);
                let root = root_of(pid, &tree, &visible);
                let Some((_, path)) = tree.get(&root) else {
                    continue;
                };
                let playing = session
                    .GetState()
                    .is_ok_and(|s| s == AudioSessionStateActive);
                found
                    .entry(root)
                    .and_modify(|(_, any)| *any |= playing)
                    .or_insert((format!("{PREFIX}{}", path.to_lowercase()), playing));
            }
        }
    }
    let mut result: Vec<_> = found
        .into_iter()
        .map(|(pid, (app, playing))| (pid, app, playing))
        .collect();
    result.sort_by_key(|p| p.0);
    result
}

pub fn app_name(id: &str) -> String {
    std::path::Path::new(id.strip_prefix(PREFIX).unwrap_or(id))
        .file_stem()
        .map_or_else(|| id.to_owned(), |name| name.to_string_lossy().into_owned())
}

// windows-rs generates these COM implementation details; handwritten code
// outside this small module follows the repository's normal lint policy.
#[allow(clippy::inline_always, clippy::ref_as_ptr)]
mod completion {
    use super::{
        mpsc, IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
        IActivateAudioInterfaceCompletionHandler_Impl,
    };
    use windows::core::implement;
    #[implement(IActivateAudioInterfaceCompletionHandler)]
    pub(super) struct Completion {
        pub(super) done: mpsc::Sender<()>,
    }
    impl IActivateAudioInterfaceCompletionHandler_Impl for Completion_Impl {
        fn ActivateCompleted(
            &self,
            _: windows::core::Ref<IActivateAudioInterfaceAsyncOperation>,
        ) -> windows::core::Result<()> {
            let _ = self.done.send(());
            Ok(())
        }
    }
}
use completion::Completion;

struct Client {
    audio: IAudioClient,
    capture: IAudioCaptureClient,
    next: Option<u64>,
}
impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            let _ = self.audio.Stop();
        }
    }
}
fn client(pid: u32, stop: &AtomicBool) -> Result<Client> {
    let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    let variant = std::mem::ManuallyDrop::new(PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_BLOB,
                Anonymous: PROPVARIANT_0_0_0 {
                    blob: BLOB {
                        cbSize: std::mem::size_of_val(&params) as u32,
                        pBlobData: (&raw mut params).cast(),
                    },
                },
                ..Default::default()
            }),
        },
    });
    let (done, ready) = mpsc::channel();
    let callback: IActivateAudioInterfaceCompletionHandler = Completion { done }.into();
    // SAFETY: activation parameters and callback are valid for the API call.
    // The operation retains the agile callback. The audio interfaces are
    // obtained and used on this same MTA, never sent across Rust threads.
    unsafe {
        let operation = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&raw const *variant),
            &callback,
        )
        .context("Process loopback requires Windows build 20348 or later")?;
        let started = Instant::now();
        loop {
            if stop.load(Ordering::Relaxed) {
                bail!("capture cancelled");
            }
            match ready.recv_timeout(Duration::from_millis(20)) {
                Ok(()) => break,
                Err(mpsc::RecvTimeoutError::Timeout)
                    if started.elapsed() < Duration::from_secs(5) => {}
                _ => bail!("Windows did not finish activating process audio"),
            }
        }
        let mut status = windows::core::HRESULT(0);
        let mut interface = None;
        operation.GetActivateResult(&raw mut status, &raw mut interface)?;
        status
            .ok()
            .context("Windows refused process loopback audio")?;
        let audio: IAudioClient = interface
            .context("Missing process audio interface")?
            .cast()?;
        let format = WAVEFORMATEX {
            wFormatTag: 3,
            nChannels: 2,
            nSamplesPerSec: RATE,
            nAvgBytesPerSec: RATE * 8,
            nBlockAlign: 8,
            wBitsPerSample: 32,
            cbSize: 0,
        };
        audio.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
                | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY,
            200_000,
            0,
            &raw const format,
            None,
        )?;
        let capture = audio.GetService::<IAudioCaptureClient>()?;
        Ok(Client {
            audio,
            capture,
            next: None,
        })
    }
}

/// Mix process trees against their shared QPC timestamps, rather than
/// concatenating packets (which doubles duration for two app instances).
struct Timeline {
    samples: Vec<[f32; 2]>,
    stamps: Vec<u64>,
}
impl Timeline {
    fn new() -> Self {
        Self {
            samples: vec![[0.0; 2]; 16384],
            stamps: vec![u64::MAX; 16384],
        }
    }
    fn add(&mut self, frame: u64, value: [f32; 2], next: u64) {
        if frame < next || frame - next >= self.samples.len() as u64 {
            return;
        }
        let at = frame as usize % self.samples.len();
        if self.stamps[at] != frame {
            self.samples[at] = [0.0; 2];
            self.stamps[at] = frame;
        }
        for (sum, v) in self.samples[at].iter_mut().zip(value) {
            *sum += v;
        }
    }
    fn take(&mut self, frame: u64) -> [f32; 2] {
        let at = frame as usize % self.samples.len();
        if self.stamps[at] != frame {
            return [0.0; 2];
        }
        self.stamps[at] = u64::MAX;
        self.samples[at]
    }
}

fn clock_frame(frequency: i64) -> Result<u64> {
    let mut now = 0;
    unsafe {
        QueryPerformanceCounter(&raw mut now)?;
    }
    Ok((now as u128 * u128::from(RATE) / frequency as u128) as u64)
}

fn packet(client: &mut Client, timeline: &mut Timeline, next: u64, now: u64) -> Result<bool> {
    unsafe {
        if client.capture.GetNextPacketSize()? == 0 {
            return Ok(false);
        }
        let (mut data, mut frames, mut flags, mut qpc) = (std::ptr::null_mut(), 0, 0, 0);
        client.capture.GetBuffer(
            &raw mut data,
            &raw mut frames,
            &raw mut flags,
            None,
            Some(&raw mut qpc),
        )?;
        let timestamp_valid = flags & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0 && qpc != 0;
        let mut first = if timestamp_valid {
            (u128::from(qpc) * u128::from(RATE) / 10_000_000) as u64
        } else {
            client.next.unwrap_or(now.saturating_sub(u64::from(frames)))
        };
        if let Some(end) = client.next {
            if end.abs_diff(first) <= 2
                && flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 == 0
            {
                first = end;
            }
        }
        let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
        if !silent && data.is_null() {
            client.capture.ReleaseBuffer(frames)?;
            bail!("Windows returned an empty PCM buffer");
        }
        for offset in 0..frames as usize {
            let value = if silent {
                [0.0; 2]
            } else {
                // Windows supplies interleaved stereo f32 in the requested
                // format, valid until ReleaseBuffer. Read without alignment
                // assumptions and replace non-finite samples with silence.
                std::array::from_fn(|ch| {
                    let v = std::ptr::read_unaligned(data.add((offset * 2 + ch) * 4).cast::<f32>());
                    if v.is_finite() {
                        v
                    } else {
                        0.0
                    }
                })
            };
            timeline.add(first + offset as u64, value, next);
        }
        client.next = Some(first + u64::from(frames));
        client.capture.ReleaseBuffer(frames)?;
        Ok(true)
    }
}

pub(crate) struct Capture {
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Capture {
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(crate) fn open(
    app: &str,
    pids: &[u32],
) -> Result<(Capture, crossbeam_channel::Receiver<super::super::PcmChunk>)> {
    let Some(path) = app.strip_prefix(PREFIX) else {
        bail!("Not a Windows app audio identifier");
    };
    let tree = process_tree();
    let mut roots: Vec<u32> = pids
        .iter()
        .copied()
        .filter(|pid| {
            !pids
                .iter()
                .any(|other| other != pid && descendant(*pid, *other, &tree))
        })
        .collect();
    roots.sort_unstable();
    roots.dedup();
    if roots.is_empty()
        || roots
            .iter()
            .any(|pid| process_path(*pid).is_none_or(|current| !current.eq_ignore_ascii_case(path)))
    {
        bail!("The selected app stopped or restarted");
    }
    let stop = Arc::new(AtomicBool::new(false));
    let failed = Arc::new(AtomicBool::new(false));
    let (tx, rx) = crossbeam_channel::bounded(16);
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread_stop = stop.clone();
    let thread_failed = failed.clone();
    let worker = std::thread::Builder::new()
        .name("windows-app-audio".into())
        .spawn(move || {
            let run = || -> Result<()> {
                let _apartment = Apartment::new()?;
                let mut clients: Vec<Client> = roots
                    .iter()
                    .map(|pid| client(*pid, &thread_stop))
                    .collect::<Result<_>>()?;
                let mut frequency = 0;
                unsafe {
                    QueryPerformanceFrequency(&raw mut frequency)?;
                    for client in &clients {
                        client.audio.Start()?;
                    }
                }
                let mut next = clock_frame(frequency)?;
                let mut timeline = Timeline::new();
                let _ = ready_tx.send(Ok(()));
                let mut lost = 0u64;
                while !thread_stop.load(Ordering::Relaxed) {
                    let now = clock_frame(frequency)?;
                    for client in &mut clients {
                        while packet(client, &mut timeline, next, now)? {}
                    }
                    let ready = now.saturating_sub(LATENCY);
                    if ready.saturating_sub(next) > u64::from(RATE) {
                        let skip = ready - next - BLOCK;
                        lost += skip * 2;
                        next += skip;
                    }
                    while next + BLOCK <= ready {
                        let samples = (next..next + BLOCK)
                            .flat_map(|frame| timeline.take(frame))
                            .collect();
                        let chunk = super::super::PcmChunk {
                            samples,
                            lost_before: lost,
                        };
                        match tx.try_send(chunk) {
                            Ok(()) => lost = 0,
                            Err(crossbeam_channel::TrySendError::Full(_)) => lost += BLOCK * 2,
                            Err(crossbeam_channel::TrySendError::Disconnected(_)) => return Ok(()),
                        }
                        next += BLOCK;
                    }
                    std::thread::sleep(Duration::from_millis(3));
                }
                Ok(())
            };
            if let Err(error) = run() {
                thread_failed.store(true, Ordering::Relaxed);
                let _ = ready_tx.send(Err(format!("{error:#}")));
                log::warn!("App audio stopped: {error:#}");
            }
        })?;
    let capture = Capture {
        stop,
        failed,
        worker: Some(worker),
    };
    match ready_rx.recv_timeout(Duration::from_secs(6)) {
        Ok(Ok(())) => Ok((capture, rx)),
        Ok(Err(error)) => bail!(error),
        Err(_) => bail!("Windows app audio activation timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Needs an interactive Windows 20348+ audio service; produces no playback"]
    fn native_silent_process_capture_starts_and_stops_without_playback() {
        let app = format!(
            "{PREFIX}{}",
            process_path(std::process::id()).unwrap().to_lowercase()
        );
        let (capture, rx) = open(&app, &[std::process::id()])
            .expect("activate process loopback without a render stream");
        let samples = record(&rx);
        assert!(
            samples.iter().all(|sample| sample.abs() < 1e-7),
            "a process with no render stream supplies silence"
        );
        assert!(!capture.failed());
        let stopping = Instant::now();
        drop(capture);
        assert!(stopping.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn timeline_mixes_instances_at_their_timestamps_and_fills_gaps() {
        let mut timeline = Timeline::new();
        timeline.add(12, [0.2, 0.3], 10);
        timeline.add(12, [0.4, -0.1], 10);
        timeline.add(9, [1.0, 1.0], 10);
        timeline.add(20_000, [1.0, 1.0], 10);
        assert_eq!(timeline.take(11), [0.0; 2]);
        let mixed = timeline.take(12);
        assert!((mixed[0] - 0.6).abs() < 1e-6 && (mixed[1] - 0.2).abs() < 1e-6);
        assert_eq!(timeline.take(12), [0.0; 2]);
        assert_eq!(timeline.take(20_000), [0.0; 2]);
        timeline.add(12 + 16_384, [0.1, 0.2], 12);
        assert_eq!(timeline.take(12), [0.0; 2]);
    }

    #[test]
    fn helpers_belong_to_the_app_but_hidden_apps_do_not_become_their_launcher() {
        let tree = HashMap::from([
            (1, (0, "C:\\Windows\\explorer.exe".into())),
            (2, (1, "C:\\Apps\\Teams.exe".into())),
            (3, (2, "C:\\WebView\\msedgewebview2.exe".into())),
            (4, (3, "C:\\WebView\\msedgewebview2.exe".into())),
            (5, (1, "C:\\Fixtures\\tone.exe".into())),
        ]);
        let visible = HashSet::from([1, 2]);
        assert_eq!(root_of(4, &tree, &visible), 2);
        assert_eq!(root_of(5, &tree, &visible), 5);
        assert!(descendant(4, 2, &tree));
        assert!(!descendant(5, 2, &tree));
        assert_eq!(app_name("windows-exe:c:\\apps\\teams.exe"), "teams");
    }

    fn record(rx: &crossbeam_channel::Receiver<super::super::super::PcmChunk>) -> Vec<f32> {
        let mut samples = vec![];
        let start = Instant::now();
        while samples.len() < RATE as usize * 2 {
            assert!(start.elapsed() < Duration::from_secs(8), "PCM timed out");
            let chunk = rx.recv_timeout(Duration::from_secs(2)).expect("live PCM");
            assert_eq!(chunk.lost_before, 0, "uninterrupted capture");
            samples.extend(chunk.samples);
        }
        samples
    }

    fn amplitude(samples: &[f32], hz: f64) -> f64 {
        let mut sin = 0.0;
        let mut cos = 0.0;
        for (frame, channels) in samples.chunks_exact(2).enumerate() {
            let phase = std::f64::consts::TAU * hz * frame as f64 / f64::from(RATE);
            sin += f64::from(channels[0]) * phase.sin();
            cos += f64::from(channels[0]) * phase.cos();
        }
        (sin * sin + cos * cos).sqrt() * 4.0 / samples.len() as f64
    }

    #[test]
    #[ignore = "Real audio test: run three quiet tone fixtures and set NIDA_APP_AUDIO_FIXTURE"]
    fn native_process_audio_is_isolated_mixes_instances_and_stops_promptly() {
        #[derive(serde::Deserialize)]
        struct Fixture {
            first_pid: u32,
            second_pid: u32,
            other_pid: u32,
        }
        let fixture: Fixture = serde_json::from_slice(
            &std::fs::read(std::env::var_os("NIDA_APP_AUDIO_FIXTURE").expect("fixture JSON"))
                .unwrap(),
        )
        .unwrap();
        let app = format!(
            "{PREFIX}{}",
            process_path(fixture.first_pid).unwrap().to_lowercase()
        );
        let (capture, rx) = open(&app, &[fixture.first_pid]).expect("real Windows process capture");
        let samples = record(&rx);
        let signal = amplitude(&samples, 440.0);
        let excluded = amplitude(&samples, 997.0);
        eprintln!("Single instance: 440Hz={signal:.8}, excluded 997Hz={excluded:.8}");
        assert!(
            signal > 1e-6 && signal > excluded * 30.0,
            "selected app only"
        );
        assert!(
            amplitude(&samples, 659.0) < signal / 30.0,
            "other process not concatenated"
        );
        let stopping = Instant::now();
        drop(capture);
        assert!(stopping.elapsed() < Duration::from_secs(2));
        assert!(
            open(&app, &[fixture.other_pid]).is_err(),
            "PID cannot select a different app"
        );
        let (capture, rx) = open(&app, &[fixture.first_pid, fixture.second_pid]).unwrap();
        let samples = record(&rx);
        let first = amplitude(&samples, 440.0);
        let second = amplitude(&samples, 659.0);
        let excluded = amplitude(&samples, 997.0);
        eprintln!(
            "Mixed instances: 440Hz={first:.8}, 659Hz={second:.8}, excluded 997Hz={excluded:.8}"
        );
        assert!(first > 1e-6 && second > 1e-6 && first.min(second) > excluded * 30.0);
        std::thread::sleep(Duration::from_millis(350));
        while rx.try_recv().is_ok() {}
        let mut reported_loss = false;
        for _ in 0..8 {
            if rx.recv_timeout(Duration::from_secs(1)).unwrap().lost_before > 0 {
                reported_loss = true;
                break;
            }
        }
        assert!(
            reported_loss,
            "bounded backpressure reports missing samples"
        );
        drop(capture);
    }
}
