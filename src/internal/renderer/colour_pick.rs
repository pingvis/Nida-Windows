//! A temporary tap immediately before an effect. Picking never bypasses the
//! effect on Program. The UI reads one linear-light pixel asynchronously.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use super::GpuContext;

#[derive(Clone, Default)]
pub(crate) struct ColourPickCapture(Arc<Shared>);

#[derive(Default)]
struct Shared {
    active: AtomicBool,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    effect: Option<String>,
    generation: u64,
    tap: Option<Tap>,
}

struct Tap {
    size: [u32; 2],
    linear: wgpu::Texture,
    display: wgpu::Texture,
    pipeline: wgpu::ComputePipeline,
}

const SHADER: &str = r"
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var linear: texture_storage_2d<rgba32float, write>;
@group(0) @binding(2) var display: texture_storage_2d<rgba8unorm, write>;
@compute @workgroup_size(8, 8)
fn capture(@builtin(global_invocation_id) id: vec3<u32>) {
    let size = textureDimensions(linear);
    if (any(id.xy >= size)) { return; }
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(size);
    let pos = min(vec2<u32>(uv * vec2<f32>(textureDimensions(source))), textureDimensions(source) - 1u);
    let rgb = textureLoad(source, vec2<i32>(pos), 0).rgb;
    textureStore(linear, vec2<i32>(id.xy), vec4<f32>(rgb, 1.0));
    let c = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    let gamma = select(1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3<f32>(0.0031308));
    textureStore(display, vec2<i32>(id.xy), vec4<f32>(gamma, 1.0));
}
";

impl ColourPickCapture {
    /// Changing or clearing the request invalidates outstanding readbacks.
    pub(crate) fn request(&self, effect: Option<&str>) {
        let mut state = self.0.state.lock().unwrap();
        if state.effect.as_deref() != effect {
            state.effect = effect.map(str::to_owned);
            state.generation = state.generation.wrapping_add(1);
            state.tap = None;
            self.0.active.store(effect.is_some(), Ordering::Release);
        }
    }

    pub(crate) fn preview(&self) -> Option<(String, u64, wgpu::TextureView, [u32; 2])> {
        let state = self.0.state.lock().unwrap();
        let tap = state.tap.as_ref()?;
        Some((
            state.effect.clone()?,
            state.generation,
            tap.display
                .create_view(&wgpu::TextureViewDescriptor::default()),
            tap.size,
        ))
    }

    pub(crate) fn capture(
        &self,
        context: &GpuContext,
        effect: &str,
        source: &wgpu::TextureView,
        size: [f32; 2],
        commands: &mut Vec<wgpu::CommandBuffer>,
    ) {
        if !self.0.active.load(Ordering::Acquire) {
            return;
        }
        let mut state = self.0.state.lock().unwrap();
        if state.effect.as_deref() != Some(effect) {
            return;
        }
        let scale = (960.0 / size[0].max(size[1]).max(1.0)).min(1.0);
        let size = size.map(|v| (v * scale).round().max(1.0) as u32);
        if state.tap.as_ref().is_none_or(|tap| tap.size != size) {
            state.generation = state.generation.wrapping_add(1);
            state.tap = Some(Tap::new(&context.device, size));
        }
        let tap = state.tap.as_ref().unwrap();
        let linear = tap
            .linear
            .create_view(&wgpu::TextureViewDescriptor::default());
        let display = tap
            .display
            .create_view(&wgpu::TextureViewDescriptor::default());
        let group = context
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Colour pick tap"),
                layout: &tap.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(source),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&linear),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&display),
                    },
                ],
            });
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&tap.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(size[0].div_ceil(8), size[1].div_ceil(8), 1);
        }
        commands.push(encoder.finish());
    }

    /// No GPU wait on the UI thread. The receiver returns None on failure or
    /// when the operator changed selection while the pixel was in flight.
    pub(crate) fn sample(
        &self,
        context: &GpuContext,
        effect: &str,
        uv: [f32; 2],
    ) -> Option<std::sync::mpsc::Receiver<Option<[f32; 3]>>> {
        let state = self.0.state.lock().unwrap();
        if state.effect.as_deref() != Some(effect) || !uv.iter().all(|v| v.is_finite()) {
            return None;
        }
        let tap = state.tap.as_ref()?;
        let generation = state.generation;
        let pixel = pixel_at(uv, tap.size);
        let buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Colour pick pixel"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tap.linear,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: pixel[0],
                    y: pixel[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        drop(state);
        context.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        let capture = self.clone();
        let mapped = buffer.clone();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let valid = capture.0.state.lock().unwrap().generation == generation;
                let rgb = if result.is_ok() && valid {
                    let bytes = mapped.slice(..).get_mapped_range().expect("mapped pixel");
                    let rgb = std::array::from_fn(|i| {
                        f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
                    });
                    drop(bytes);
                    mapped.unmap();
                    Some(rgb)
                } else {
                    None
                };
                let _ = tx.send(rgb);
            });
        Some(rx)
    }
}

fn pixel_at(uv: [f32; 2], size: [u32; 2]) -> [u32; 2] {
    std::array::from_fn(|i| ((uv[i].clamp(0.0, 1.0) * size[i] as f32) as u32).min(size[i] - 1))
}

impl Tap {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let texture = |format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Colour pick tap"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let linear = texture(
            wgpu::TextureFormat::Rgba32Float,
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        );
        let display = texture(
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Colour pick"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Colour pick"),
            layout: None,
            module: &module,
            entry_point: Some("capture"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            size,
            linear,
            display,
            pipeline,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_pick_keeps_linear_rgb_and_invalidates_cancelled_readbacks() {
        let gpu = GpuContext::new_headless().expect("GPU for picker test");
        let capture = &gpu.colour_pick;
        let source = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Picker fixture"),
            size: wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Transparent inputs must still expose their RGB for picking.
        gpu.queue.write_texture(
            source.as_image_copy(),
            &[
                0, 255, 0, 0, 255, 0, 0, 255, 64, 128, 192, 0, 0, 0, 255, 255,
            ],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: None,
            },
            source.size(),
        );
        let view = source.create_view(&wgpu::TextureViewDescriptor::default());
        capture.request(Some("key"));
        let mut commands = vec![];
        capture.capture(&gpu, "other", &view, [2.0, 2.0], &mut commands);
        assert!(commands.is_empty());
        capture.capture(&gpu, "key", &view, [2.0, 2.0], &mut commands);
        gpu.submit(commands);
        let read = capture.sample(&gpu, "key", [0.1, 0.9]).unwrap();
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let rgb = read.recv().unwrap().unwrap();
        for (actual, byte) in rgb.into_iter().zip([64.0, 128.0, 192.0]) {
            assert!((actual - byte / 255.0).abs() < 1e-6);
        }
        let read = capture.sample(&gpu, "key", [0.0, 0.0]).unwrap();
        capture.request(None);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        assert_eq!(read.recv().unwrap(), None);
        assert!(capture.preview().is_none());
        assert!(capture.sample(&gpu, "key", [0.0, 0.0]).is_none());
    }
    #[test]
    fn pixels_include_edges_and_keep_y_down() {
        assert_eq!(pixel_at([0.0, 0.0], [960, 540]), [0, 0]);
        assert_eq!(pixel_at([1.0, 1.0], [960, 540]), [959, 539]);
        assert_eq!(pixel_at([0.25, 0.75], [960, 540]), [240, 405]);
    }
}
