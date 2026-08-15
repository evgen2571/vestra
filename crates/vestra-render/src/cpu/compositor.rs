use std::{sync::Arc, time::Instant};

use image::{Rgba, RgbaImage};

use crate::plan::{
    ColourTransform, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource,
    TemporalDependency,
};
use crate::{
    blend::blend_surface,
    cpu::{assets::PreparedAssets, effects, raster::draw_layer},
    render::{ByteLruCache, metrics::CpuHotPathTimings},
};

pub(crate) use super::surfaces::{CompositionSurfacePool, EffectSurfacePool};

/// Immutable complete layer output retained by the CPU static-layer cache.
pub(crate) struct CachedCpuLayerSurface {
    image: RgbaImage,
    fully_opaque: bool,
}

impl CachedCpuLayerSurface {
    fn from_image(image: RgbaImage) -> Self {
        let fully_opaque = image
            .as_raw()
            .chunks_exact(4)
            .all(|pixel| pixel[3] == u8::MAX);
        Self {
            image,
            fully_opaque,
        }
    }
}

#[derive(Default)]
pub(crate) struct ComposeStats {
    pub static_layer_renders: u64,
    pub opaque_copy_fast_path_hits: u64,
    pub opaque_copy_fast_path_bytes: u64,
    pub generic_blend_surface_calls: u64,
}

#[cfg(test)]
use crate::cpu::effects::blur;
#[cfg(test)]
use crate::cpu::raster::{apply_colour_transform, draw_image, sample_bilinear, visible_bounds};
#[cfg(test)]
use crate::{animation::Transform2D, domain::Crop, render::geometry};

/// Composites an immutable, backend-neutral frame program into a reusable buffer.
#[allow(clippy::too_many_arguments)]
pub fn compose(
    frame: &EvaluatedFrame,
    assets: &mut PreparedAssets,
    canvas: &mut RgbaImage,
    surfaces: &mut EffectSurfacePool,
    compositions: &mut CompositionSurfacePool,
    static_layers: &mut ByteLruCache<usize, Arc<CachedCpuLayerSurface>>,
    timings: &mut CpuHotPathTimings,
    profiling_enabled: bool,
) -> ComposeStats {
    let mut stats = ComposeStats::default();
    let first_is_opaque_cached = frame.layers.first().and_then(|layer| {
        static_layers
            .peek(&layer.compiled_layer_index)
            .map(|cached| is_opaque_copy(layer, cached, frame.width, frame.height))
    }) == Some(true);
    if canvas.width() != frame.width || canvas.height() != frame.height {
        *canvas = RgbaImage::new(frame.width, frame.height);
        if !first_is_opaque_cached {
            for pixel in canvas.pixels_mut() {
                *pixel = Rgba(frame.background);
            }
        }
    } else if !first_is_opaque_cached {
        for pixel in canvas.pixels_mut() {
            *pixel = Rgba(frame.background);
        }
    }
    let started = profiling_enabled.then(Instant::now);
    surfaces.resize(frame.width, frame.height);
    if let Some(started) = started {
        timings.layer_composition += started.elapsed();
    }
    compose_layers(
        &frame.layers,
        frame.width,
        frame.height,
        canvas,
        assets,
        surfaces,
        compositions,
        static_layers,
        timings,
        profiling_enabled,
        0,
        &mut stats,
    );
    effects::apply_to(
        surfaces,
        canvas,
        &frame.post_effects,
        timings,
        profiling_enabled,
    );
    stats
}

