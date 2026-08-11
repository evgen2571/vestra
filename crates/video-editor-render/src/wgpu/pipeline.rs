//! Prepared texture pipelines and bind-group layouts.

use super::parameters::PARAMETER_RECORD_BYTES;
use crate::{
    backend::RenderBackendKind,
    kernel::{EffectKernel, supports_kernel},
};

pub(super) struct GpuPipelines {
    pub(super) _layer_shader: wgpu::ShaderModule,
    pub(super) _composite_shader: wgpu::ShaderModule,
    pub(super) _effect_shaders: Vec<(EffectKernel, wgpu::ShaderModule)>,
    pub(super) layer: wgpu::ComputePipeline,
    pub(super) composite: wgpu::ComputePipeline,
    pub(super) effects: Vec<(EffectKernel, wgpu::ComputePipeline)>,
    pub(super) layer_bindings: wgpu::BindGroupLayout,
    pub(super) composite_bindings: wgpu::BindGroupLayout,
    pub(super) effect_bindings: wgpu::BindGroupLayout,
}

impl GpuPipelines {
    pub(super) const BASE_SHADER_MODULE_COUNT: usize = 2;
    pub(super) const BASE_PIPELINE_COUNT: usize = 2;

    pub(super) fn create(device: &wgpu::Device) -> Self {
        let layer_shader = shader(
            device,
            "video-editor layer shader",
            include_str!("../shaders/layer.wgsl"),
        );
        let composite_shader = shader(
            device,
            "video-editor composite shader",
            include_str!("../shaders/composite_normal.wgsl"),
        );
        let uniform = wgpu::BindGroupLayoutEntry {
            binding: 2,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(PARAMETER_RECORD_BYTES),
            },
            count: None,
        };
        let layer_bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video-editor layer texture bindings"),
            entries: &[sampled(0), storage_texture(1), uniform],
        });
        let composite_bindings =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("video-editor composite texture bindings"),
                entries: &[
                    sampled(0),
                    sampled(1),
                    storage_texture(2),
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        ..uniform
                    },
                ],
            });
        let layer = pipeline(
            device,
            "video-editor layer pipeline",
            &layer_shader,
            &layer_bindings,
        );
        let composite = pipeline(
            device,
            "video-editor composite pipeline",
            &composite_shader,
            &composite_bindings,
        );
        let effect_bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video-editor effect texture bindings"),
            entries: &[
                sampled(0),
                sampled(1),
                storage_texture(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    ..uniform
                },
            ],
        });
        let effect_shaders = supported_kernels()
            .map(|kernel| (kernel, shader(device, kernel.label(), kernel.source())))
            .collect::<Vec<_>>();
        let effects = effect_shaders
            .iter()
            .map(|(kernel, shader)| {
                (
                    *kernel,
                    pipeline(device, kernel.label(), shader, &effect_bindings),
                )
            })
            .collect::<Vec<_>>();
        Self {
            _layer_shader: layer_shader,
            _composite_shader: composite_shader,
            _effect_shaders: effect_shaders,
            layer,
            composite,
            effects,
            layer_bindings,
            composite_bindings,
            effect_bindings,
        }
    }

    pub(super) fn effect(&self, kernel: EffectKernel) -> Option<&wgpu::ComputePipeline> {
        self.effects
            .iter()
            .find_map(|(stored, pipeline)| (*stored == kernel).then_some(pipeline))
    }

    pub(super) fn shader_module_count(&self) -> usize {
        Self::BASE_SHADER_MODULE_COUNT + self._effect_shaders.len()
    }

    pub(super) fn pipeline_count(&self) -> usize {
        Self::BASE_PIPELINE_COUNT + self.effects.len()
    }

    #[cfg(test)]
    pub(super) fn declared_supported_kernel_count() -> usize {
        supported_kernels().count()
    }
}

fn supported_kernels() -> impl Iterator<Item = EffectKernel> {
    EffectKernel::ALL
        .into_iter()
        .filter(|&kernel| supports_kernel(RenderBackendKind::Wgpu, kernel))
}

impl EffectKernel {
    const fn label(self) -> &'static str {
        match self {
            Self::ColourTransform => "video-editor.effect.colour-transform",
            Self::GaussianBlur => "video-editor.effect.gaussian-blur",
            Self::HighlightExtract => "video-editor.effect.highlight-extract",
            Self::Composite => "video-editor.effect.composite",
            Self::DirectionalBlur => "video-editor.effect.directional-blur",
            Self::ZoomBlur => "video-editor.effect.zoom-blur",
            Self::ChromaticAberration => "video-editor.effect.chromatic-aberration",
            Self::Vignette => "video-editor.effect.vignette",
            Self::ColorAdjust => "video-editor.effect.color-adjust",
            Self::MotionBlur => "video-editor.effect.motion-blur",
        }
    }

    pub(super) const fn source(self) -> &'static str {
        match self {
            Self::ColourTransform => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/colour_transform.wgsl")
            ),
            Self::GaussianBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/gaussian_blur.wgsl")
            ),
            Self::HighlightExtract => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/highlight_extract.wgsl")
            ),
            Self::Composite => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/composite.wgsl")
            ),
            Self::DirectionalBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/line_blur_common.wgsl"),
                include_str!("../shaders/effects/directional_blur.wgsl")
            ),
            Self::ZoomBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/zoom_blur.wgsl")
            ),
            Self::ChromaticAberration => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/chromatic_aberration.wgsl")
            ),
            Self::Vignette => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/vignette.wgsl")
            ),
            Self::ColorAdjust => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/color_adjust.wgsl")
            ),
            Self::MotionBlur => concat!(
                include_str!("../shaders/effects/common.wgsl"),
                include_str!("../shaders/effects/line_blur_common.wgsl"),
                include_str!("../shaders/effects/motion_blur.wgsl")
            ),
        }
    }
}

fn shader(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}
fn sampled(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            multisampled: false,
            view_dimension: wgpu::TextureViewDimension::D2,
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
        },
        count: None,
    }
}
fn storage_texture(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: wgpu::TextureFormat::Rgba8Unorm,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}
fn pipeline(
    device: &wgpu::Device,
    label: &str,
    shader: &wgpu::ShaderModule,
    bindings: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[bindings],
        push_constant_ranges: &[],
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        module: shader,
        entry_point: "compose",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_shader_sources_isolate_kernel_algorithms() {
        assert!(EffectKernel::GaussianBlur.source().contains("fn gaussian"));
        assert!(!EffectKernel::ColorAdjust.source().contains("fn gaussian"));
        assert!(EffectKernel::ZoomBlur.source().contains("fn zoom_blur"));
        assert!(!EffectKernel::Vignette.source().contains("fn zoom_blur"));
        assert!(
            EffectKernel::DirectionalBlur
                .source()
                .contains("fn line_blur")
        );
        assert!(!EffectKernel::Composite.source().contains("fn line_blur"));
    }
}
