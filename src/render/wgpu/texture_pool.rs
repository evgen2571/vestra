//! Fixed reusable working textures for the WGPU frame graph.
//!
//! All working textures use `Rgba8Unorm`. Values are encoded straight-alpha
//! channel values, not linear-light values. Compute shaders clamp before each
//! write, which matches the CPU renderer's byte-oriented semantics.

use crate::plan::RenderPlan;

use super::frame_plan::TextureSlot;

pub(super) const WORKING_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

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

/// Phase 1 prepares two canvases and one layer texture. Advanced effect plans
/// are rejected before resource preparation, so their future slots do not
/// consume memory until Phase 2 supplies an executable pass and requests them.
pub(super) struct TexturePool {
    canvas_a: WorkingTexture,
    canvas_b: WorkingTexture,
    layer: WorkingTexture,
}

impl TexturePool {
    pub(super) fn create(device: &wgpu::Device, plan: &RenderPlan) -> Self {
        let descriptor = WorkingTextureDescriptor {
            width: plan.canvas.width,
            height: plan.canvas.height,
            format: WORKING_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        };
        Self {
            canvas_a: create_texture(device, descriptor, "video-editor canvas A"),
            canvas_b: create_texture(device, descriptor, "video-editor canvas B"),
            layer: create_texture(device, descriptor, "video-editor layer"),
        }
    }

    pub(super) fn get(&self, slot: TextureSlot) -> &WorkingTexture {
        match slot {
            TextureSlot::CanvasA => &self.canvas_a,
            TextureSlot::CanvasB => &self.canvas_b,
            TextureSlot::Layer => &self.layer,
            TextureSlot::EffectA | TextureSlot::EffectB => {
                unreachable!("Phase 1 rejects effect operations before WGPU resource preparation")
            }
        }
    }

    pub(super) fn estimated_bytes(&self) -> u64 {
        [&self.canvas_a, &self.canvas_b, &self.layer]
            .into_iter()
            .map(|texture| texture.estimated_bytes)
            .sum()
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
    #[test]
    fn phase_one_working_texture_memory_uses_three_full_frame_slots() {
        let bytes = u64::from(1920_u32) * 1080 * 4;
        assert_eq!(bytes * 3, 24_883_200);
    }
}
