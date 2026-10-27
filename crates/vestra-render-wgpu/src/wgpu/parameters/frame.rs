//! Ordered frame parameter records and particle source preparation.

use bytemuck::Zeroable;
use image::RgbaImage;
use vestra_core::plan::{EvaluatedFrame, EvaluatedSource};

use super::{self as parameters, FrameParameterArena, LayerParameters};
use crate::wgpu::{
    frame_plan::{GpuFramePlan, GpuOperation},
    particles::{self, ParticleUpload, append_particle_upload},
    resources::SourceResources,
};
use crate::{Diagnostic, project::BlendMode};

#[expect(
    clippy::too_many_arguments,
    reason = "parameter preparation receives reusable frame-slot arenas separately to preserve their lifetimes"
)]
pub(in crate::wgpu) fn encode_parameters(
    arena: &mut FrameParameterArena,
    frame: &EvaluatedFrame,
    plan: &mut GpuFramePlan,
    sources: &SourceResources,
    particle_instances: &mut Vec<particles::GpuParticleInstance>,
    particle_pixels: &mut Vec<u8>,
    particle_upload_bytes: &mut Vec<u8>,
    particle_uploads: &mut Vec<Option<ParticleUpload>>,
) -> Result<(), Diagnostic> {
    particle_instances.clear();
    particle_upload_bytes.clear();
    particle_uploads.clear();
    for operation in &mut plan.operations {
        match operation {
            GpuOperation::ClearCanvas { destination, .. } => {
                arena.push(&LayerParameters {
                    header: [frame.width, frame.height, 0, 0],
                    solid_or_background: if destination.is_group_canvas() {
                        [0.0; 4]
                    } else {
                        frame.background.map(f64::from).map(|value| value as f32)
                    },
                    ..LayerParameters::zeroed()
                })?;
            }
            GpuOperation::RenderRasterLayer {
                layer_index,
                source_index,
                ..
            } => {
                let presentation = plan.layers[*layer_index]
                    .source
                    .raster_presentation()
                    .expect("raster frame operation must reference a raster source");
                let intrinsic = sources.raster_textures[*source_index].intrinsic_size;
                let parameters = parameters::raster(
                    frame,
                    intrinsic,
                    presentation.crop,
                    presentation.cacheable_crop,
                    &presentation.sizing,
                    plan.layers[*layer_index].transform,
                    1.0,
                    vestra_core::plan::ColourTransform::default(),
                    parameters::motion_tile(&plan.layers[*layer_index].effects),
                );
                arena.push(&parameters)?;
            }
            GpuOperation::RenderMask {
                layer_index,
                mask_index,
                source_index,
                source_layer,
                ..
            } => {
                let mask = &plan.layers[*layer_index].masks[*mask_index];
                let intrinsic = if *source_layer {
                    crate::render::geometry::IntrinsicSize::new(frame.width, frame.height)
                } else {
                    sources.raster_textures[*source_index].intrinsic_size
                };
                let mut parameters = parameters::raster(
                    frame,
                    intrinsic,
                    crate::domain::Crop {
                        x: 0.0,
                        y: 0.0,
                        width: 1.0,
                        height: 1.0,
                    },
                    false,
                    &vestra_core::plan::CompiledSizing::Original,
                    if *source_layer {
                        crate::animation::Transform2D::identity(
                            crate::domain::Point { x: 0.5, y: 0.5 },
                            crate::domain::Point { x: 0.5, y: 0.5 },
                        )
                    } else {
                        mask.transform
                    },
                    1.0,
                    vestra_core::plan::ColourTransform::default(),
                    None,
                );
                parameters.header[3] = match mask.input {
                    vestra_core::plan::EvaluatedMaskInput::Shape { .. } => 3,
                    vestra_core::plan::EvaluatedMaskInput::Image { mode, .. } => match mode {
                        crate::project::MaskCoverageMode::Alpha => 3,
                        crate::project::MaskCoverageMode::Luma => 4,
                    },
                    vestra_core::plan::EvaluatedMaskInput::Source { mode, .. } => match mode {
                        crate::project::MaskCoverageMode::Alpha => 3,
                        crate::project::MaskCoverageMode::Luma => 4,
                    },
                };
                arena.push(&parameters)?;
            }
            GpuOperation::FeatherMask {
                layer_index,
                mask_index,
                horizontal,
                ..
            } => {
                let radius = plan.layers[*layer_index].masks[*mask_index].feather;
                arena.push(&parameters::mask_feather(frame, radius, *horizontal))?;
            }
            GpuOperation::RenderSurfaceLayer { layer_index, .. } => {
                let vestra_core::plan::EvaluatedSource::Group { .. } =
                    &plan.layers[*layer_index].source
                else {
                    unreachable!("surface frame operation must reference a Group source")
                };
                // Group effects are emitted below as ordinary ordered effect
                // passes.  Keep the surface rasterization neutral so a basic
                // colour chain is not applied once here and once again by
                // ApplyEffect.
                arena.push(&parameters::surface(
                    frame,
                    plan.layers[*layer_index].transform,
                    vestra_core::plan::ColourTransform::default(),
                    parameters::motion_tile(&plan.layers[*layer_index].effects),
                ))?;
            }
            GpuOperation::RenderSolidLayer { layer_index, .. } => {
                let EvaluatedSource::SolidColor { colour } = plan.layers[*layer_index].source
                else {
                    unreachable!("solid frame operation must reference solid source")
                };
                arena.push(&LayerParameters {
                    header: [frame.width, frame.height, 0, 2],
                    effective: [0.0, 0.0, 1.0, 0.0],
                    colour_row0: [1.0, 0.0, 0.0, 0.0],
                    colour_row1: [0.0, 1.0, 0.0, 0.0],
                    colour_row2: [0.0, 0.0, 1.0, 0.0],
                    colour_offset: [0.0, 0.0, 0.0, 0.0],
                    solid_or_background: colour.map(f64::from).map(|value| value as f32),
                    ..LayerParameters::zeroed()
                })?;
            }
            GpuOperation::RenderSpectrum2DLayer { layer_index, .. } => {
                let EvaluatedSource::Spectrum2D {
                    bands,
                    x,
                    y,
                    width,
                    height,
                    bar_gap_ratio,
                    min_bar_height_ratio,
                    layout,
                    gradient,
                    colour,
                } = &plan.layers[*layer_index].source
                else {
                    unreachable!("Spectrum2D frame operation must reference Spectrum2D source")
                };
                arena.push(&parameters::spectrum2d(
                    frame,
                    bands,
                    *x,
                    *y,
                    *width,
                    *height,
                    *bar_gap_ratio,
                    *min_bar_height_ratio,
                    layout,
                    *gradient,
                    *colour,
                )?)?;
            }
            GpuOperation::RenderParticleLayer {
                layer_index,
                instance_offset,
                instance_count,
                ..
            } => {
                let EvaluatedSource::ParticleSystem {
                    system,
                    time_nanos,
                    appearance,
                } = &plan.layers[*layer_index].source
                else {
                    unreachable!("particle frame operation must reference particle source")
                };
                particle_uploads.push(None);
                *instance_offset = u32::try_from(particle_instances.len()).map_err(|_| {
                    Diagnostic::error(
                        "WGPU-PARTICLE-COUNT",
                        crate::Category::Backend,
                        "particle instance count exceeds WGPU draw range",
                        "",
                    )
                })?;
                if matches!(
                    system.blend_mode,
                    crate::project::ParticleBlendMode::Additive
                ) {
                    // CPU Additive is a saturated straight-alpha blend. Portable
                    // fixed-function WGPU blending cannot express that equation
                    // while preserving ordered overlap in one draw, so this is
                    // the exact compatibility path. Effects and outer composition
                    // still run on WGPU after this source upload.
                    let pixel_len = usize::try_from(frame.width)
                        .ok()
                        .and_then(|width| {
                            usize::try_from(frame.height)
                                .ok()
                                .and_then(|height| width.checked_mul(height))
                        })
                        .and_then(|pixels| pixels.checked_mul(4))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "WGPU-PARTICLE-UPLOAD",
                                crate::Category::Backend,
                                "particle image size overflow",
                                "",
                            )
                        })?;
                    particle_pixels.resize(pixel_len, 0);
                    particle_pixels.fill(0);
                    let mut image = RgbaImage::from_raw(
                        frame.width,
                        frame.height,
                        std::mem::take(particle_pixels),
                    )
                    .expect("validated particle image dimensions");
                    crate::particle_raster::rasterize_instances(
                        &mut image,
                        system.evaluated_particles_at_with_appearance(*time_nanos, *appearance),
                        system.primitive,
                        system.blend_mode,
                        vestra_core::plan::ColourTransform::default(),
                    );
                    let pixels = image.into_raw();
                    let upload = append_particle_upload(
                        particle_upload_bytes,
                        &pixels,
                        frame.width,
                        frame.height,
                    )?;
                    particle_uploads
                        .last_mut()
                        .expect("upload entry exists")
                        .replace(upload);
                    *particle_pixels = pixels;
                    *instance_count = 0;
                    arena.push(&parameters::particles(frame, system.primitive))?;
                    continue;
                }
                let range =
                    particles::pack_into(particle_instances, system, *time_nanos, *appearance)?;
                *instance_count = u32::try_from(range.len()).map_err(|_| {
                    Diagnostic::error(
                        "WGPU-PARTICLE-COUNT",
                        crate::Category::Backend,
                        "particle instance count exceeds WGPU draw range",
                        "",
                    )
                })?;
                arena.push(&parameters::particles(frame, system.primitive))?;
            }
            GpuOperation::CompositeLayer { layer_index, .. }
            | GpuOperation::CompositeCachedLayer { layer_index, .. } => {
                arena.push(&LayerParameters {
                    header: [
                        frame.width,
                        frame.height,
                        0,
                        blend_mode(plan.layers[*layer_index].blend_mode),
                    ],
                    effective: [0.0, 0.0, plan.layers[*layer_index].opacity as f32, 0.0],
                    ..LayerParameters::zeroed()
                })?;
            }
            GpuOperation::ApplyEffect { kernel, pass, .. } => {
                let parameters = parameters::effect_parameters(frame.width, frame.height, *pass);
                if parameters.kernel() != *kernel {
                    return Err(Diagnostic::error(
                        "WGPU-EFFECT-KERNEL-MISMATCH",
                        crate::Category::Backend,
                        "encoded effect parameters do not match the frame-plan kernel",
                        "",
                    ));
                }
                parameters::push_effect_parameters(arena, parameters)?;
            }
            GpuOperation::ApplyMask {
                layer_index,
                mask_index,
                ..
            } => {
                let encoded = parameters::mask(
                    frame,
                    &plan.layers[*layer_index].masks[*mask_index],
                    plan.layers[*layer_index].transform,
                    *mask_index == 0,
                );
                arena.push(&encoded)?;
            }
            GpuOperation::UpdateMaskCoverage {
                layer_index,
                mask_index,
                ..
            } => {
                let encoded = parameters::mask(
                    frame,
                    &plan.layers[*layer_index].masks[*mask_index],
                    plan.layers[*layer_index].transform,
                    *mask_index == 0,
                );
                arena.push(&encoded)?;
            }
            GpuOperation::CopyForEffect { .. }
            | GpuOperation::ResolveParticleLayer { .. }
            | GpuOperation::StoreStaticLayer { .. }
            | GpuOperation::CopyForReadback { .. } => {
                continue;
            }
        }
    }
    Ok(())
}

