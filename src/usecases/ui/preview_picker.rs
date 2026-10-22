//! Input Settings colour picking state, local to this egui context.
use super::{UIActions, UIData};
use crate::{engine::EngineCommand, params::ParamValue};

#[derive(Clone)]
pub(crate) struct Pick {
    pub owner: String,
    pub effect: String,
    pub alpha: f32,
    pub uv: Option<[f32; 2]>,
    pub waiting: bool,
    pub rgb: Option<[f32; 3]>,
}

fn id() -> egui::Id {
    egui::Id::new("preview_colour_pick")
}
pub(crate) fn current(ctx: &egui::Context) -> Option<Pick> {
    ctx.data(|d| d.get_temp(id()))
}
pub(crate) fn store(ctx: &egui::Context, pick: Pick) {
    ctx.data_mut(|d| d.insert_temp(id(), pick));
}
pub(crate) fn cancel(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Pick>(id()));
}
pub(crate) fn validate(ctx: &egui::Context, owner: &str, effects: &[super::EffectInfo]) {
    if current(ctx)
        .is_some_and(|p| p.owner != owner || !effects.iter().any(|e| e.0 == p.effect && e.2))
    {
        cancel(ctx);
    }
}

pub(crate) fn button(ui: &mut egui::Ui, owner: &str, effect: &super::EffectInfo) {
    if !cfg!(target_os = "windows") || effect.1 != "chroma_key" {
        return;
    }
    if ui.add_enabled(effect.2 && current(ui.ctx()).is_none(), egui::Button::new("Pick colour from preview").small())
        .on_hover_text("Click the background in the picture before this key. Esc cancels. Program keeps rendering normally.")
        .clicked()
    {
        let alpha = effect.3.params.iter().find_map(|p| match (&*p.name, &p.value) {
            ("target_color", ParamValue::Color(c)) => Some(c[3]), _ => None,
        }).unwrap_or(1.0);
        store(ui.ctx(), Pick { owner: owner.to_owned(), effect: effect.0.clone(), alpha, uv: None, waiting: false, rgb: None });
    }
}

/// Consumes all preview gestures while picking, including double-click and drag.
pub(crate) fn handle(
    ui: &mut egui::Ui,
    response: &egui::Response,
    data: &UIData,
    actions: &mut UIActions,
) -> bool {
    let Some(mut pick) = current(ui.ctx()) else {
        return false;
    };
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        cancel(ui.ctx());
        return true;
    }
    if let Some(rgb) = pick.rgb {
        if rgb.iter().all(|v| v.is_finite()) {
            // ParamValue::Color is saved as sRGB and decoded by write_bytes.
            // Encode the linear GPU sample once so the keyer receives it back.
            let rgb = rgb.map(srgb_from_linear);
            actions.commands.push(EngineCommand::SetEffectParam {
                effect_uuid: pick.effect,
                name: "target_color".into(),
                value: ParamValue::Color([rgb[0], rgb[1], rgb[2], pick.alpha]),
            });
        }
        cancel(ui.ctx());
        return true;
    }
    let ready = data
        .colour_pick_preview
        .as_ref()
        .is_some_and(|p| p.0 == pick.effect);
    if ready && !pick.waiting {
        response
            .clone()
            .on_hover_cursor(egui::CursorIcon::Crosshair);
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let uv = (pos - response.rect.min) / response.rect.size();
                pick.uv = Some([uv.x, uv.y]);
                pick.waiting = true;
                store(ui.ctx(), pick);
            }
        }
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(if !ready {
            "Preparing picture before key…"
        } else if current(ui.ctx()).is_some_and(|p| p.waiting) {
            "Reading colour…"
        } else {
            "Click a colour in the picture · Esc cancels"
        });
        if ui.small_button("Cancel pick").clicked() {
            cancel(ui.ctx());
        }
    });
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(16));
    true
}