#[allow(clippy::too_many_arguments)]
fn compose_layers(
    layers: &[EvaluatedLayer],
    width: u32,
    height: u32,
    canvas: &mut RgbaImage,
    assets: &mut PreparedAssets,
    surfaces: &mut EffectSurfacePool,
    compositions: &mut CompositionSurfacePool,
    static_layers: &mut ByteLruCache<usize, Arc<CachedCpuLayerSurface>>,
    timings: &mut CpuHotPathTimings,
    profiling_enabled: bool,
    depth: usize,
    stats: &mut ComposeStats,
) {
    for layer in layers {
        if let EvaluatedSource::Group { composition } = &layer.source {
            render_group(
                layer,
                composition,
                layer.transform,
                width,
                height,
                canvas,
                assets,
                surfaces,
                compositions,
                static_layers,
                timings,
                profiling_enabled,
                depth,
                stats,
            );
            continue;
        }

        if layer.content_dependency == TemporalDependency::Static {
            if let Some(cached) = static_layers.get(&layer.compiled_layer_index).cloned() {
                composite_cached_surface(
                    canvas,
                    &cached,
                    layer,
                    stats,
                    profiling_enabled.then_some(&mut timings.composition_cases),
                );
                continue;
            }
            surfaces.clear();
            stats.static_layer_renders += 1;
            if uses_direct_colour_path(layer) {
                let started = profiling_enabled.then(Instant::now);
                draw_layer(
                    surfaces.current(),
                    assets,
                    layer,
                    1.0,
                    layer.colour_transform,
                    timings,
                    profiling_enabled,
                );
                if let Some(started) = started {
                    timings.source_rasterization += started.elapsed();
                }
            } else {
                let started = profiling_enabled.then(Instant::now);
                draw_layer(
                    surfaces.current(),
                    assets,
                    layer,
                    1.0,
                    ColourTransform::default(),
                    timings,
                    profiling_enabled,
                );
                if let Some(started) = started {
                    timings.source_rasterization += started.elapsed();
                }
                effects::apply_chain(surfaces, &layer.effects, timings, profiling_enabled);
            }
            let bytes = u64::from(width) * u64::from(height) * 4;
            if let Some(cached) =
                static_layers.insert_with(layer.compiled_layer_index, bytes, || {
                    Arc::new(CachedCpuLayerSurface::from_image(surfaces.take_current()))
                })
            {
                composite_cached_surface(
                    canvas,
                    cached,
                    layer,
                    stats,
                    profiling_enabled.then_some(&mut timings.composition_cases),
                );
            } else {
                let started = profiling_enabled.then(Instant::now);
                blend_surface(
                    canvas,
                    surfaces.current(),
                    layer.blend_mode,
                    layer.opacity,
                    profiling_enabled.then_some(&mut timings.composition_cases),
                );
                if let Some(started) = started {
                    timings.layer_composition += started.elapsed();
                }
                stats.generic_blend_surface_calls += 1;
            }
            continue;
        }
        if uses_direct_colour_path(layer) {
            let started = profiling_enabled.then(Instant::now);
            draw_layer(
                canvas,
                assets,
                layer,
                layer.opacity,
                layer.colour_transform,
                timings,
                profiling_enabled,
            );
            if let Some(started) = started {
                timings.source_rasterization += started.elapsed();
            }
            continue;
        }
        surfaces.clear();
        let started = profiling_enabled.then(Instant::now);
        draw_layer(
            surfaces.current(),
            assets,
            layer,
            1.0,
            ColourTransform::default(),
            timings,
            profiling_enabled,
        );
        if let Some(started) = started {
            timings.source_rasterization += started.elapsed();
        }
        effects::apply_chain(surfaces, &layer.effects, timings, profiling_enabled);
        let started = profiling_enabled.then(Instant::now);
        blend_surface(
            canvas,
            surfaces.current(),
            layer.blend_mode,
            layer.opacity,
            profiling_enabled.then_some(&mut timings.composition_cases),
        );
        if let Some(started) = started {
            timings.layer_composition += started.elapsed();
        }
        stats.generic_blend_surface_calls += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn render_group(
    layer: &EvaluatedLayer,
    composition: &crate::plan::EvaluatedComposition,
    transform: crate::animation::Transform2D,
    width: u32,
    height: u32,
    parent: &mut RgbaImage,
    assets: &mut PreparedAssets,
    surfaces: &mut EffectSurfacePool,
    compositions: &mut CompositionSurfacePool,
    static_layers: &mut ByteLruCache<usize, Arc<CachedCpuLayerSurface>>,
    timings: &mut CpuHotPathTimings,
    profiling_enabled: bool,
    depth: usize,
    stats: &mut ComposeStats,
) {
    if layer.content_dependency == TemporalDependency::Static {
        if let Some(cached) = static_layers.get(&layer.compiled_layer_index).cloned() {
            composite_cached_surface(
                parent,
                &cached,
                layer,
                stats,
                profiling_enabled.then_some(&mut timings.composition_cases),
            );
            return;
        }
        stats.static_layer_renders += 1;
    }

    let mut group_surface = compositions.acquire(depth, width, height);
    compose_layers(
        &composition.layers,
        width,
        height,
        &mut group_surface,
        assets,
        surfaces,
        compositions,
        static_layers,
        timings,
        profiling_enabled,
        depth + 1,
        stats,
    );

    // A Group becomes image-like only after all children have been composed.
    // The effect pool is separate so the live composition surface remains
    // available while nested composition is unwound.
    surfaces.clear();
    let direct_colour_path = uses_direct_colour_path(layer);
    let started = profiling_enabled.then(Instant::now);
    crate::cpu::raster::draw_surface(
        surfaces.current(),
        &group_surface,
        transform,
        if direct_colour_path {
            layer.colour_transform
        } else {
            ColourTransform::default()
        },
    );
    if let Some(started) = started {
        timings.transform_sampling += started.elapsed();
    }
    if !direct_colour_path {
        effects::apply_chain(surfaces, &layer.effects, timings, profiling_enabled);
    }
    if layer.content_dependency == TemporalDependency::Static {
        let bytes = u64::from(width) * u64::from(height) * 4;
        if let Some(cached) = static_layers.insert_with(layer.compiled_layer_index, bytes, || {
            Arc::new(CachedCpuLayerSurface::from_image(surfaces.take_current()))
        }) {
            composite_cached_surface(
                parent,
                cached,
                layer,
                stats,
                profiling_enabled.then_some(&mut timings.composition_cases),
            );
        } else {
            let started = profiling_enabled.then(Instant::now);
            blend_surface(
                parent,
                surfaces.current(),
                layer.blend_mode,
                layer.opacity,
                profiling_enabled.then_some(&mut timings.composition_cases),
            );
            if let Some(started) = started {
                timings.layer_composition += started.elapsed();
            }
            stats.generic_blend_surface_calls += 1;
        }
    } else {
        let started = profiling_enabled.then(Instant::now);
        blend_surface(
            parent,
            surfaces.current(),
            layer.blend_mode,
            layer.opacity,
            profiling_enabled.then_some(&mut timings.composition_cases),
        );
        if let Some(started) = started {
            timings.layer_composition += started.elapsed();
        }
        stats.generic_blend_surface_calls += 1;
    }
    compositions.release(depth, group_surface);
}

fn is_opaque_copy(
    layer: &EvaluatedLayer,
    cached: &CachedCpuLayerSurface,
    width: u32,
    height: u32,
) -> bool {
    matches!(layer.blend_mode, crate::project::BlendMode::Normal)
        && layer.opacity == 1.0
        && cached.fully_opaque
        && cached.image.width() == width
        && cached.image.height() == height
}

fn composite_cached_surface(
    canvas: &mut RgbaImage,
    cached: &CachedCpuLayerSurface,
    layer: &EvaluatedLayer,
    stats: &mut ComposeStats,
    cases: Option<&mut crate::blend::CompositionCaseCounts>,
) {
    if is_opaque_copy(layer, cached, canvas.width(), canvas.height()) {
        canvas.as_mut().copy_from_slice(cached.image.as_raw());
        stats.opaque_copy_fast_path_hits += 1;
        stats.opaque_copy_fast_path_bytes += cached.image.as_raw().len() as u64;
    } else {
        blend_surface(
            canvas,
            &cached.image,
            layer.blend_mode,
            layer.opacity,
            cases,
        );
        stats.generic_blend_surface_calls += 1;
    }
}

fn uses_direct_colour_path(layer: &EvaluatedLayer) -> bool {
    matches!(layer.blend_mode, crate::project::BlendMode::Normal)
        && !matches!(layer.source, EvaluatedSource::ParticleSystem { .. })
        && layer
            .effects
            .iter()
            .all(EvaluatedEffect::is_basic_colour_effect)
}

/// Test helper that keeps the compositor assertions on the production blur path.
#[cfg(test)]
fn zoom_blur(
    source: &RgbaImage,
    target: &mut RgbaImage,
    radius: f64,
    samples: u8,
    anchor: crate::domain::Point,
    direction: crate::project::ZoomBlurDirection,
) {
    crate::cpu::zoom_blur::apply(source, target, radius, samples, anchor, direction);
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::blend::{blend_pixel, source_over};
    use crate::effects::effect_pass_plan;
    use crate::plan::EvaluatedComposition;
    use crate::plan::EvaluatedEffect;

    fn apply_sequential(mut rgb: [f64; 3], effects: &[EvaluatedEffect]) -> [f64; 3] {
        for effect in effects {
            match effect {
                EvaluatedEffect::Brightness { amount } => {
                    rgb = rgb.map(|channel| channel + amount * 255.0);
                }
                EvaluatedEffect::Contrast { amount } => {
                    rgb = rgb.map(|channel| (channel - 128.0) * amount + 128.0);
                }
                EvaluatedEffect::Saturation { amount } => {
                    let luma = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
                    rgb = rgb.map(|channel| luma * (1.0 - amount) + channel * amount);
                }
                EvaluatedEffect::Tint { colour, amount } => {
                    let amount = amount.clamp(0.0, 1.0);
                    rgb = std::array::from_fn(|channel| {
                        rgb[channel] * (1.0 - amount) + f64::from(colour[channel]) * amount
                    });
                }
                _ => {}
            }
        }
        rgb
    }

    fn cached(alpha: u8) -> CachedCpuLayerSurface {
        CachedCpuLayerSurface::from_image(RgbaImage::from_pixel(2, 2, Rgba([30, 60, 90, alpha])))
    }

    fn layer(opacity: f64, blend_mode: crate::project::BlendMode) -> EvaluatedLayer {
        EvaluatedLayer {
            compiled_layer_index: 0,
            content_dependency: TemporalDependency::Static,
            source: EvaluatedSource::SolidColor {
                colour: [0, 0, 0, 255],
            },
            transform: identity_transform(),
            opacity,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode,
        }
    }

    fn identity_transform() -> Transform2D {
        Transform2D::identity(
            crate::domain::Point { x: 0.5, y: 0.5 },
            crate::domain::Point { x: 0.5, y: 0.5 },
        )
    }

    fn render_test_frame(frame: EvaluatedFrame) -> RgbaImage {
        let validated = crate::project::load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &crate::project::ValidationOptions {
                check_backend: false,
                ..crate::project::ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = crate::plan::compile(&validated, crate::plan::CompileOptions::default())
            .expect("fixture compiles");
        render_frame_with_plan(frame, &plan)
    }

    fn render_frame_with_plan(frame: EvaluatedFrame, plan: &crate::plan::RenderPlan) -> RgbaImage {
        let mut assets = crate::cpu::assets::PreparedAssets::build(plan).expect("assets decode");
        let mut canvas = RgbaImage::new(frame.width, frame.height);
        let mut effects = EffectSurfacePool::new(frame.width, frame.height);
        let mut compositions = CompositionSurfacePool::new();
        let mut cache = ByteLruCache::new(0);
        compose(
            &frame,
            &mut assets,
            &mut canvas,
            &mut effects,
            &mut compositions,
            &mut cache,
            &mut crate::render::metrics::CpuHotPathTimings::default(),
            false,
        );
        canvas
    }

    fn frame_with_layers(layers: Vec<EvaluatedLayer>) -> EvaluatedFrame {
        EvaluatedFrame {
            time: 0,
            background: [0, 0, 0, 0],
            width: 16,
            height: 16,
            layers,
            post_effects: Vec::new(),
            evaluated_track_count: 0,
        }
    }

    fn image_layer(index: usize, position: crate::domain::Point) -> EvaluatedLayer {
        EvaluatedLayer {
            compiled_layer_index: index,
            content_dependency: TemporalDependency::Dynamic,
            source: EvaluatedSource::Image {
                asset_index: 0,
                crop: Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                sizing: crate::plan::CompiledSizing::Fit,
                cacheable_crop: false,
            },
            transform: Transform2D {
                position,
                ..identity_transform()
            },
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }
    }

    fn scaled_image_layer(index: usize, position: crate::domain::Point) -> EvaluatedLayer {
        let mut layer = image_layer(index, position);
        layer.transform.scale = crate::domain::Point { x: 0.35, y: 0.35 };
        layer
    }

    fn solid_layer(index: usize, colour: [u8; 4]) -> EvaluatedLayer {
        EvaluatedLayer {
            compiled_layer_index: index,
            content_dependency: TemporalDependency::Dynamic,
            source: EvaluatedSource::SolidColor { colour },
            transform: identity_transform(),
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }
    }

    fn group_layer(
        index: usize,
        children: Vec<EvaluatedLayer>,
        effects: Vec<EvaluatedEffect>,
        transform: Transform2D,
    ) -> EvaluatedLayer {
        EvaluatedLayer {
            compiled_layer_index: index,
            content_dependency: TemporalDependency::Dynamic,
            source: EvaluatedSource::Group {
                composition: EvaluatedComposition { layers: children },
            },
            transform,
            opacity: 1.0,
            colour_transform: ColourTransform::from_effects(effects.clone()),
            effects,
            blend_mode: crate::project::BlendMode::Normal,
        }
    }

    #[test]
    fn static_group_is_cached_at_the_complete_layer_stage() {
        let validated = crate::project::load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &crate::project::ValidationOptions {
                check_backend: false,
                ..crate::project::ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = crate::plan::compile(&validated, crate::plan::CompileOptions::default())
            .expect("fixture compiles");
        let mut assets = crate::cpu::assets::PreparedAssets::build(&plan).expect("assets decode");
        let child = {
            let mut child = solid_layer(2, [40, 80, 120, 255]);
            child.content_dependency = TemporalDependency::Static;
            child
        };
        let mut group = group_layer(1, vec![child], Vec::new(), identity_transform());
        group.content_dependency = TemporalDependency::Static;
        let frame = frame_with_layers(vec![group]);
        let mut canvas = RgbaImage::new(frame.width, frame.height);
        let mut surfaces = EffectSurfacePool::new(frame.width, frame.height);
        let mut compositions = CompositionSurfacePool::new();
        let mut cache = ByteLruCache::new(u64::from(frame.width) * u64::from(frame.height) * 4 * 4);
        let mut timings = crate::render::metrics::CpuHotPathTimings::default();

        let first = compose(
            &frame,
            &mut assets,
            &mut canvas,
            &mut surfaces,
            &mut compositions,
            &mut cache,
            &mut timings,
            false,
        );
        let first_pixels = canvas.clone();
        let second = compose(
            &frame,
            &mut assets,
            &mut canvas,
            &mut surfaces,
            &mut compositions,
            &mut cache,
            &mut timings,
            false,
        );

        assert_eq!(first.static_layer_renders, 2);
        assert_eq!(second.static_layer_renders, 0);
        assert_eq!(cache.stats().insertions, 2);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(canvas, first_pixels);
    }

    #[test]
    fn dynamic_group_never_reuses_a_static_surface() {
        let validated = crate::project::load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &crate::project::ValidationOptions {
                check_backend: false,
                ..crate::project::ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = crate::plan::compile(&validated, crate::plan::CompileOptions::default())
            .expect("fixture compiles");
        let mut assets = crate::cpu::assets::PreparedAssets::build(&plan).expect("assets decode");
        let make_frame = |colour| {
            frame_with_layers(vec![group_layer(
                1,
                vec![solid_layer(2, colour)],
                Vec::new(),
                identity_transform(),
            )])
        };
        let first_frame = make_frame([255, 0, 0, 255]);
        let second_frame = make_frame([0, 0, 255, 255]);
        let mut canvas = RgbaImage::new(first_frame.width, first_frame.height);
        let mut surfaces = EffectSurfacePool::new(first_frame.width, first_frame.height);
        let mut compositions = CompositionSurfacePool::new();
        let mut cache = ByteLruCache::new(16 * 16 * 4 * 4);
        let mut timings = crate::render::metrics::CpuHotPathTimings::default();

        compose(
            &first_frame,
            &mut assets,
            &mut canvas,
            &mut surfaces,
            &mut compositions,
            &mut cache,
            &mut timings,
            false,
        );
        let first_pixel = *canvas.get_pixel(8, 8);
        compose(
            &second_frame,
            &mut assets,
            &mut canvas,
            &mut surfaces,
            &mut compositions,
            &mut cache,
            &mut timings,
            false,
        );
        assert_ne!(first_pixel, *canvas.get_pixel(8, 8));
        assert_eq!(cache.stats().requests, 0);
    }

    #[test]
    fn opaque_normal_cached_surface_uses_bulk_copy_only_at_exact_opacity() {
        let mut canvas = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        let cached = cached(255);
        let mut stats = ComposeStats::default();
        composite_cached_surface(
            &mut canvas,
            &cached,
            &layer(1.0, crate::project::BlendMode::Normal),
            &mut stats,
            None,
        );
        assert_eq!(canvas.as_raw(), cached.image.as_raw());
        assert_eq!(stats.opaque_copy_fast_path_hits, 1);
        assert_eq!(stats.generic_blend_surface_calls, 0);

        composite_cached_surface(
            &mut canvas,
            &cached,
            &layer(0.999, crate::project::BlendMode::Normal),
            &mut stats,
            None,
        );
        assert_eq!(stats.opaque_copy_fast_path_hits, 1);
        assert_eq!(stats.generic_blend_surface_calls, 1);
    }

    #[test]
    fn transparent_or_non_normal_cached_surface_uses_generic_blending() {
        let mut canvas = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
        let mut stats = ComposeStats::default();
        composite_cached_surface(
            &mut canvas,
            &cached(254),
            &layer(1.0, crate::project::BlendMode::Normal),
            &mut stats,
            None,
        );
        composite_cached_surface(
            &mut canvas,
            &cached(255),
            &layer(1.0, crate::project::BlendMode::Screen),
            &mut stats,
            None,
        );
        assert_eq!(stats.opaque_copy_fast_path_hits, 0);
        assert_eq!(stats.generic_blend_surface_calls, 2);
    }

    #[test]
    fn group_opacity_is_applied_once_to_the_isolated_child_result() {
        let validated = crate::project::load_and_validate(
            std::path::Path::new("examples/projects/animation-effects.json"),
            &crate::project::ValidationOptions {
                check_backend: false,
                ..crate::project::ValidationOptions::default()
            },
        )
        .expect("fixture validates");
        let plan = crate::plan::compile(&validated, crate::plan::CompileOptions::default())
            .expect("fixture compiles");
        let mut assets = crate::cpu::assets::PreparedAssets::build(&plan).expect("assets decode");
        let child = |index, colour| EvaluatedLayer {
            compiled_layer_index: index,
            content_dependency: TemporalDependency::Dynamic,
            source: EvaluatedSource::SolidColor { colour },
            transform: identity_transform(),
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        };
        let frame = EvaluatedFrame {
            time: 0,
            background: [0, 0, 0, 0],
            width: 2,
            height: 2,
            layers: vec![EvaluatedLayer {
                compiled_layer_index: 50,
                content_dependency: TemporalDependency::Dynamic,
                source: EvaluatedSource::Group {
                    composition: EvaluatedComposition {
                        layers: vec![
                            child(51, [255, 0, 0, 128]),
                            EvaluatedLayer {
                                compiled_layer_index: 52,
                                content_dependency: TemporalDependency::Dynamic,
                                source: EvaluatedSource::Group {
                                    composition: EvaluatedComposition {
                                        layers: vec![child(53, [0, 0, 255, 128])],
                                    },
                                },
                                transform: Transform2D::identity(
                                    crate::domain::Point { x: 0.5, y: 0.5 },
                                    crate::domain::Point { x: 0.5, y: 0.5 },
                                ),
                                opacity: 1.0,
                                effects: Vec::new(),
                                colour_transform: ColourTransform::default(),
                                blend_mode: crate::project::BlendMode::Normal,
                            },
                        ],
                    },
                },
                transform: Transform2D::identity(
                    crate::domain::Point { x: 0.5, y: 0.5 },
                    crate::domain::Point { x: 0.5, y: 0.5 },
                ),
                opacity: 0.5,
                effects: Vec::new(),
                colour_transform: ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Normal,
            }],
            post_effects: Vec::new(),
            evaluated_track_count: 0,
        };
        let mut canvas = RgbaImage::new(2, 2);
        let mut effects = EffectSurfacePool::new(2, 2);
        let mut compositions = CompositionSurfacePool::new();
        let mut cache = ByteLruCache::new(0);
        compose(
            &frame,
            &mut assets,
            &mut canvas,
            &mut effects,
            &mut compositions,
            &mut cache,
            &mut crate::render::metrics::CpuHotPathTimings::default(),
            false,
        );

        let mut isolated = Rgba([0, 0, 0, 0]);
        isolated = blend_pixel(
            isolated,
            Rgba([255, 0, 0, 128]),
            crate::project::BlendMode::Normal,
            1.0,
        );
        isolated = blend_pixel(
            isolated,
            Rgba([0, 0, 255, 128]),
            crate::project::BlendMode::Normal,
            1.0,
        );
        let expected = blend_pixel(
            Rgba([0, 0, 0, 0]),
            isolated,
            crate::project::BlendMode::Normal,
            0.5,
        );
        assert_eq!(canvas.get_pixel(0, 0), &expected);
    }

    #[test]
    fn group_basic_colour_effect_is_applied_once() {
        let effects = vec![EvaluatedEffect::Brightness { amount: 0.1 }];
        let output = render_test_frame(frame_with_layers(vec![group_layer(
            1,
            vec![EvaluatedLayer {
                compiled_layer_index: 2,
                content_dependency: TemporalDependency::Dynamic,
                source: EvaluatedSource::SolidColor {
                    colour: [80, 110, 160, 255],
                },
                transform: identity_transform(),
                opacity: 1.0,
                effects: Vec::new(),
                colour_transform: ColourTransform::default(),
                blend_mode: crate::project::BlendMode::Normal,
            }],
            effects.clone(),
            identity_transform(),
        )]));
        let expected = apply_colour_transform(
            Rgba([80, 110, 160, 255]),
            ColourTransform::from_effects(effects),
        );
        assert_eq!(output.get_pixel(8, 8), &expected);
    }

    #[test]
    fn group_multiple_basic_colour_effects_match_ordinary_layer_semantics() {
        let effects = vec![
            EvaluatedEffect::Brightness { amount: 0.08 },
            EvaluatedEffect::Contrast { amount: 1.15 },
            EvaluatedEffect::Saturation { amount: 0.6 },
        ];
        let group = group_layer(
            1,
            vec![solid_layer(2, [80, 110, 160, 255])],
            effects.clone(),
            identity_transform(),
        );
        let mut child = solid_layer(2, [80, 110, 160, 255]);
        child.effects = effects.clone();
        child.colour_transform = ColourTransform::from_effects(effects);
        let grouped = render_test_frame(frame_with_layers(vec![group]));
        let ordinary = render_test_frame(frame_with_layers(vec![child]));
        assert_eq!(grouped, ordinary);
    }

    #[test]
    fn group_mixed_effects_match_ordinary_order_in_both_declared_orders() {
        for effects in [
            vec![
                EvaluatedEffect::Brightness { amount: 0.15 },
                EvaluatedEffect::GaussianBlur { radius: 1.0 },
            ],
            vec![
                EvaluatedEffect::GaussianBlur { radius: 1.0 },
                EvaluatedEffect::Brightness { amount: 0.15 },
            ],
        ] {
            let mut ordinary = image_layer(1, crate::domain::Point { x: 0.5, y: 0.5 });
            ordinary.effects = effects.clone();
            ordinary.colour_transform = ColourTransform::from_effects(effects.clone());
            let group = group_layer(
                3,
                vec![image_layer(4, crate::domain::Point { x: 0.5, y: 0.5 })],
                effects,
                identity_transform(),
            );
            assert_eq!(
                render_test_frame(frame_with_layers(vec![group])),
                render_test_frame(frame_with_layers(vec![ordinary])),
            );
        }
    }

    #[test]
    fn group_transform_moves_the_completed_multi_child_arrangement() {
        let children = vec![
            scaled_image_layer(1, crate::domain::Point { x: 0.25, y: 0.5 }),
            scaled_image_layer(2, crate::domain::Point { x: 0.75, y: 0.5 }),
        ];
        let unchanged = render_test_frame(frame_with_layers(vec![group_layer(
            3,
            children.clone(),
            Vec::new(),
            identity_transform(),
        )]));
        let shifted = render_test_frame(frame_with_layers(vec![group_layer(
            3,
            children,
            Vec::new(),
            Transform2D {
                position: crate::domain::Point { x: 0.65, y: 0.5 },
                ..identity_transform()
            },
        )]));
        assert_ne!(unchanged, shifted);
        assert!(
            shifted
                .enumerate_pixels()
                .any(|(x, _, pixel)| x > 8 && pixel[3] > 0)
        );
    }

    #[test]
    fn group_transform_preserves_transparent_edges_and_empty_groups() {
        let background = [9, 8, 7, 255];
        let child = scaled_image_layer(1, crate::domain::Point { x: 0.5, y: 0.5 });
        let mut frame = frame_with_layers(vec![group_layer(
            2,
            vec![child],
            Vec::new(),
            Transform2D {
                rotation_radians: 0.35,
                ..identity_transform()
            },
        )]);
        frame.background = background;
        let output = render_test_frame(frame);
        assert_eq!(output.get_pixel(0, 0), &Rgba(background));

        let mut empty = frame_with_layers(vec![group_layer(
            4,
            Vec::new(),
            Vec::new(),
            identity_transform(),
        )]);
        empty.background = background;
        let output = render_test_frame(empty);
        assert!(output.pixels().all(|pixel| *pixel == Rgba(background)));
    }

    #[test]
    fn sibling_groups_keep_both_surfaces_and_group_blend_applies_to_completed_result() {
        let siblings = vec![
            group_layer(
                1,
                vec![scaled_image_layer(
                    2,
                    crate::domain::Point { x: 0.25, y: 0.5 },
                )],
                Vec::new(),
                identity_transform(),
            ),
            group_layer(
                3,
                vec![scaled_image_layer(
                    4,
                    crate::domain::Point { x: 0.75, y: 0.5 },
                )],
                Vec::new(),
                identity_transform(),
            ),
        ];
        let output = render_test_frame(frame_with_layers(siblings));
        assert!(
            output
                .enumerate_pixels()
                .any(|(x, _, pixel)| x < 6 && pixel[3] > 0)
        );
        assert!(
            output
                .enumerate_pixels()
                .any(|(x, _, pixel)| x > 9 && pixel[3] > 0)
        );

        let background = Rgba([40, 100, 200, 255]);
        let source = Rgba([200, 80, 20, 255]);
        let mut frame = frame_with_layers(vec![group_layer(
            5,
            vec![solid_layer(6, source.0)],
            Vec::new(),
            identity_transform(),
        )]);
        frame.background = background.0;
        if let Some(layer) = frame.layers.first_mut() {
            layer.blend_mode = crate::project::BlendMode::Multiply;
        }
        assert_eq!(
            render_test_frame(frame).get_pixel(8, 8),
            &blend_pixel(background, source, crate::project::BlendMode::Multiply, 1.0)
        );
    }

    #[test]
    fn three_nested_groups_recurse_without_losing_the_child() {
        let child = solid_layer(4, [180, 20, 40, 255]);
        let level_c = group_layer(3, vec![child], Vec::new(), identity_transform());
        let level_b = group_layer(2, vec![level_c], Vec::new(), identity_transform());
        let level_a = group_layer(1, vec![level_b], Vec::new(), identity_transform());
        let output = render_test_frame(frame_with_layers(vec![level_a]));
        assert_eq!(output.get_pixel(8, 8), &Rgba([180, 20, 40, 255]));
    }

    #[test]
    fn particle_and_spectrum_sources_render_inside_groups_deterministically() {
        let particle_system = crate::plan::CompiledParticleSystem {
            seed: 7,
            emitter: crate::project::ParticleEmitter::default(),
            rate_units_per_second: 0,
            lifetime_nanos: 1_000_000_000,
            lifetime_range: crate::project::ScalarRange { min: 1.0, max: 1.0 },
            initial_velocity: crate::domain::Point { x: 0.0, y: 0.0 },
            speed: crate::project::ScalarRange { min: 0.0, max: 0.0 },
            direction_degrees: 0.0,
            direction_spread_degrees: 0.0,
            acceleration: crate::domain::Point { x: 0.0, y: 0.0 },
            size: 0.5,
            size_range: None,
            opacity: 1.0,
            colour: [220, 40, 20, 255],
            rotation_degrees: 0.0,
            rotation_range: None,
            angular_velocity_degrees: 0.0,
            angular_velocity_range: None,
            primitive: crate::project::ParticlePrimitive::Square,
            blend_mode: crate::project::ParticleBlendMode::Normal,
            bursts: vec![crate::plan::CompiledParticleBurst {
                time_nanos: 0,
                count: 1,
            }],
            maximum_live_particles: 1,
            lifetime_size: None,
            lifetime_opacity: None,
            lifetime_colour: None,
            audio_size: None,
            audio_opacity: None,
            audio_intensity: None,
        };
        let particle = EvaluatedLayer {
            compiled_layer_index: 2,
            content_dependency: TemporalDependency::Dynamic,
            source: EvaluatedSource::ParticleSystem {
                system: std::sync::Arc::new(particle_system),
                time_nanos: 0,
                appearance: crate::plan::EvaluatedParticleAppearance::default(),
            },
            transform: identity_transform(),
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        };
        let particle_group = group_layer(
            1,
            vec![particle],
            Vec::new(),
            Transform2D {
                position: crate::domain::Point { x: 0.6, y: 0.5 },
                ..identity_transform()
            },
        );
        let first = render_test_frame(frame_with_layers(vec![particle_group.clone()]));
        let second = render_test_frame(frame_with_layers(vec![particle_group]));
        assert_eq!(first, second);
        assert!(first.pixels().any(|pixel| pixel[3] > 0));

        let spectrum = EvaluatedLayer {
            compiled_layer_index: 4,
            content_dependency: TemporalDependency::Dynamic,
            source: EvaluatedSource::Spectrum2D {
                bands: vec![1.0, 0.75, 0.5],
                x: 0.25,
                y: 0.25,
                width: 0.5,
                height: 0.5,
                bar_gap_ratio: 0.1,
                min_bar_height_ratio: 0.1,
                layout: crate::project::Spectrum2DLayout::default(),
                gradient: None,
                colour: [30, 180, 240, 255],
            },
            transform: identity_transform(),
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        };
        let spectrum_output = render_test_frame(frame_with_layers(vec![group_layer(
            5,
            vec![spectrum],
            Vec::new(),
            identity_transform(),
        )]));
        assert!(spectrum_output.pixels().any(|pixel| pixel[3] > 0));
    }

    #[test]
    fn canonical_group_project_validates_compiles_evaluates_and_renders_on_cpu() {
        let project = crate::project::Project::from_json(
            r##"{
                "schema_version": 3,
                "name": "cpu group integration",
                "output": {
                    "path": "group.mp4",
                    "width": 4,
                    "height": 4,
                    "frame_rate": "24/1",
                    "background": "#00000000",
                    "quality": "preview",
                    "audio": false,
                    "duration_mode": "automatic"
                },
                "assets": [],
                "visual": {
                    "clips": [{
                        "id": "group",
                        "source": {
                            "type": "group",
                            "clips": [{
                                "id": "child",
                                "source": { "type": "solid_color", "colour": "#4C8CCC" },
                                "start": 0,
                                "duration": 1,
                                "layer": 0,
                                "opacity": { "base_value": 1 }
                            }]
                        },
                        "start": 0,
                        "duration": 1,
                        "layer": 0,
                        "opacity": { "base_value": 1 },
                        "effects": [{
                            "id": "lift",
                            "type": "brightness",
                            "amount": { "base_value": 0.1 }
                        }]
                    }]
                }
            }"##,
        )
        .expect("canonical Group JSON parses");
        let report = vestra_core::validation::validate(
            &project,
            vestra_core::validation::ResourceLimits::default(),
        );
        assert!(report.is_valid(), "{:?}", report.diagnostics());

        let asset_paths = std::collections::BTreeMap::new();
        let audio_durations = std::collections::BTreeMap::new();
        let warnings = Vec::new();
        let input = crate::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &asset_paths,
            &audio_durations,
            1.0,
            (24, 1),
            24,
            &warnings,
        );
        let plan = crate::plan::compile(&input, crate::plan::CompileOptions::default())
            .expect("canonical Group compiles");
        let frame = crate::plan::evaluate(&plan, &[crate::plan::ScheduledItem(0)], 0);
        let output = render_frame_with_plan(frame, &plan);
        assert_eq!(output.get_pixel(2, 2), &Rgba([102, 166, 230, 255]));
    }

    #[test]
    fn root_group_crossfade_renders_the_complete_group_layers_on_cpu() {
        let project = crate::project::Project::from_json(
            r##"{
                "schema_version": 3,
                "output": {
                    "path": "group-transition.mp4", "width": 2, "height": 2,
                    "frame_rate": "24/1", "background": "#00000000",
                    "quality": "preview", "audio": false,
                    "duration_mode": "automatic"
                },
                "assets": [],
                "visual": {
                    "clips": [
                        {"id": "red-group", "source": {"type": "group", "clips": [
                            {"id": "child", "source": {"type": "solid_color", "colour": "#FF0000"},
                             "start": 0, "duration": 2, "layer": 0, "opacity": {"base_value": 1}}
                        ]}, "start": 0, "duration": 2, "layer": 0, "opacity": {"base_value": 1}},
                        {"id": "blue-group", "source": {"type": "group", "clips": [
                            {"id": "child", "source": {"type": "solid_color", "colour": "#0000FF"},
                             "start": 0, "duration": 2, "layer": 1, "opacity": {"base_value": 1}}
                        ]}, "start": 0, "duration": 2, "layer": 1, "opacity": {"base_value": 1}}
                    ],
                    "transitions": [{"id": "fade", "outgoing": "red-group",
                        "incoming": "blue-group", "start": 0.5, "duration": 1.0,
                        "definition": {
                            "outgoing": {"opacity": {"keyframes": [
                                {"progress": 0.0, "value": 1.0, "interpolation": "linear"},
                                {"progress": 1.0, "value": 0.0, "interpolation": "linear"}
                            ]}},
                            "incoming": {"opacity": {"keyframes": [
                                {"progress": 0.0, "value": 0.0, "interpolation": "linear"},
                                {"progress": 1.0, "value": 1.0, "interpolation": "linear"}
                            ]}}
                        }}]
                }
            }"##,
        )
        .expect("Group transition JSON parses");
        let report = vestra_core::validation::validate(
            &project,
            vestra_core::validation::ResourceLimits::default(),
        );
        assert!(report.is_valid(), "{:?}", report.diagnostics());
        let assets = std::collections::BTreeMap::new();
        let durations = std::collections::BTreeMap::new();
        let warnings = Vec::new();
        let input = crate::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &assets,
            &durations,
            2.0,
            (24, 1),
            48,
            &warnings,
        );
        let plan = crate::plan::compile(&input, crate::plan::CompileOptions::default())
            .expect("Group transition compiles");
        let frame = crate::plan::evaluate(
            &plan,
            &[crate::plan::ScheduledItem(0), crate::plan::ScheduledItem(1)],
            1_000_000_000,
        );
        let output = render_frame_with_plan(frame, &plan);
        let pixel = output.get_pixel(1, 1);
        assert!(pixel[0] > 0 && pixel[2] > 0 && pixel[1] == 0);
    }

    #[test]
    fn alpha_composition_is_known() {
        assert_eq!(
            source_over(Rgba([0, 0, 255, 255]), Rgba([255, 0, 0, 128]), 1.0),
            Rgba([128, 0, 127, 255])
        );
    }
    #[test]
    fn bilinear_sampling_blends_four_neighbors() {
        let mut image = RgbaImage::new(2, 2);
        image.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
        image.put_pixel(1, 0, Rgba([100, 0, 0, 255]));
        image.put_pixel(0, 1, Rgba([0, 100, 0, 255]));
        image.put_pixel(1, 1, Rgba([100, 100, 0, 255]));
        assert_eq!(sample_bilinear(&image, 1.0, 1.0), Rgba([50, 50, 0, 255]));
    }

    #[test]
    fn directional_blur_keeps_transparent_edges_coloured() {
        let mut source = RgbaImage::from_pixel(7, 1, Rgba([0, 0, 255, 0]));
        source.put_pixel(3, 0, Rgba([255, 128, 32, 255]));
        let mut target = RgbaImage::new(7, 1);
        blur(&source, &mut target, 2.0, Some(0.0), Some(5));
        let edge = target.get_pixel(2, 0);
        assert!(edge[3] > 0);
        assert!(edge[0] > edge[2]);
    }

    #[test]
    fn directional_blur_matches_the_transparent_pixel_golden_fixture() {
        let mut source = RgbaImage::new(5, 1);
        source.put_pixel(2, 0, Rgba([255, 80, 20, 192]));
        let mut output = RgbaImage::new(5, 1);
        blur(&source, &mut output, 1.0, Some(0.0), Some(3));
        assert_eq!(
            output.as_raw(),
            &[
                0, 0, 0, 0, 255, 80, 20, 64, 255, 80, 20, 64, 255, 80, 20, 64, 0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn zoom_blur_streaks_along_the_ray_from_the_anchor() {
        let mut source = RgbaImage::new(9, 9);
        source.put_pixel(7, 4, Rgba([255, 255, 255, 255]));
        let mut target = RgbaImage::new(9, 9);
        zoom_blur(
            &source,
            &mut target,
            4.0,
            12,
            crate::domain::Point { x: 0.5, y: 0.5 },
            crate::project::ZoomBlurDirection::Centered,
        );
        assert!(target.get_pixel(6, 4)[3] > target.get_pixel(7, 3)[3]);
    }

    #[test]
    fn vignette_normalizes_each_frame_axis_independently() {
        let source = RgbaImage::from_pixel(10, 100, Rgba([255, 255, 255, 255]));
        let mut target = RgbaImage::new(10, 100);
        crate::cpu::vignette::apply(&source, &mut target, 1.0, 0.0, 1.0, [0, 0, 0, 255]);
        let top = target.get_pixel(5, 0)[0];
        let side = target.get_pixel(0, 50)[0];
        assert!(top.abs_diff(side) <= 32);
        assert!(target.get_pixel(5, 50)[0] > top);
    }

    #[test]
    fn vertical_vignette_matches_the_pixel_golden_fixture() {
        let source = RgbaImage::from_pixel(3, 5, Rgba([200, 160, 120, 255]));
        let mut target = RgbaImage::new(3, 5);
        crate::cpu::vignette::apply(&source, &mut target, 0.8, 0.25, 0.5, [0, 0, 0, 255]);
        assert_eq!(
            target.as_raw(),
            &[
                40, 32, 24, 255, 40, 32, 24, 255, 40, 32, 24, 255, 40, 32, 24, 255, 152, 122, 91,
                255, 40, 32, 24, 255, 67, 53, 40, 255, 200, 160, 120, 255, 67, 53, 40, 255, 40, 32,
                24, 255, 152, 122, 91, 255, 40, 32, 24, 255, 40, 32, 24, 255, 40, 32, 24, 255, 40,
                32, 24, 255,
            ]
        );
    }

    #[test]
    fn basic_color_effects_apply_in_declared_order() {
        let transform = ColourTransform::from_effects([
            EvaluatedEffect::Brightness { amount: 0.1 },
            EvaluatedEffect::Tint {
                colour: [0, 0, 255, 255],
                amount: 0.5,
            },
        ]);
        assert_eq!(
            apply_colour_transform(Rgba([100, 0, 0, 255]), transform),
            Rgba([63, 13, 140, 255])
        );
    }

    #[test]
    fn basic_colour_effects_keep_the_direct_render_path() {
        let base = EvaluatedLayer {
            compiled_layer_index: 0,
            content_dependency: TemporalDependency::Dynamic,
            source: EvaluatedSource::SolidColor {
                colour: [0, 0, 0, 255],
            },
            transform: identity_transform(),
            opacity: 1.0,
            effects: vec![EvaluatedEffect::Brightness { amount: 0.1 }],
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        };
        assert!(uses_direct_colour_path(&base));
        let mut advanced = base;
        advanced.effects = vec![EvaluatedEffect::GaussianBlur { radius: 1.0 }];
        assert!(!uses_direct_colour_path(&advanced));
        advanced.effects.clear();
        advanced.blend_mode = crate::project::BlendMode::Screen;
        assert!(!uses_direct_colour_path(&advanced));
    }

    #[test]
    fn particle_sources_enter_the_effect_capable_compositor_path() {
        let mut particle_layer = layer(1.0, crate::project::BlendMode::Normal);
        particle_layer.content_dependency = TemporalDependency::Dynamic;
        particle_layer.source = EvaluatedSource::ParticleSystem {
            system: std::sync::Arc::new(crate::plan::CompiledParticleSystem {
                seed: 0,
                emitter: crate::project::ParticleEmitter::default(),
                rate_units_per_second: 0,
                lifetime_nanos: 1,
                lifetime_range: crate::project::ScalarRange { min: 1.0, max: 1.0 },
                initial_velocity: crate::domain::Point { x: 0.0, y: 0.0 },
                speed: crate::project::ScalarRange { min: 0.0, max: 0.0 },
                direction_degrees: 0.0,
                direction_spread_degrees: 0.0,
                acceleration: crate::domain::Point { x: 0.0, y: 0.0 },
                size: 0.0,
                size_range: None,
                opacity: 0.0,
                colour: [0; 4],
                rotation_degrees: 0.0,
                rotation_range: None,
                angular_velocity_degrees: 0.0,
                angular_velocity_range: None,
                primitive: crate::project::ParticlePrimitive::Disc,
                blend_mode: crate::project::ParticleBlendMode::Normal,
                bursts: Vec::new(),
                maximum_live_particles: 0,
                lifetime_size: None,
                lifetime_opacity: None,
                lifetime_colour: None,
                audio_size: None,
                audio_opacity: None,
                audio_intensity: None,
            }),
            time_nanos: 0,
            appearance: crate::plan::EvaluatedParticleAppearance::default(),
        };
        particle_layer.effects = vec![EvaluatedEffect::GaussianBlur { radius: 1.0 }];
        assert!(!uses_direct_colour_path(&particle_layer));
    }

    #[test]
    fn particle_pixels_pass_through_the_cpu_effect_chain() {
        let system = crate::plan::CompiledParticleSystem {
            seed: 0,
            emitter: crate::project::ParticleEmitter::default(),
            rate_units_per_second: 0,
            lifetime_nanos: 1_000_000_000,
            lifetime_range: crate::project::ScalarRange { min: 1.0, max: 1.0 },
            initial_velocity: crate::domain::Point { x: 0.0, y: 0.0 },
            speed: crate::project::ScalarRange { min: 0.0, max: 0.0 },
            direction_degrees: 0.0,
            direction_spread_degrees: 0.0,
            acceleration: crate::domain::Point { x: 0.0, y: 0.0 },
            size: 0.5,
            size_range: None,
            opacity: 1.0,
            colour: [100, 0, 0, 255],
            rotation_degrees: 0.0,
            rotation_range: None,
            angular_velocity_degrees: 0.0,
            angular_velocity_range: None,
            primitive: crate::project::ParticlePrimitive::Square,
            blend_mode: crate::project::ParticleBlendMode::Normal,
            bursts: vec![crate::plan::CompiledParticleBurst {
                time_nanos: 0,
                count: 1,
            }],
            maximum_live_particles: 1,
            lifetime_size: None,
            lifetime_opacity: None,
            lifetime_colour: None,
            audio_size: None,
            audio_opacity: None,
            audio_intensity: None,
        };
        let mut surfaces = EffectSurfacePool::new(4, 4);
        surfaces.clear();
        crate::cpu::particles::rasterize(
            surfaces.current(),
            &system,
            0,
            ColourTransform::default(),
        );
        let before = surfaces.current().clone();
        effects::apply_chain(
            &mut surfaces,
            &[EvaluatedEffect::Brightness { amount: 0.2 }],
            &mut crate::render::metrics::CpuHotPathTimings::default(),
            false,
        );
        assert_ne!(surfaces.current().get_pixel(2, 2), before.get_pixel(2, 2));
    }

    #[test]
    fn saturation_zero_produces_neutral_channels() {
        let pixel = apply_colour_transform(
            Rgba([255, 0, 0, 255]),
            ColourTransform::from_effects([EvaluatedEffect::Saturation { amount: 0.0 }]),
        );
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
    }

    #[test]
    fn contrast_one_is_identity() {
        assert_eq!(
            apply_colour_transform(
                Rgba([30, 140, 250, 180]),
                ColourTransform::from_effects([EvaluatedEffect::Contrast { amount: 1.0 }])
            ),
            Rgba([30, 140, 250, 180])
        );
    }

    #[test]
    fn evaluated_effect_identity_plan_skips_only_identity_work() {
        assert!(effect_pass_plan(&EvaluatedEffect::GaussianBlur { radius: 0.0 }).is_empty());
        assert!(
            effect_pass_plan(&EvaluatedEffect::ColorAdjust {
                exposure: 0.0,
                gamma: 1.0,
                black_point: 0.0,
                white_point: 1.0,
            })
            .is_empty()
        );
        assert!(
            effect_pass_plan(&EvaluatedEffect::CameraShake {
                local_time: 0,
                position_amount: 1.0,
                rotation_radians: 1.0,
                scale_amount: 1.0,
                frequency: 1.0,
                seed: 1,
                attack: 0.0,
                decay: 0.0,
            })
            .is_empty()
        );
        assert!(
            !effect_pass_plan(&EvaluatedEffect::Glow {
                threshold: 0.5,
                radius: 2.0,
                intensity: 1.0,
                colour: [255, 255, 255, 255],
            })
            .is_empty()
        );
    }

    #[test]
    fn combined_colour_matrix_matches_ordered_sequential_effects() {
        let effects = [
            EvaluatedEffect::Brightness { amount: -0.12 },
            EvaluatedEffect::Saturation { amount: 0.55 },
            EvaluatedEffect::Contrast { amount: 1.15 },
            EvaluatedEffect::Tint {
                colour: [30, 120, 240, 255],
                amount: 0.3,
            },
        ];
        let input = Rgba([180, 80, 40, 173]);
        let expected = apply_sequential(
            [
                f64::from(input[0]),
                f64::from(input[1]),
                f64::from(input[2]),
            ],
            &effects,
        )
        .map(|channel| channel.round().clamp(0.0, 255.0) as u8);
        let actual = apply_colour_transform(input, ColourTransform::from_effects(effects));
        for channel in 0..3 {
            assert!(actual[channel].abs_diff(expected[channel]) <= 1);
        }
        assert_eq!(actual[3], input[3]);
    }

    #[test]
    fn identity_colour_matrix_leaves_pixels_unchanged() {
        let pixel = Rgba([31, 127, 249, 90]);
        assert_eq!(
            apply_colour_transform(pixel, ColourTransform::default()),
            pixel
        );
    }

    #[test]
    fn inverse_affine_matches_transform_reference_mapping() {
        let transform = Transform2D {
            position: crate::domain::Point { x: 0.37, y: 0.61 },
            anchor: crate::domain::Point { x: 0.4, y: 0.7 },
            scale: crate::domain::Point { x: 1.3, y: 0.8 },
            rotation_radians: 0.42,
        };
        let inverse =
            geometry::InverseAffine::for_transform(transform, 320, 180, 140.0, 90.0, 0.0, 0.0);
        let mapped = inverse.map(81.5, 44.5);
        let reference = transform.destination_to_source(81.5, 44.5, 320, 180, 140, 90);
        assert!((mapped.x - reference.x).abs() < 1e-10 && (mapped.y - reference.y).abs() < 1e-10);
    }

    #[test]
    fn blend_modes_are_distinct_and_preserve_alpha() {
        let destination = Rgba([40, 100, 200, 255]);
        let source = Rgba([200, 80, 20, 255]);
        let add = blend_pixel(destination, source, crate::project::BlendMode::Add, 1.0);
        let multiply = blend_pixel(
            destination,
            source,
            crate::project::BlendMode::Multiply,
            1.0,
        );
        let screen = blend_pixel(destination, source, crate::project::BlendMode::Screen, 1.0);
        let overlay = blend_pixel(destination, source, crate::project::BlendMode::Overlay, 1.0);
        assert_eq!(add[3], 255);
        assert_eq!(multiply[3], 255);
        assert_ne!(add, multiply);
        assert_ne!(screen, overlay);
    }

    #[test]
    fn zero_radius_blur_is_an_exact_noop() {
        let mut source = RgbaImage::new(2, 1);
        source.put_pixel(0, 0, Rgba([255, 0, 0, 127]));
        source.put_pixel(1, 0, Rgba([0, 0, 255, 255]));
        let mut target = RgbaImage::new(2, 1);
        blur(&source, &mut target, 0.0, None, None);
        assert_eq!(source, target);
    }

    #[test]
    fn chromatic_zero_amount_is_an_exact_noop() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([17, 83, 201, 129]));
        let mut target = RgbaImage::new(2, 2);
        crate::cpu::chromatic::apply(&source, &mut target, 0.0, 0.0);
        assert_eq!(source, target);
    }

    #[test]
    fn rotated_scanline_advances_after_an_out_of_bounds_sample() {
        let source = RgbaImage::from_pixel(4, 2, Rgba([255, 0, 0, 255]));
        let transform = Transform2D {
            position: crate::domain::Point { x: 0.5, y: 0.5 },
            anchor: crate::domain::Point { x: 0.5, y: 0.5 },
            scale: crate::domain::Point { x: 1.0, y: 1.0 },
            rotation_radians: 0.7,
        };
        let (min_x, max_x, min_y, max_y) = visible_bounds(transform, 4.0, 2.0, 12, 12);
        let inverse = geometry::InverseAffine::for_transform(transform, 12, 12, 4.0, 2.0, 0.0, 0.0);
        let entering_row = (min_y..max_y)
            .find(|y| {
                let first = inverse.map(f64::from(min_x) + 0.5, f64::from(*y) + 0.5);
                let later_is_valid = (min_x + 1..max_x).any(|x| {
                    let mapped = inverse.map(f64::from(x) + 0.5, f64::from(*y) + 0.5);
                    mapped.x >= 0.0 && mapped.y >= 0.0 && mapped.x < 4.0 && mapped.y < 2.0
                });
                (first.x < 0.0 || first.y < 0.0 || first.x >= 4.0 || first.y >= 2.0)
                    && later_is_valid
            })
            .expect("rotation has a scanline that enters the source");
        let mut canvas = RgbaImage::new(12, 12);
        draw_image(
            &mut canvas,
            &source,
            Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            4.0,
            2.0,
            transform,
            1.0,
            ColourTransform::default(),
        );
        assert!(
            (min_x..max_x).any(|x| canvas.get_pixel(x, entering_row)[3] > 0),
            "the scanline must render after its mapping enters the rotated source"
        );
    }

    #[test]
    fn positive_and_negative_rotations_keep_a_non_square_source_visible() {
        let source = RgbaImage::from_fn(5, 3, |x, y| Rgba([x as u8 * 40, y as u8 * 80, 255, 255]));
        for rotation_radians in [0.65, -0.65] {
            let mut canvas = RgbaImage::new(16, 16);
            draw_image(
                &mut canvas,
                &source,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                5.0,
                3.0,
                Transform2D {
                    position: crate::domain::Point { x: 0.5, y: 0.5 },
                    anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                    scale: crate::domain::Point { x: 1.0, y: 1.0 },
                    rotation_radians,
                },
                1.0,
                ColourTransform::default(),
            );
            assert!(canvas.pixels().any(|pixel| pixel[3] > 0));
        }
    }

    #[test]
    fn scale_expands_coverage_around_the_anchor() {
        let source = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        let coverage = |scale| {
            let mut canvas = RgbaImage::new(12, 12);
            draw_image(
                &mut canvas,
                &source,
                Crop {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
                2.0,
                2.0,
                Transform2D {
                    position: crate::domain::Point { x: 0.5, y: 0.5 },
                    anchor: crate::domain::Point { x: 0.5, y: 0.5 },
                    scale: crate::domain::Point { x: scale, y: scale },
                    rotation_radians: 0.0,
                },
                1.0,
                ColourTransform::default(),
            );
            canvas.pixels().filter(|pixel| pixel[3] > 0).count()
        };
        assert!(coverage(2.0) > coverage(1.0));
    }

    #[test]
    fn opacity_animation_uses_source_over_alpha() {
        let destination = Rgba([0, 0, 255, 255]);
        let source = Rgba([255, 0, 0, 255]);
        assert_eq!(source_over(destination, source, 0.0), destination);
        assert_eq!(source_over(destination, source, 1.0), source);
        assert_eq!(
            source_over(destination, source, 0.5),
            Rgba([128, 0, 128, 255])
        );
    }
}
