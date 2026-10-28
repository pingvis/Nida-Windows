//! HTML deck source — offscreen web rendering via Servo → wgpu texture.
//!
//! See `/spec/html-source.md` for the full design.
//!
//! The integration surface (`HtmlManager` and its public API) always compiles
//! so the deck/persistence/UI/API plumbing is testable without pulling in the
//! heavy Servo dependency. The Servo-backed rendering is gated behind the
//! `html` cargo feature (`servo_backend`); when the feature is disabled the
//! manager still allocates a texture per instance but produces a blank frame.

#[cfg(feature = "html")]
mod servo_backend;

use std::sync::{Arc, Mutex};

#[cfg(all(target_os = "windows", feature = "windows-browser"))]
mod cef_backend;
#[cfg(all(target_os = "windows", feature = "windows-browser"))]
mod cef_input;
#[cfg(all(target_os = "windows", feature = "windows-browser"))]
mod cef_job;
#[cfg(any(test, all(target_os = "windows", feature = "windows-browser")))]
use nida_browser_protocol as protocol;

mod settings;
pub use settings::{browser_source_name, BrowserSettings, BrowserSnapshot};

/// Default render resolution for an HTML instance when none is supplied.
const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;

/// Stable opaque identifier for an HTML render instance. Addresses the owning
/// servo thread's `WebViews` independently of render-side `Vec` ordering.
type HtmlId = u64;

/// A finished RGBA frame (`width*height*4`) published from the servo thread.
struct HtmlFrame {
    data: Vec<u8>,
    width: u32,
    height: u32,
    generation: u64,
}

/// Latest-frame slot shared between the servo thread (writer) and the render
/// thread (reader), mirroring the NDI/stream `Arc<Mutex<Option<…>>>` pattern.
type FrameSlot = Arc<Mutex<Option<HtmlFrame>>>;

/// A single interactive-mode input event forwarded from an interactive window to
/// the owning servo thread (feature `html`). Carries only `Send` data plus
/// `keyboard_types` values; the winit→here translation lives in the app layer so
/// winit never reaches this module, preserving the "only `Send` data crosses the
/// thread boundary" contract. Positions are `WebView` **device** pixels. See
/// `/spec/html-source.md` §4.
#[cfg(any(feature = "html", all(target_os = "windows", feature = "windows-browser")))]
#[derive(Debug, Clone)]
pub enum HtmlInputEvent {
    /// Pointer moved to a `WebView` device-pixel position.
    MouseMove { x: f32, y: f32 },
    /// Mouse button pressed/released. `button`: 0=left, 1=middle, 2=right.
    MouseButton {
        x: f32,
        y: f32,
        button: u16,
        pressed: bool,
    },
    /// Wheel delta (DOM `wheel` event) in device pixels.
    Wheel { x: f32, y: f32, dx: f64, dy: f64 },
    /// Scroll the page by a device-pixel delta at a position.
    Scroll { x: f32, y: f32, dx: f64, dy: f64 },
    /// A translated keyboard event.
    Key(keyboard_types::KeyboardEvent),
    /// An IME composition update (preedit start/update or commit end).
    Ime(keyboard_types::CompositionEvent),
    /// IME composition dismissed/cancelled.
    ImeDismissed,
    /// Window focus gained/lost → `WebView::focus()`/`blur()`.
    Focus(bool),
    /// Modifier state, including when no character key is pressed.
    Modifiers(keyboard_types::Modifiers),
}

/// Manages offscreen HTML render instances and their destination GPU textures.
///
/// Mirrors the shape of `NdiManager` / `StreamManager`: the render loop calls
/// [`HtmlManager::update`] each frame and looks up [`HtmlManager::texture_view`]
/// for compositing.
pub struct HtmlManager {
    instances: Vec<Option<HtmlInstance>>,
    disabled: bool,
    next_id: HtmlId,
    /// The single shared servo pump thread, spawned lazily on first render.
    #[cfg(feature = "html")]
    engine: Option<servo_backend::ServoEngine>,
    #[cfg(all(target_os = "windows", feature = "windows-browser"))]
    cef: Option<cef_backend::CefEngine>,
}