pub(crate) fn srgb_from_linear(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_pick_before_key_keeps_program_keyed_and_round_trips_midtones() {
        let gpu = crate::renderer::GpuContext::new_headless().expect("picker GPU");
        let shader = crate::isf::ISFShader::from_file(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("shaders/chroma_key.fs"),
        )
        .unwrap();
        let mut effect = crate::deck::Effect::new(&gpu, shader).unwrap();
        let texture = |format, usage| {
            gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Picker key fixture"),
                size: wgpu::Extent3d {
                    width: 2,
                    height: 2,
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
        let source = texture(
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let bytes = [64, 128, 192, 255].repeat(4);
        gpu.queue.write_texture(
            source.as_image_copy(),
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: None,
            },
            source.size(),
        );
        let program = texture(
            gpu.compositing_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let input = source.create_view(&wgpu::TextureViewDescriptor::default());
        let output = program.create_view(&wgpu::TextureViewDescriptor::default());
        let uniforms = crate::renderer::ISFUniforms {
            render_size: [2.0, 2.0],
            ..Default::default()
        };
        let linear = [64.0 / 255.0, 128.0 / 255.0, 192.0 / 255.0];
        let param_rgb = linear.map(srgb_from_linear);
        effect
            .params
            .set_color("target_color", [param_rgb[0], param_rgb[1], param_rgb[2], 0.37]);
        effect.params.set_float("tolerance", 0.001);
        effect.params.set_float("softness", 0.001);
        gpu.colour_pick.request(Some(effect.uuid()));
        let mut commands = vec![];
        effect
            .apply(&gpu, &input, &output, &uniforms, &mut commands)
            .unwrap();
        gpu.submit(commands);
        let read = gpu
            .colour_pick
            .sample(&gpu, effect.uuid(), [0.5, 0.5])
            .unwrap();
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let picked = read.recv().unwrap().unwrap();
        for (actual, expected) in picked.into_iter().zip(linear) {
            assert!((actual - expected).abs() < 1e-6);
        }
        // The real chroma shader is still active on Program while the tap
        // exposes its input. Rgba16Float alpha zero proves it stayed keyed.
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            program.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
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
        gpu.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        rx.recv().unwrap().unwrap();
        let bytes = readback.slice(..).get_mapped_range().unwrap();
        assert_eq!(
            u16::from_le_bytes([bytes[6], bytes[7]]),
            0,
            "Program remains keyed"
        );
    }
    fn pick() -> Pick {
        Pick {
            owner: "deck/a".into(),
            effect: "key".into(),
            alpha: 0.37,
            uv: None,
            waiting: false,
            rgb: None,
        }
    }

    #[test]
    fn selection_or_effect_changes_cancel_the_pick() {
        let ctx = egui::Context::default();
        let effect = (
            "key".into(),
            "chroma_key".into(),
            true,
            super::super::ShaderParamsUI {
                shader_name: "chroma_key".into(),
                params: vec![],
            },
        );
        store(&ctx, pick());
        validate(&ctx, "deck/a", std::slice::from_ref(&effect));
        assert!(current(&ctx).is_some());
        validate(&ctx, "deck/b", std::slice::from_ref(&effect));
        assert!(current(&ctx).is_none());
        store(&ctx, pick());
        validate(&ctx, "deck/a", &[]);
        assert!(current(&ctx).is_none());
        store(&ctx, pick());
        let mut disabled = effect;
        disabled.2 = false;
        validate(&ctx, "deck/a", &[disabled]);
        assert!(current(&ctx).is_none());
    }

    #[test]
    fn completed_sample_preserves_alpha_and_emits_one_normal_edit() {
        let ctx = egui::Context::default();
        let mut request = pick();
        request.rgb = Some([0.12, 0.34, 0.56]);
        store(&ctx, request);
        let mut actions = UIActions::new();
        let data = UIData::test_fixture();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let (_, response) =
                ui.allocate_exact_size(egui::vec2(100.0, 100.0), egui::Sense::click_and_drag());
            assert!(handle(ui, &response, &data, &mut actions));
            assert!(!handle(ui, &response, &data, &mut actions));
        });
        output.textures_delta.clear();
        assert_eq!(actions.commands.len(), 1);
        let EngineCommand::SetEffectParam {
            effect_uuid,
            name,
            value,
        } = &actions.commands[0]
        else {
            panic!("normal effect edit");
        };
        assert_eq!(
            (effect_uuid.as_str(), name.as_str()),
            ("key", "target_color")
        );
        let mut bytes = vec![];
        value.write_bytes(&mut bytes);
        for (actual, expected) in bytes.chunks_exact(4).zip([0.12, 0.34, 0.56, 0.37]) {
            let actual = f32::from_le_bytes(actual.try_into().unwrap());
            assert!(
                (actual - expected).abs() < 1e-6,
                "shader receives {expected}, got {actual}"
            );
        }
    }
}
