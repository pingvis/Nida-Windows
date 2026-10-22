use super::UIRunner;
use crate::usecases::ui::preview_picker;

impl UIRunner {
    pub(super) fn refresh_colour_pick(&mut self) {
        let Some(varda) = self.varda.as_ref() else {
            return;
        };
        let context = varda.gpu_context();
        if let Some((effect, rx)) = &self.colour_pick_readback {
            match rx.try_recv() {
                Ok(rgb) => {
                    if let Some(mut pick) =
                        preview_picker::current(&self.egui_ctx).filter(|p| p.effect == *effect)
                    {
                        if let Some(rgb) = rgb {
                            pick.rgb = Some(rgb);
                        } else {
                            pick.waiting = false;
                        }
                        preview_picker::store(&self.egui_ctx, pick);
                    }
                    self.colour_pick_readback = None;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    preview_picker::cancel(&self.egui_ctx);
                    self.colour_pick_readback = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let Some(renderer) = self.egui_renderer.as_mut() else {
            return;
        };
        let preview = context.colour_pick.preview();
        let generation = preview.as_ref().map(|p| p.1);
        if self.colour_pick_texture.as_ref().map(|p| p.1) != generation {
            if let Some((_, _, texture, _)) = self.colour_pick_texture.take() {
                renderer.free_texture(&texture);
            }
            if let Some((effect, generation, view, size)) = preview {
                let texture = renderer.register_native_texture(
                    &context.device,
                    &view,
                    wgpu::FilterMode::Nearest,
                );
                self.colour_pick_texture = Some((effect, generation, texture, size));
            }
        }
    }

    pub(super) fn sync_colour_pick(&mut self) {
        let Some(varda) = self.varda.as_ref() else {
            return;
        };
        let context = varda.gpu_context();
        let pick = preview_picker::current(&self.egui_ctx);
        context
            .colour_pick
            .request(pick.as_ref().map(|p| p.effect.as_str()));
        if let Some(mut pick) = pick {
            if let Some(uv) = pick.uv.take() {
                if let Some(rx) = context.colour_pick.sample(context, &pick.effect, uv) {
                    self.colour_pick_readback = Some((pick.effect.clone(), rx));
                } else {
                    pick.waiting = false;
                }
                preview_picker::store(&self.egui_ctx, pick);
            }
        } else {
            self.colour_pick_readback = None;
        }
    }
}