/// A single HTML render target: the wgpu texture frames are uploaded into, plus
/// the shared slot the servo thread publishes finished frames to.
struct HtmlInstance {
    url: String,
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// True once at least one frame (or the placeholder) has been written.
    initialized: bool,
    painted: bool,
    /// Stable id addressing this instance on the servo thread.
    #[cfg_attr(not(feature = "html"), allow(dead_code))]
    id: HtmlId,
    /// Latest frame published by the servo thread (never filled when the `html`
    /// feature is off → placeholder is shown).
    frame: FrameSlot,
    settings: BrowserSettings,
    generation: u64,
    error: Option<String>,
}

impl Default for HtmlManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HtmlManager {
    /// Create an active manager. Servo itself is initialized lazily per instance.
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            disabled: false,
            next_id: 0,
            #[cfg(feature = "html")]
            engine: None,
            #[cfg(all(target_os = "windows", feature = "windows-browser"))]
            cef: None,
        }
    }

    /// Create a no-op manager (for the `--no-html` CLI flag). `start_render`
    /// always returns `None`.
    pub fn new_disabled() -> Self {
        log::info!("HTML source manager disabled");
        Self {
            instances: Vec::new(),
            disabled: true,
            next_id: 0,
            #[cfg(feature = "html")]
            engine: None,
            #[cfg(all(target_os = "windows", feature = "windows-browser"))]
            cef: None,
        }
    }

    /// Whether HTML rendering is available (feature enabled and not disabled).
    pub fn is_available(&self) -> bool {
        if self.disabled {
            return false;
        }
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        {
            cef_backend::helper_path().is_some()
        }
        #[cfg(not(all(target_os = "windows", feature = "windows-browser")))]
        {
            cfg!(feature = "html")
        }
    }

    /// Allocate an independent browser source. An unavailable backend retains a
    /// transparent placeholder so loading a show never drops its authored deck.
    pub fn start_render(
        &mut self,
        url: &str,
        width: u32,
        height: u32,
        device: &wgpu::Device,
    ) -> Option<usize> {
        let width = if width == 0 { DEFAULT_WIDTH } else { width };
        let height = if height == 0 { DEFAULT_HEIGHT } else { height };
        let settings = BrowserSettings {
            url: url.to_string(),
            width,
            height,
            ..BrowserSettings::default()
        };
        self.start_with_settings(settings, device)
    }

    /// Allocate a source with its complete authored settings.
    pub fn start_with_settings(
        &mut self,
        settings: BrowserSettings,
        device: &wgpu::Device,
    ) -> Option<usize> {
        let (url, width, height) = (settings.url.as_str(), settings.width, settings.height);
        settings.validate().ok()?;
        if width > device.limits().max_texture_dimension_2d
            || height > device.limits().max_texture_dimension_2d
        {
            return None;
        }

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("browser-source"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            // COPY_SRC lets the rendered frame be read back to CPU (deck smoke
            // tests; future thumbnail/snapshot of HTML decks).
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let id = self.next_id;
        self.next_id += 1;
        let frame: FrameSlot = Arc::new(Mutex::new(None));

        // Spawn the shared servo thread on first use, then start this instance.
        #[cfg(feature = "html")]
        {
            if self.is_available() && !cfg!(all(target_os = "windows", feature = "windows-browser"))
            {
                let engine = self
                    .engine
                    .get_or_insert_with(servo_backend::ServoEngine::new);
                engine.start(id, url, width, height, frame.clone());
            }
        }

        #[allow(unused_mut)] // CEF adds asynchronous command submission errors on Windows.
        let mut error = (!self.is_available()).then(|| {
            "Browser rendering is unavailable in this build or the CEF runtime is missing"
                .to_string()
        });
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        if self.is_available() {
            let cef = self.cef.get_or_insert_with(cef_backend::CefEngine::new);
            error = cef.configure(id, 0, &settings).err().map(str::to_string);
        }
        self.instances.push(Some(HtmlInstance {
            url: url.to_string(),
            width,
            height,
            texture,
            view,
            initialized: false,
            painted: false,
            id,
            frame,
            settings,
            generation: 0,
            error,
        }));
        log::info!("Browser instance {id} allocated ({width}x{height})");
        Some(self.instances.len() - 1)
    }

    /// Per-frame: pump each Servo instance and upload its latest frame. When the
    /// `html` feature is off this writes a one-time placeholder so the deck is
    /// not invisible.
    pub fn update(&mut self, _device: &wgpu::Device, queue: &wgpu::Queue) {
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        if let Some(cef) = &self.cef {
            if !cef.is_alive() {
                for instance in self.instances.iter_mut().flatten() {
                    instance.error =
                        Some("Browser host stopped. Check the CEF runtime, then refresh.".into());
                }
            }
            for message in cef.drain() {
                if message.id == u64::MAX {
                    for instance in self.instances.iter_mut().flatten() {
                        instance.error.clone_from(&message.error);
                    }
                } else if let Some(instance) =
                    self.instances.iter_mut().flatten().find(|instance| {
                        instance.id == message.id && instance.generation == message.generation
                    })
                {
                    if let Some(error) = message.error {
                        instance.error = (!error.is_empty()).then_some(error);
                    } else if let Ok(mut frame) = instance.frame.try_lock() {
                        *frame = Some(HtmlFrame {
                            data: message.pixels,
                            width: message.width,
                            height: message.height,
                            generation: message.generation,
                        });
                    }
                }
            }
        }
        for instance in self.instances.iter_mut().flatten() {
            // Non-blocking poll of the latest frame published by the servo thread
            // (latest-wins, identical to the NDI/stream sources).
            let frame = instance.frame.try_lock().ok().and_then(|mut g| g.take());

            if let Some(frame) = frame {
                if frame.width == instance.width
                    && frame.height == instance.height
                    && frame.generation == instance.generation
                {
                    Self::upload(queue, instance, &frame.data);
                    instance.initialized = true;
                    instance.painted = true;
                }
            } else if !instance.initialized {
                let placeholder = placeholder_frame(instance.width, instance.height);
                Self::upload(queue, instance, &placeholder);
                instance.initialized = true;
            }
        }
    }

    /// Upload an RGBA byte buffer (`width*height*4`) into the instance texture.
    fn upload(queue: &wgpu::Queue, instance: &HtmlInstance, rgba: &[u8]) {
        let expected = (instance.width * instance.height * 4) as usize;
        if rgba.len() < expected {
            return;
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &instance.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..expected],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(instance.width * 4),
                rows_per_image: Some(instance.height),
            },
            wgpu::Extent3d {
                width: instance.width,
                height: instance.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Texture view for compositing the instance at `idx`.
    pub fn texture_view(&self, idx: usize) -> Option<&wgpu::TextureView> {
        self.instances
            .get(idx)
            .and_then(Option::as_ref)
            .map(|i| &i.view)
    }

    /// Render dimensions of the instance at `idx`.
    pub fn instance_dimensions(&self, idx: usize) -> Option<(u32, u32)> {
        self.instances
            .get(idx)
            .and_then(Option::as_ref)
            .map(|i| (i.width, i.height))
    }

    /// URL of the instance at `idx`.
    pub fn instance_url(&self, idx: usize) -> Option<&str> {
        self.instances
            .get(idx)
            .and_then(Option::as_ref)
            .map(|i| i.url.as_str())
    }

    /// Number of active instances.
    pub fn instance_count(&self) -> usize {
        self.instances.iter().flatten().count()
    }

    /// Whether the library URL currently has a live browser deck.
    pub fn has_url(&self, url: &str) -> bool {
        self.instances
            .iter()
            .flatten()
            .any(|instance| instance.url == url)
    }

    /// Release browsers whose decks were deleted or replaced by a loaded show.
    pub fn reconcile(&mut self, live: &[usize]) {
        for idx in 0..self.instances.len() {
            if self.instances[idx].is_some() && !live.contains(&idx) {
                self.stop_render(idx);
            }
        }
    }

    /// Authored settings and asynchronous host status for a browser deck.
    pub fn snapshot(&self, idx: usize) -> Option<BrowserSnapshot> {
        self.instances
            .get(idx)
            .and_then(Option::as_ref)
            .map(|instance| BrowserSnapshot {
                settings: instance.settings.clone(),
                error: instance.error.clone(),
                ready: instance.painted,
                interactive: self.is_available(),
            })
    }

    /// Update a CEF viewport without reloading its page. Servo restarts its view.
    ///
    /// # Errors
    /// Rejects invalid sizes, unsupported rates or a busy/disconnected host.
    pub fn configure(
        &mut self,
        idx: usize,
        settings: BrowserSettings,
        device: &wgpu::Device,
    ) -> Result<(), &'static str> {
        settings.validate()?;
        if settings.width > device.limits().max_texture_dimension_2d
            || settings.height > device.limits().max_texture_dimension_2d
        {
            return Err("Viewport exceeds GPU texture limits");
        }
        let instance = self
            .instances
            .get_mut(idx)
            .and_then(Option::as_mut)
            .ok_or("Browser source not found")?;
        let generation = instance.generation + 1;
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        {
            let cef = self
                .cef
                .as_ref()
                .ok_or("Browser host unavailable; reopen the show after installing its runtime")?;
            cef.configure(instance.id, generation, &settings)?;
        }
        #[cfg(feature = "html")]
        if let Some(engine) = &self.engine {
            engine.stop(instance.id);
            engine.start(
                instance.id,
                &settings.url,
                settings.width,
                settings.height,
                instance.frame.clone(),
            );
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("browser-source"),
            size: wgpu::Extent3d {
                width: settings.width,
                height: settings.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        instance.view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        instance.texture = texture;
        instance.url.clone_from(&settings.url);
        instance.width = settings.width;
        instance.height = settings.height;
        instance.settings = settings;
        instance.generation = if cfg!(all(target_os = "windows", feature = "windows-browser")) {
            generation
        } else {
            0
        };
        instance.initialized = false;
        instance.painted = false;
        instance.error = None;
        if let Ok(mut frame) = instance.frame.try_lock() {
            *frame = None;
        }
        Ok(())
    }

    /// Navigate an existing instance to a new URL.
    pub fn navigate(&mut self, idx: usize, url: &str) {
        if let Some(instance) = self.instances.get_mut(idx).and_then(Option::as_mut) {
            instance.url = url.to_string();
            instance.settings.url = url.to_string();
            instance.initialized = false;
            instance.painted = false;
            #[cfg(all(target_os = "windows", feature = "windows-browser"))]
            if let Some(cef) = &self.cef {
                instance.generation += 1;
                instance.error = cef
                    .configure(instance.id, instance.generation, &instance.settings)
                    .err()
                    .map(str::to_string);
            }
            #[cfg(feature = "html")]
            if let Some(engine) = self.engine.as_ref() {
                engine.navigate(instance.id, url);
            }
        }
    }

    /// Reload an existing instance, re-fetching its current URL.
    pub fn reload(&mut self, idx: usize) {
        // Validate before restarting CEF: an unknown source must not spawn a
        // host or contend for the existing manager's Chromium profile.
        if self.instances.get(idx).and_then(Option::as_ref).is_none() {
            return;
        }
        let available = self.is_available();
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        if self.is_available() && self.cef.as_ref().is_none_or(|cef| !cef.is_alive()) {
            self.cef = Some(cef_backend::CefEngine::new());
            if let Some(cef) = &self.cef {
                for instance in self.instances.iter_mut().flatten() {
                    instance.error = cef
                        .configure(instance.id, instance.generation, &instance.settings)
                        .err()
                        .map(str::to_string);
                }
            }
        }
        if let Some(instance) = self.instances.get_mut(idx).and_then(Option::as_mut) {
            instance.error = (!available).then(|| "Browser rendering is unavailable".to_string());
            #[cfg(all(target_os = "windows", feature = "windows-browser"))]
            if let Some(cef) = &self.cef {
                instance.error = cef.reload(instance.id).err().map(str::to_string);
            }
            #[cfg(feature = "html")]
            if let Some(engine) = self.engine.as_ref() {
                engine.reload(instance.id);
            }
        }
    }

    /// Forward an interactive-mode input event to the instance at `idx`.
    /// No-op for an unknown index or when no engine has been spawned.
    #[cfg(any(feature = "html", all(target_os = "windows", feature = "windows-browser")))]
    pub fn send_input(&self, idx: usize, event: HtmlInputEvent) {
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        {
            if let (Some(instance), Some(cef)) = (
                self.instances.get(idx).and_then(Option::as_ref), self.cef.as_ref(),
            ) {
                let _ = cef.send(cef_input::command(instance.id, instance.generation, event));
            }
        }
        #[cfg(not(all(target_os = "windows", feature = "windows-browser")))]
        if let (Some(instance), Some(engine)) = (
            self.instances.get(idx).and_then(Option::as_ref),
            self.engine.as_ref(),
        ) {
            engine.send_input(instance.id, event);
        }
    }

    /// Stop a browser without changing any other deck's instance index.
    pub fn stop_render(&mut self, idx: usize) {
        if let Some(instance) = self.instances.get_mut(idx).and_then(Option::take) {
            #[cfg(all(target_os = "windows", feature = "windows-browser"))]
            if let Some(cef) = &self.cef {
                if cef.is_alive() && cef.stop(instance.id).is_err() {
                    self.instances[idx] = Some(instance);
                    return;
                }
            }
            #[cfg(feature = "html")]
            if let Some(engine) = &self.engine {
                engine.stop(instance.id);
            }
            drop(instance);
        }
    }
}

/// A fully transparent placeholder used when no frame is available. Transparent
/// (rather than opaque dark-gray) so that on a deck flagged `transparent` the
/// pre-first-frame window shows through; an unflagged deck composites it over the
/// opaque black base clear, yielding black (see /spec/html-source.md §2).
fn placeholder_frame(width: u32, height: u32) -> Vec<u8> {
    vec![0u8; (width * height * 4) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_manager_new_no_crash() {
        let mgr = HtmlManager::new();
        assert_eq!(mgr.instance_count(), 0);
    }

    #[test]
    fn html_manager_disabled_not_available() {
        let mgr = HtmlManager::new_disabled();
        assert!(!mgr.is_available());
        assert_eq!(mgr.instance_count(), 0);
    }

    #[test]
    fn html_manager_lookup_out_of_bounds() {
        let mgr = HtmlManager::new();
        assert!(mgr.texture_view(0).is_none());
        assert!(mgr.instance_dimensions(0).is_none());
        assert!(mgr.instance_url(0).is_none());
        assert!(mgr.instance_url(999).is_none());
    }

    #[test]
    fn reload_out_of_range_is_noop() {
        // Reloading a non-existent instance must not panic (no engine spawned).
        let mut mgr = HtmlManager::new();
        mgr.reload(0);
        mgr.reload(999);
        // Removed sources leave an empty slot; these must also remain no-ops.
        mgr.instances.push(None);
        mgr.reload(0);
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        assert!(
            mgr.cef.is_none(),
            "unknown sources must not start a browser host"
        );
        let mut disabled = HtmlManager::new_disabled();
        disabled.reload(0);
        #[cfg(all(target_os = "windows", feature = "windows-browser"))]
        assert!(disabled.cef.is_none());
    }

    #[cfg(feature = "html")]
    #[test]
    fn send_input_out_of_range_is_noop() {
        // Forwarding input to a non-existent instance must not panic (no engine).
        let mgr = HtmlManager::new();
        mgr.send_input(0, HtmlInputEvent::MouseMove { x: 1.0, y: 2.0 });
        mgr.send_input(999, HtmlInputEvent::Focus(true));
        let disabled = HtmlManager::new_disabled();
        disabled.send_input(0, HtmlInputEvent::Focus(false));
    }

    #[test]
    fn placeholder_frame_is_transparent_and_sized() {
        let buf = placeholder_frame(4, 2);
        assert_eq!(buf.len(), 4 * 2 * 4);
        // Fully transparent (all channels 0) so a `transparent` deck shows through
        // before its first Servo frame; see /spec/html-source.md §2.
        assert!(buf.iter().all(|&b| b == 0));
    }
}

/// True end-to-end smoke tests for the HTML deck path (feature `html`).
///
/// These drive a real Servo instance through `HtmlManager` (the deck source),
/// render into the GPU texture, then read the pixels back to verify content.
/// They are `#[ignore]` because each starts a full Servo engine (heavy, several
/// seconds). Run explicitly with:
///   cargo test `html_deck_smoke` -- --ignored --test-threads=1
#[cfg(all(test, feature = "html"))]
mod smoke_tests {
    use super::*;
    use base64::Engine as _;
    use std::time::{Duration, Instant};

    use crate::renderer::context::GpuContext;

    const W: u32 = 320; // 320*4 = 1280 bytes/row, already 256-aligned (no padding)
    const H: u32 = 240;
    const ROW_BYTES: u32 = W * 4;

    /// Wrap an HTML document in a base64 `data:` URL (avoids percent-encoding).
    fn data_url(html: &str) -> String {
        let b64 = base64::engine::general_purpose::STANDARD.encode(html.as_bytes());
        format!("data:text/html;base64,{b64}")
    }

    /// Read the center pixel of an instance's texture back to CPU.
    fn center_pixel(gpu: &GpuContext, mgr: &HtmlManager, idx: usize) -> [u8; 4] {
        let texture = &mgr.instances[idx].as_ref().unwrap().texture;
        let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("html-smoke-readback"),
            size: u64::from(ROW_BYTES * H),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ROW_BYTES),
                    rows_per_image: Some(H),
                },
            },
            wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
        );
        gpu.submit(Some(encoder.finish()));

        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().unwrap().unwrap();

        let data = buffer
            .slice(..)
            .get_mapped_range()
            .expect("test readback buffer must be mapped");
        let off = ((H / 2) * ROW_BYTES + (W / 2) * 4) as usize;
        let px = [data[off], data[off + 1], data[off + 2], data[off + 3]];
        drop(data);
        buffer.unmap();
        px
    }

    /// True when `px` is a solid version of `want` (each channel near 0 or 255).
    fn is_color(px: [u8; 4], want: [u8; 3]) -> bool {
        px[3] > 200
            && (0..3).all(|i| {
                if want[i] >= 200 {
                    px[i] >= 200
                } else {
                    px[i] <= 60
                }
            })
    }

    /// Pump the deck each frame until its center pixel matches `want` (or timeout).
    fn pump_until(
        gpu: &GpuContext,
        mgr: &mut HtmlManager,
        idx: usize,
        want: [u8; 3],
        timeout: Duration,
    ) -> [u8; 4] {
        let start = Instant::now();
        let mut last = [0u8; 4];
        while start.elapsed() < timeout {
            mgr.update(&gpu.device, &gpu.queue);
            last = center_pixel(gpu, mgr, idx);
            if is_color(last, want) {
                return last;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        last
    }

    #[test]
    #[ignore = "heavy: starts a real Servo engine; run with --ignored --test-threads=1"]
    fn html_deck_smoke_renders_plain_and_css_js() {
        let Ok(gpu) = GpuContext::new_headless() else {
            eprintln!("skipping: no GPU adapter available");
            return;
        };
        let mut mgr = HtmlManager::new();

        // 1) Plain HTML: the body background propagates to the viewport.
        let red = data_url("<!doctype html><html><body bgcolor=\"red\"></body></html>");
        let idx = mgr
            .start_render(&red, W, H, &gpu.device)
            .expect("start_render returned None with the html feature enabled");
        let px = pump_until(&gpu, &mut mgr, idx, [255, 0, 0], Duration::from_secs(30));
        assert!(
            is_color(px, [255, 0, 0]),
            "plain HTML deck did not render red; got {px:?}"
        );

        // 2) HTML + CSS + JS: CSS paints black, then JS overrides to blue.
        //    Asserting blue proves both CSS parsing and JS execution in the deck.
        let css_js = "<!doctype html><html><head><style>html,body{height:100%;margin:0}body{background:#000}</style></head><body><script>document.body.style.background='rgb(0,0,255)';</script></body></html>";
        mgr.navigate(idx, &data_url(css_js));
        let px2 = pump_until(&gpu, &mut mgr, idx, [0, 0, 255], Duration::from_secs(30));
        assert!(
            is_color(px2, [0, 0, 255]),
            "HTML+CSS+JS deck did not render blue (CSS/JS not applied); got {px2:?}"
        );
    }

    /// Idle-repaint correctness: a `setInterval` timer mutates the DOM *after*
    /// the initial load completes — driving neither `requestAnimationFrame` nor a
    /// CSS animation, so `animating()` stays false. This is the gating caveat
    /// noted in `/spec/html-source.md`: the off-thread engine must still repaint
    /// via the `frame_ready` path (`notify_new_frame_ready` + the event-loop
    /// waker unparking the pump thread). Reaching blue proves a timer-driven
    /// update on a settled page is not dropped.
    #[test]
    #[ignore = "heavy: starts a real Servo engine; run with --ignored --test-threads=1"]
    fn html_deck_setinterval_idle_repaint() {
        let Ok(gpu) = GpuContext::new_headless() else {
            eprintln!("skipping: no GPU adapter available");
            return;
        };
        let mut mgr = HtmlManager::new();

        // Paints black at load, then a setInterval timer flips to blue on its
        // first tick (~120ms after load) — no rAF, no CSS animation/transition.
        let idle = "<!doctype html><html><head><style>html,body{height:100%;margin:0}\
body{background:#000}</style></head><body><script>var done=false;\
setInterval(function(){if(!done){document.body.style.background='rgb(0,0,255)';\
done=true;}},120);</script></body></html>";
        let idx = mgr
            .start_render(&data_url(idle), W, H, &gpu.device)
            .expect("start_render returned None with the html feature enabled");
        let px = pump_until(&gpu, &mut mgr, idx, [0, 0, 255], Duration::from_secs(30));
        assert!(
            is_color(px, [0, 0, 255]),
            "setInterval idle DOM update was not repainted; got {px:?}"
        );
    }

    /// Reload correctness: after a page has rendered, `reload()` re-fetches the
    /// current URL and re-executes its JS, repainting the same content without
    /// error. Proves the `HtmlCommand::Reload` → `WebView::reload()` path drives a
    /// fresh paint on a live, settled instance.
    #[test]
    #[ignore = "heavy: starts a real Servo engine; run with --ignored --test-threads=1"]
    fn html_deck_reload_repaints() {
        let Ok(gpu) = GpuContext::new_headless() else {
            eprintln!("skipping: no GPU adapter available");
            return;
        };
        let mut mgr = HtmlManager::new();

        // CSS paints black, JS overrides to blue — reaching blue proves JS ran.
        let css_js = "<!doctype html><html><head><style>html,body{height:100%;margin:0}body{background:#000}</style></head><body><script>document.body.style.background='rgb(0,0,255)';</script></body></html>";
        let idx = mgr
            .start_render(&data_url(css_js), W, H, &gpu.device)
            .expect("start_render returned None with the html feature enabled");
        let px = pump_until(&gpu, &mut mgr, idx, [0, 0, 255], Duration::from_secs(30));
        assert!(
            is_color(px, [0, 0, 255]),
            "deck did not render blue; got {px:?}"
        );

        // Reload re-fetches the same URL and re-runs the JS; it must paint blue again.
        mgr.reload(idx);
        let px2 = pump_until(&gpu, &mut mgr, idx, [0, 0, 255], Duration::from_secs(30));
        assert!(
            is_color(px2, [0, 0, 255]),
            "deck did not repaint blue after reload; got {px2:?}"
        );
    }

    /// Interactive input correctness: a forwarded mouse click reaches the DOM.
    /// The page paints red and registers a `click` handler that flips it blue;
    /// after `send_input` delivers `MouseMove` + button down/up at the center, the
    /// deck must repaint blue. Proves the `HtmlInputEvent` → `HtmlCommand::Input`
    /// → `WebView::notify_input_event` path drives a real DOM event + repaint.
    #[test]
    #[ignore = "heavy: starts a real Servo engine; run with --ignored --test-threads=1"]
    fn html_deck_click_input_repaints() {
        let Ok(gpu) = GpuContext::new_headless() else {
            eprintln!("skipping: no GPU adapter available");
            return;
        };
        let mut mgr = HtmlManager::new();

        // Paints red; a document-level click handler flips the background to blue.
        let page = "<!doctype html><html><head><style>html,body{height:100%;margin:0}\
body{background:red}</style></head><body><script>\
document.addEventListener('click',function(){\
document.body.style.background='rgb(0,0,255)';});</script></body></html>";
        let idx = mgr
            .start_render(&data_url(page), W, H, &gpu.device)
            .expect("start_render returned None with the html feature enabled");

        // Wait for the page's red paint (distinct from the gray placeholder) so
        // the document is loaded and interactive before we click.
        let red = pump_until(&gpu, &mut mgr, idx, [255, 0, 0], Duration::from_secs(30));
        assert!(
            is_color(red, [255, 0, 0]),
            "page did not render red; got {red:?}"
        );

        // Forward a click at the viewport center: move, press, release.
        let (cx, cy) = ((W / 2) as f32, (H / 2) as f32);
        mgr.send_input(idx, HtmlInputEvent::MouseMove { x: cx, y: cy });
        mgr.send_input(
            idx,
            HtmlInputEvent::MouseButton {
                x: cx,
                y: cy,
                button: 0,
                pressed: true,
            },
        );
        mgr.send_input(
            idx,
            HtmlInputEvent::MouseButton {
                x: cx,
                y: cy,
                button: 0,
                pressed: false,
            },
        );

        let px = pump_until(&gpu, &mut mgr, idx, [0, 0, 255], Duration::from_secs(30));
        assert!(
            is_color(px, [0, 0, 255]),
            "forwarded click did not repaint blue; got {px:?}"
        );
    }

    /// Transparency correctness (Blocker 1): with `shell_background_color_rgba`
    /// set to `[0,0,0,0]`, a page whose html/body are transparent must read back
    /// with alpha below 255 — proving the Servo viewport clears transparent rather
    /// than the default opaque white. See /spec/html-source.md §2.
    #[test]
    #[ignore = "heavy: starts a real Servo engine; run with --ignored --test-threads=1"]
    fn html_deck_transparent_background_has_alpha() {
        let Ok(gpu) = GpuContext::new_headless() else {
            eprintln!("skipping: no GPU adapter available");
            return;
        };
        let mut mgr = HtmlManager::new();

        // Explicitly transparent html/body; no opaque content anywhere.
        let page = "<!doctype html><html><head><style>html,body{height:100%;margin:0;\
background:transparent}</style></head><body></body></html>";
        let idx = mgr
            .start_render(&data_url(page), W, H, &gpu.device)
            .expect("start_render returned None with the html feature enabled");

        // Pump until the center pixel reads transparent (alpha < 255), or time out.
        let start = Instant::now();
        let mut px = [255u8; 4];
        while start.elapsed() < Duration::from_secs(30) {
            mgr.update(&gpu.device, &gpu.queue);
            px = center_pixel(&gpu, &mgr, idx);
            if px[3] < 255 && mgr.instances[idx].as_ref().unwrap().painted {
                break;
            }
            std::thread::sleep(Duration::from_millis(16));
        }
        assert!(
            px[3] < 255,
            "transparent page did not yield alpha<255 (viewport still opaque); got {px:?}"
        );
    }
}