const fn blend_mode(mode: BlendMode) -> u32 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Add => 1,
        BlendMode::Screen => 2,
        BlendMode::Multiply => 3,
        BlendMode::Overlay => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::effects::{CompositeMode, EffectOperation, EffectPass};

    #[test]
    fn effect_parameter_encodings_use_semantic_typed_layouts() {
        let gaussian = parameters::effect_parameters(
            320,
            180,
            EffectPass::new(
                EffectOperation::GaussianVertical {
                    integer: false,
                    radius: 4.0,
                },
                vestra_core::plan::EffectResource::Current,
                vestra_core::plan::EffectResource::Current,
            ),
        );
        let parameters::EffectKernelParameters::GaussianBlur(gaussian) = gaussian else {
            panic!("Gaussian pass must use the Gaussian layout");
        };
        assert_eq!((gaussian.canvas_width, gaussian.canvas_height), (320, 180));
        assert_eq!(gaussian.radius, 4.0);
        assert_eq!(gaussian.direction, 1);

        let composite = parameters::effect_parameters(
            320,
            180,
            EffectPass::new(
                EffectOperation::Composite {
                    mode: CompositeMode::Additive,
                    amount: 0.75,
                },
                vestra_core::plan::EffectResource::Original,
                vestra_core::plan::EffectResource::Current,
            ),
        );
        let parameters::EffectKernelParameters::Composite(composite) = composite else {
            panic!("Composite pass must use the Composite layout");
        };
        assert_eq!(composite.mode, 0);
        assert_eq!(composite.amount, 0.75);

        let vignette = parameters::effect_parameters(
            320,
            180,
            EffectPass::new(
                EffectOperation::Vignette {
                    amount: 0.5,
                    radius: 0.6,
                    softness: 0.2,
                    colour: [10, 20, 30, 255],
                },
                vestra_core::plan::EffectResource::Current,
                vestra_core::plan::EffectResource::Current,
            ),
        );
        let parameters::EffectKernelParameters::Vignette(vignette) = vignette else {
            panic!("Vignette pass must use the Vignette layout");
        };
        assert_eq!(
            (vignette.amount, vignette.radius, vignette.softness),
            (0.5, 0.6, 0.2)
        );
        assert_eq!(vignette.colour, [10.0, 20.0, 30.0, 255.0]);
        assert_eq!(
            std::mem::size_of::<parameters::GaussianBlurParameters>() % 16,
            0
        );
        assert_eq!(
            std::mem::size_of::<parameters::VignetteParameters>() % 16,
            0
        );
        assert!(
            std::mem::size_of::<parameters::GaussianBlurParameters>()
                < std::mem::size_of::<LayerParameters>()
        );
    }
}
