//! Windows display capture must not capture Nida's own windows recursively.
//! Affinity is temporary, process-local and restored when the last display
//! capture stops. Explicit window captures take precedence for their window.

/// Call after creating a top-level window, including output and interaction
/// windows. Other platforms implement their exclusion inside the capture API.
pub(crate) fn window_created() {
    #[cfg(all(target_os = "windows", feature = "screen-capture"))]
    windows::refresh();
}

/// Also catch native dialogs that are created outside winit. This costs an
/// atomic read while idle and at most five window enumerations/second live.
pub(crate) fn poll() {
    #[cfg(all(target_os = "windows", feature = "screen-capture"))]
    windows::poll();
}

#[cfg(all(target_os = "windows", feature = "screen-capture"))]
pub(crate) mod windows {
    use ::windows::Win32::Foundation::{HWND, LPARAM};
    use ::windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowDisplayAffinity, GetWindowThreadProcessId, SetWindowDisplayAffinity,
        WDA_EXCLUDEFROMCAPTURE, WINDOW_DISPLAY_AFFINITY,
    };
    use std::collections::{HashMap, HashSet};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, OnceLock,
    };
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct State {
        displays: usize,
        allowed: HashMap<usize, usize>,
        epoch: u64,
    }
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    static ORIGINAL: OnceLock<Mutex<HashMap<usize, WINDOW_DISPLAY_AFFINITY>>> = OnceLock::new();
    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    static LAST: Mutex<Option<Instant>> = Mutex::new(None);
    // Serialize native affinity changes without holding the source registry.
    static APPLY: Mutex<()> = Mutex::new(());
    fn state() -> &'static Mutex<State> {
        STATE.get_or_init(Mutex::default)
    }
    fn originals() -> &'static Mutex<HashMap<usize, WINDOW_DISPLAY_AFFINITY>> {
        ORIGINAL.get_or_init(Mutex::default)
    }

    enum Kind {
        Display,
        Window(usize),
    }
    pub(crate) struct Lease(Option<Kind>);
    impl Lease {
        pub(crate) fn display() -> Self {
            {
                let mut state = state().lock().unwrap();
                state.displays += 1;
                state.epoch = state.epoch.wrapping_add(1);
                ACTIVE.store(state.displays, Ordering::Release);
            }
            // Apply before WGC starts, so its first frame is excluded too.
            refresh();
            Self(Some(Kind::Display))
        }
        pub(crate) fn window(handle: usize) -> Self {
            if !owned(HWND(handle as *mut core::ffi::c_void)) {
                return Self(None);
            }
            {
                let mut state = state().lock().unwrap();
                *state.allowed.entry(handle).or_default() += 1;
                state.epoch = state.epoch.wrapping_add(1);
            }
            refresh();
            Self(Some(Kind::Window(handle)))
        }
    }
    impl Drop for Lease {
        fn drop(&mut self) {
            let Some(kind) = self.0.take() else {
                return;
            };
            {
                let mut state = state().lock().unwrap();
                match kind {
                    Kind::Display => state.displays = state.displays.saturating_sub(1),
                    Kind::Window(handle) => {
                        if let Some(count) = state.allowed.get_mut(&handle) {
                            *count -= 1;
                            if *count == 0 {
                                state.allowed.remove(&handle);
                            }
                        }
                    }
                }
                state.epoch = state.epoch.wrapping_add(1);
                ACTIVE.store(state.displays, Ordering::Release);
            }
            refresh();
        }
    }

    struct Pass {
        exclude: bool,
        allowed: HashSet<usize>,
        live: HashSet<usize>,
        epoch: u64,
    }
    fn snapshot() -> Pass {
        let state = state().lock().unwrap();
        Pass {
            exclude: state.displays != 0,
            allowed: state.allowed.keys().copied().collect(),
            live: HashSet::new(),
            epoch: state.epoch,
        }
    }
    fn owned(hwnd: HWND) -> bool {
        let mut pid = 0;
        // SAFETY: reads process ownership only; no window pointer is dereferenced.
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
        }
        pid == std::process::id()
    }
    fn apply(hwnd: HWND, pass: &mut Pass) {
        if !owned(hwnd) {
            return;
        }
        let handle = hwnd.0 as usize;
        pass.live.insert(handle);
        let mut current = 0u32;
        // SAFETY: only windows belonging to this process are modified. No
        // registry lock is held across a Win32 call or a possible callback.
        if unsafe { GetWindowDisplayAffinity(hwnd, &raw mut current) }.is_err() {
            return;
        }
        let current = WINDOW_DISPLAY_AFFINITY(current);
        if pass.exclude && !pass.allowed.contains(&handle) {
            originals().lock().unwrap().entry(handle).or_insert(current);
            if current != WDA_EXCLUDEFROMCAPTURE {
                if let Err(error) =
                    unsafe { SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE) }
                {
                    log::warn!("Could not exclude a Nida window from display capture: {error}");
                }
            }
        } else {
            let original = originals().lock().unwrap().remove(&handle);
            if let Some(original) = original {
                if let Err(error) = unsafe { SetWindowDisplayAffinity(hwnd, original) } {
                    log::warn!("Could not restore a Nida window's capture affinity: {error}");
                }
            }
        }
    }
    unsafe extern "system" fn visit(hwnd: HWND, data: LPARAM) -> ::windows::core::BOOL {
        // SAFETY: EnumWindows invokes this synchronously with its caller's Pass.
        apply(hwnd, unsafe { &mut *(data.0 as *mut Pass) });
        true.into()
    }
    pub(super) fn refresh() {
        let _applying = APPLY.lock().unwrap();
        // Concurrent source starts/stops cannot leave an older policy applied.
        // Bound retries; poll() catches further changes on the next UI frame.
        for _ in 0..3 {
            let mut pass = snapshot();
            unsafe {
                let _ = EnumWindows(Some(visit), LPARAM((&raw mut pass) as isize));
            }
            originals()
                .lock()
                .unwrap()
                .retain(|handle, _| pass.live.contains(handle));
            if state().lock().unwrap().epoch == pass.epoch {
                break;
            }
        }
        *LAST.lock().unwrap() = Some(Instant::now());
    }
    pub(super) fn poll() {
        if ACTIVE.load(Ordering::Acquire) == 0 {
            return;
        }
        let due = LAST
            .lock()
            .unwrap()
            .is_none_or(|last| last.elapsed() >= Duration::from_millis(200));
        if due {
            refresh();
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ::windows::core::w;
        use ::windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WDA_MONITOR, WDA_NONE, WINDOW_EX_STYLE,
            WS_OVERLAPPEDWINDOW,
        };
        struct TestWindow(HWND);
        impl TestWindow {
            fn new() -> Self {
                let hwnd = unsafe {
                    CreateWindowExW(
                        WINDOW_EX_STYLE::default(),
                        w!("STATIC"),
                        w!("Nida capture policy test"),
                        WS_OVERLAPPEDWINDOW,
                        0,
                        0,
                        64,
                        64,
                        None,
                        None,
                        None,
                        None,
                    )
                }
                .unwrap();
                super::super::window_created();
                Self(hwnd)
            }
            fn affinity(&self) -> WINDOW_DISPLAY_AFFINITY {
                let mut affinity = 0u32;
                unsafe { GetWindowDisplayAffinity(self.0, &raw mut affinity) }.unwrap();
                WINDOW_DISPLAY_AFFINITY(affinity)
            }
        }
        impl Drop for TestWindow {
            fn drop(&mut self) {
                unsafe {
                    let _ = DestroyWindow(self.0);
                }
            }
        }

        #[test]
        #[ignore = "Needs an interactive Windows 10 2004+ desktop; creates hidden test windows"]
        fn native_affinity_follows_display_leases_new_windows_and_explicit_window_capture() {
            let first = TestWindow::new();
            assert_eq!(first.affinity(), WDA_NONE);
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                tx.send(Lease::display()).unwrap();
            });
            let display_a = rx
                .recv_timeout(Duration::from_secs(2))
                .expect("capture worker can change affinity without a UI callback");
            assert_eq!(first.affinity(), WDA_EXCLUDEFROMCAPTURE);
            let display_b = Lease::display();
            drop(display_a);
            assert_eq!(first.affinity(), WDA_EXCLUDEFROMCAPTURE);
            let second = TestWindow::new();
            assert_eq!(second.affinity(), WDA_EXCLUDEFROMCAPTURE);
            let explicit = Lease::window(first.0 .0 as usize);
            assert_eq!(first.affinity(), WDA_NONE);
            assert_eq!(second.affinity(), WDA_EXCLUDEFROMCAPTURE);
            drop(explicit);
            assert_eq!(first.affinity(), WDA_EXCLUDEFROMCAPTURE);
            drop(display_b);
            assert_eq!(first.affinity(), WDA_NONE);
            assert_eq!(second.affinity(), WDA_NONE);
            unsafe { SetWindowDisplayAffinity(first.0, WDA_MONITOR) }.unwrap();
            let display = Lease::display();
            assert_eq!(first.affinity(), WDA_EXCLUDEFROMCAPTURE);
            drop(display);
            assert_eq!(
                first.affinity(),
                WDA_MONITOR,
                "restore pre-existing affinity"
            );
            unsafe { SetWindowDisplayAffinity(first.0, WDA_NONE) }.unwrap();
            assert_eq!(ACTIVE.load(Ordering::Acquire), 0);
        }
    }
}
