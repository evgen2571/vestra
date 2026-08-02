//! Fixed reusable working textures for the WGPU frame graph.
//!
//! All working textures use `Rgba8Unorm`. Values are encoded straight-alpha
//! channel values, not linear-light values. Compute shaders clamp before each
//! write, which matches the CPU renderer's byte-oriented semantics.

use crate::plan::RenderPlan;

use super::frame_plan::{TextureSlot, plan_requires_auxiliary};

pub(super) const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub(super) const WORKING_TEXTURE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::TEXTURE_BINDING
    .union(wgpu::TextureUsages::STORAGE_BINDING)
    .union(wgpu::TextureUsages::COPY_SRC)
    .union(wgpu::TextureUsages::COPY_DST);

#[derive(Clone, Copy, Debug)]
pub(super) struct WorkingTextureDescriptor {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) format: wgpu::TextureFormat,
    pub(super) usage: wgpu::TextureUsages,
}

pub(super) struct WorkingTexture {
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) _descriptor: WorkingTextureDescriptor,
    pub(super) estimated_bytes: u64,
}

/// Immutable completed layer-local output retained for Phase 10B reuse.
/// It is sampled by composition and never used as a render target after
/// publication.
pub(super) struct StaticLayerTexture {
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) estimated_bytes: u64,
}

pub(super) fn static_layer_texture_bytes(width: u32, height: u32) -> u64 {
    u64::from(width) * u64::from(height) * 4
}

pub(super) fn create_static_layer_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> StaticLayerTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("video-editor cached static layer"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: WORKING_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    StaticLayerTexture {
        texture,
        view,
        estimated_bytes: static_layer_texture_bytes(width, height),
    }
}

/// Effect textures are fixed reusable full-frame slots. They are created with
/// the canvas resources during preparation, never while encoding a frame.
pub(super) struct TexturePool {
    canvas_a: WorkingTexture,
    canvas_b: WorkingTexture,
    layer: WorkingTexture,
    effect_a: Option<WorkingTexture>,
    effect_b: Option<WorkingTexture>,
    auxiliary: Option<WorkingTexture>,
}

impl TexturePool {
    pub(super) fn create(device: &wgpu::Device, plan: &RenderPlan) -> Self {
        let descriptor = WorkingTextureDescriptor {
            width: plan.canvas.width,
            height: plan.canvas.height,
            format: WORKING_FORMAT,
            usage: WORKING_TEXTURE_USAGE,
        };
        let effect_pass_count = plan.compilation.effect_pass_count;
        Self {
            canvas_a: create_texture(device, descriptor, "video-editor canvas A"),
            canvas_b: create_texture(device, descriptor, "video-editor canvas B"),
            layer: create_texture(device, descriptor, "video-editor layer"),
            effect_a: (effect_pass_count > 0)
                .then(|| create_texture(device, descriptor, "video-editor effect A")),
            effect_b: (effect_pass_count > 1)
                .then(|| create_texture(device, descriptor, "video-editor effect B")),
            auxiliary: plan_requires_auxiliary(plan).then(|| {
                create_texture(device, descriptor, "video-editor retained effect original")
            }),
        }
    }

    pub(super) fn get(&self, slot: TextureSlot) -> &WorkingTexture {
        match slot {
            TextureSlot::CanvasA => &self.canvas_a,
            TextureSlot::CanvasB => &self.canvas_b,
            TextureSlot::Layer => &self.layer,
            TextureSlot::EffectA => self
                .effect_a
                .as_ref()
                .expect("effect plan requires prepared Effect A"),
            TextureSlot::EffectB => self
                .effect_b
                .as_ref()
                .expect("effect plan requires prepared Effect B"),
            TextureSlot::Auxiliary => self
                .auxiliary
                .as_ref()
                .expect("effect plan requires prepared Auxiliary texture"),
        }
    }

    pub(super) fn estimated_bytes(&self) -> u64 {
        self.canvas_a.estimated_bytes
            + self.canvas_b.estimated_bytes
            + self.layer.estimated_bytes
            + self
                .effect_a
                .as_ref()
                .map_or(0, |texture| texture.estimated_bytes)
            + self
                .effect_b
                .as_ref()
                .map_or(0, |texture| texture.estimated_bytes)
            + self
                .auxiliary
                .as_ref()
                .map_or(0, |texture| texture.estimated_bytes)
    }

    pub(super) fn has_effects(&self) -> bool {
        self.effect_a.is_some()
    }

    pub(super) fn has_effect_b(&self) -> bool {
        self.effect_b.is_some()
    }

    pub(super) fn has_auxiliary(&self) -> bool {
        self.auxiliary.is_some()
    }

    pub(super) fn texture_count(&self) -> usize {
        3 + usize::from(self.effect_a.is_some())
            + usize::from(self.effect_b.is_some())
            + usize::from(self.auxiliary.is_some())
    }
}

fn create_texture(
    device: &wgpu::Device,
    descriptor: WorkingTextureDescriptor,
    label: &str,
) -> WorkingTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: descriptor.width,
            height: descriptor.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: descriptor.format,
        usage: descriptor.usage,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    WorkingTexture {
        texture,
        view,
        _descriptor: descriptor,
        estimated_bytes: u64::from(descriptor.width) * u64::from(descriptor.height) * 4,
    }
}

#[cfg(test)]
mod tests {
    use super::WORKING_TEXTURE_USAGE;

    #[test]
    fn retained_originals_require_copy_destination_usage() {
        assert!(WORKING_TEXTURE_USAGE.contains(wgpu::TextureUsages::COPY_DST));
        assert!(WORKING_TEXTURE_USAGE.contains(wgpu::TextureUsages::COPY_SRC));
        assert!(WORKING_TEXTURE_USAGE.contains(wgpu::TextureUsages::TEXTURE_BINDING));
        assert!(WORKING_TEXTURE_USAGE.contains(wgpu::TextureUsages::STORAGE_BINDING));
    }

    #[test]
    fn effect_pipeline_working_texture_memory_uses_five_full_frame_slots() {
        let bytes = u64::from(1920_u32) * 1080 * 4;
        assert_eq!(bytes * 5, 41_472_000);
    }
}
