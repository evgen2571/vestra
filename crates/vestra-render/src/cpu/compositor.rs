use std::{collections::HashMap, sync::Arc, time::Instant};

use image::{GenericImage, Rgba, RgbaImage};

use crate::{
    blend::blend_surface,
    cpu::{assets::PreparedAssets, effects, raster::draw_layer},
    render::{ByteLruCache, metrics::CpuHotPathTimings},
};
use vestra_core::plan::{
    ColourTransform, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedMaskInput,
    EvaluatedSource, TemporalDependency,
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

/// Composites an immutable, backend-neutral frame program into a reusable buffer.
#[allow(clippy::too_many_arguments)]
pub(super) fn compose(
    frame: &EvaluatedFrame,
    assets: &mut PreparedAssets,
    canvas: &mut RgbaImage,
    surfaces: &mut EffectSurfacePool,
    compositions: &mut CompositionSurfacePool,
    static_layers: &mut ByteLruCache<usize, Arc<CachedCpuLayerSurface>>,
    timings: &mut CpuHotPathTimings,
    profiling_enabled: bool,
) -> ComposeStats {
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

    let mut context = ComposeContext {
        assets,
        surfaces,
        compositions,
        static_layers,
        timings,
        profiling_enabled,
        matte_cache: HashMap::new(),
        stats: ComposeStats::default(),
    };
    context.compose_layers(&frame.layers, frame.width, frame.height, canvas, 0);
    effects::apply_to(
        context.surfaces,
        canvas,
        &frame.post_effects,
        context.timings,
        context.profiling_enabled,
        context.assets.glyph_atlases(),
    );
    context.stats
}

struct ComposeContext<'a> {
    assets: &'a mut PreparedAssets,
    surfaces: &'a mut EffectSurfacePool,
    compositions: &'a mut CompositionSurfacePool,
    static_layers: &'a mut ByteLruCache<usize, Arc<CachedCpuLayerSurface>>,
    timings: &'a mut CpuHotPathTimings,
    profiling_enabled: bool,
    matte_cache: HashMap<usize, RgbaImage>,
    stats: ComposeStats,
}

impl ComposeContext<'_> {
    fn compose_layers(
        &mut self,
        layers: &[EvaluatedLayer],
        width: u32,
        height: u32,
        canvas: &mut RgbaImage,
        depth: usize,
    ) {
        for layer in layers {
            if !layer.visible {
                continue;
            }
            if let EvaluatedSource::Group { composition } = &layer.source {
                self.render_group(layer, composition, layers, canvas, depth);
                continue;
            }

            if layer.content_dependency == TemporalDependency::Static {
                if let Some(cached) = self.static_layers.get(&layer.compiled_layer_index).cloned() {
                    composite_cached_surface(
                        canvas,
                        &cached,
                        layer,
                        &mut self.stats,
                        self.profiling_enabled
                            .then_some(&mut self.timings.composition_cases),
                    );
                    continue;
                }
                self.surfaces.clear();
                self.stats.static_layer_renders += 1;
                if uses_direct_colour_path(layer) {
                    let started = self.profiling_enabled.then(Instant::now);
                    draw_layer(
                        self.surfaces.current(),
                        self.assets,
                        layer,
                        1.0,
                        layer.colour_transform,
                        self.timings,
                        self.profiling_enabled,
                    );
                    if let Some(started) = started {
                        self.timings.source_rasterization += started.elapsed();
                    }
                } else {
                    let started = self.profiling_enabled.then(Instant::now);
                    draw_layer(
                        self.surfaces.current(),
                        self.assets,
                        layer,
                        1.0,
                        ColourTransform::default(),
                        self.timings,
                        self.profiling_enabled,
                    );
                    if let Some(started) = started {
                        self.timings.source_rasterization += started.elapsed();
                    }
                    effects::apply_chain(
                        self.surfaces,
                        &layer.effects,
                        self.timings,
                        self.profiling_enabled,
                        self.assets.glyph_atlases(),
                    );
                }
                self.apply_masks(layer, depth);
                self.apply_track_matte(layer, layers, depth);
                let bytes = u64::from(width) * u64::from(height) * 4;
                let cached =
                    self.static_layers
                        .insert_with(layer.compiled_layer_index, bytes, || {
                            Arc::new(CachedCpuLayerSurface::from_image(
                                self.surfaces.take_current(),
                            ))
                        });
                if let Some(cached) = cached {
                    composite_cached_surface(
                        canvas,
                        cached,
                        layer,
                        &mut self.stats,
                        self.profiling_enabled
                            .then_some(&mut self.timings.composition_cases),
                    );
                } else {
                    let started = self.profiling_enabled.then(Instant::now);
                    blend_surface(
                        canvas,
                        self.surfaces.current(),
                        layer.blend_mode,
                        layer.opacity,
                        self.profiling_enabled
                            .then_some(&mut self.timings.composition_cases),
                    );
                    if let Some(started) = started {
                        self.timings.layer_composition += started.elapsed();
                    }
                    self.stats.generic_blend_surface_calls += 1;
                }
                continue;
            }
            if uses_direct_colour_path(layer) {
                let started = self.profiling_enabled.then(Instant::now);
                draw_layer(
                    canvas,
                    self.assets,
                    layer,
                    layer.opacity,
                    layer.colour_transform,
                    self.timings,
                    self.profiling_enabled,
                );
                if let Some(started) = started {
                    self.timings.source_rasterization += started.elapsed();
                }
                continue;
            }
            self.surfaces.clear();
            let started = self.profiling_enabled.then(Instant::now);
            draw_layer(
                self.surfaces.current(),
                self.assets,
                layer,
                1.0,
                ColourTransform::default(),
                self.timings,
                self.profiling_enabled,
            );
            if let Some(started) = started {
                self.timings.source_rasterization += started.elapsed();
            }
            effects::apply_chain(
                self.surfaces,
                &layer.effects,
                self.timings,
                self.profiling_enabled,
                self.assets.glyph_atlases(),
            );
            self.apply_masks(layer, depth);
            self.apply_track_matte(layer, layers, depth);
            let started = self.profiling_enabled.then(Instant::now);
            blend_surface(
                canvas,
                self.surfaces.current(),
                layer.blend_mode,
                layer.opacity,
                self.profiling_enabled
                    .then_some(&mut self.timings.composition_cases),
            );
            if let Some(started) = started {
                self.timings.layer_composition += started.elapsed();
            }
            self.stats.generic_blend_surface_calls += 1;
        }
    }

    fn render_group(
        &mut self,
        layer: &EvaluatedLayer,
        composition: &vestra_core::plan::EvaluatedComposition,
        scope_layers: &[EvaluatedLayer],
        parent: &mut RgbaImage,
        depth: usize,
    ) {
        if layer.content_dependency == TemporalDependency::Static {
            if let Some(cached) = self.static_layers.get(&layer.compiled_layer_index).cloned() {
                composite_cached_surface(
                    parent,
                    &cached,
                    layer,
                    &mut self.stats,
                    self.profiling_enabled
                        .then_some(&mut self.timings.composition_cases),
                );
                return;
            }
            self.stats.static_layer_renders += 1;
        }

        let width = parent.width();
        let height = parent.height();
        let transform = layer.transform;
        let mut group_surface = self.compositions.acquire(depth, width, height);
        self.compose_layers(
            &composition.layers,
            width,
            height,
            &mut group_surface,
            depth + 1,
        );

        // A Group becomes image-like only after all children have been composed.
        // The effect pool is separate so the live composition surface remains
        // available while nested composition is unwound.
        self.surfaces.clear();
        let direct_colour_path = uses_direct_colour_path(layer);
        let motion_tile = layer
            .effects
            .iter()
            .find_map(crate::cpu::raster::motion_tile_parameters);
        let started = self.profiling_enabled.then(Instant::now);
        crate::cpu::raster::draw_surface_with_motion_tile(
            self.surfaces.current(),
            &group_surface,
            transform,
            if direct_colour_path {
                layer.colour_transform
            } else {
                ColourTransform::default()
            },
            motion_tile,
        );
        if let Some(started) = started {
            let elapsed = started.elapsed();
            self.timings.transform_sampling += elapsed;
            if motion_tile.is_some() {
                self.timings.motion_tile += elapsed;
            }
        }
        if !direct_colour_path {
            effects::apply_chain(
                self.surfaces,
                &layer.effects,
                self.timings,
                self.profiling_enabled,
                self.assets.glyph_atlases(),
            );
        }
        self.apply_masks(layer, depth);
        self.apply_track_matte(layer, scope_layers, depth);
        if layer.content_dependency == TemporalDependency::Static {
            let bytes = u64::from(width) * u64::from(height) * 4;
            let cached = self
                .static_layers
                .insert_with(layer.compiled_layer_index, bytes, || {
                    Arc::new(CachedCpuLayerSurface::from_image(
                        self.surfaces.take_current(),
                    ))
                });
            if let Some(cached) = cached {
                composite_cached_surface(
                    parent,
                    cached,
                    layer,
                    &mut self.stats,
                    self.profiling_enabled
                        .then_some(&mut self.timings.composition_cases),
                );
            } else {
                let started = self.profiling_enabled.then(Instant::now);
                blend_surface(
                    parent,
                    self.surfaces.current(),
                    layer.blend_mode,
                    layer.opacity,
                    self.profiling_enabled
                        .then_some(&mut self.timings.composition_cases),
                );
                if let Some(started) = started {
                    self.timings.layer_composition += started.elapsed();
                }
                self.stats.generic_blend_surface_calls += 1;
            }
        } else {
            let started = self.profiling_enabled.then(Instant::now);
            blend_surface(
                parent,
                self.surfaces.current(),
                layer.blend_mode,
                layer.opacity,
                self.profiling_enabled
                    .then_some(&mut self.timings.composition_cases),
            );
            if let Some(started) = started {
                self.timings.layer_composition += started.elapsed();
            }
            self.stats.generic_blend_surface_calls += 1;
        }
        self.compositions.release(depth, group_surface);
    }
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
        && layer.masks.is_empty()
        && layer.matte.is_none()
        && !matches!(layer.source, EvaluatedSource::ParticleSystem { .. })
        && layer
            .effects
            .iter()
            .all(EvaluatedEffect::is_basic_colour_effect)
}

impl ComposeContext<'_> {
    fn apply_track_matte(
        &mut self,
        layer: &EvaluatedLayer,
        layers: &[EvaluatedLayer],
        depth: usize,
    ) {
        let Some(matte) = &layer.matte else { return };
        let mode = match matte.mode {
            crate::project::MatteMode::Alpha => crate::project::MaskCoverageMode::Alpha,
            crate::project::MatteMode::Luma => crate::project::MaskCoverageMode::Luma,
        };
        let Some(source) = layers
            .iter()
            .find(|candidate| candidate.compiled_layer_index == matte.source_layer_identity)
        else {
            for pixel in self.surfaces.mask_local_surface().pixels_mut() {
                *pixel = Rgba([0, 0, 0, 0]);
            }
            self.surfaces.apply_external_matte(mode, matte.invert);
            return;
        };
        let width = self.surfaces.current().width();
        let height = self.surfaces.current().height();
        let mut saved_consumer = self.compositions.acquire(depth + 1, width, height);
        saved_consumer
            .copy_from(self.surfaces.current(), 0, 0)
            .expect("matching matte save surface dimensions");
        if let Some(cached) = self.matte_cache.get(&source.compiled_layer_index) {
            self.surfaces
                .mask_local_surface()
                .copy_from(cached, 0, 0)
                .expect("matching cached matte surface dimensions");
        } else {
            self.render_isolated_layer(source, layers, depth + 1);
            self.matte_cache.insert(
                source.compiled_layer_index,
                self.surfaces.mask_local_surface().clone(),
            );
        }
        self.surfaces
            .current()
            .copy_from(&saved_consumer, 0, 0)
            .expect("matching matte restore surface dimensions");
        self.compositions.release(depth + 1, saved_consumer);
        self.surfaces.apply_external_matte(mode, matte.invert);
    }

    fn render_isolated_layer(
        &mut self,
        layer: &EvaluatedLayer,
        scope_layers: &[EvaluatedLayer],
        depth: usize,
    ) {
        self.surfaces.clear();
        if let EvaluatedSource::Group { composition } = &layer.source {
            let width = self.surfaces.current().width();
            let height = self.surfaces.current().height();
            let mut group_surface = self.compositions.acquire(depth + 1, width, height);
            self.compose_layers(
                &composition.layers,
                width,
                height,
                &mut group_surface,
                depth + 2,
            );
            // Child effects leave pixels in scratch; transform only the completed group.
            self.surfaces.clear();
            let motion_tile = layer
                .effects
                .iter()
                .find_map(crate::cpu::raster::motion_tile_parameters);
            let started = self.profiling_enabled.then(Instant::now);
            crate::cpu::raster::draw_surface_with_motion_tile(
                self.surfaces.current(),
                &group_surface,
                layer.transform,
                if uses_direct_colour_path(layer) {
                    layer.colour_transform
                } else {
                    ColourTransform::default()
                },
                motion_tile,
            );
            if let Some(started) = started {
                let elapsed = started.elapsed();
                self.timings.transform_sampling += elapsed;
                if motion_tile.is_some() {
                    self.timings.motion_tile += elapsed;
                }
            }
            self.compositions.release(depth + 1, group_surface);
        } else {
            draw_layer(
                self.surfaces.current(),
                self.assets,
                layer,
                1.0,
                if uses_direct_colour_path(layer) {
                    layer.colour_transform
                } else {
                    ColourTransform::default()
                },
                self.timings,
                self.profiling_enabled,
            );
        }
        if !uses_direct_colour_path(layer) {
            effects::apply_chain(
                self.surfaces,
                &layer.effects,
                self.timings,
                self.profiling_enabled,
                self.assets.glyph_atlases(),
            );
        }
        self.apply_masks(layer, depth);
        self.apply_track_matte(layer, scope_layers, depth);
        if layer.opacity != 1.0 {
            for pixel in self.surfaces.current().pixels_mut() {
                pixel[3] = (f64::from(pixel[3]) * layer.opacity)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
        self.surfaces.copy_current_to_mask_local();
    }

    fn apply_masks(&mut self, layer: &EvaluatedLayer, depth: usize) {
        if layer.masks.is_empty() {
            return;
        }
        let started = self.profiling_enabled.then(Instant::now);
        self.surfaces.reset_mask_coverage();
        for mask in &layer.masks {
            for pixel in self.surfaces.mask_local_surface().pixels_mut() {
                *pixel = Rgba([0, 0, 0, 0]);
            }
            match mask.input.clone() {
                EvaluatedMaskInput::Shape { shape_index } => {
                    let mask_layer = EvaluatedLayer {
                        compiled_layer_index: usize::MAX,
                        visible: true,
                        content_dependency: TemporalDependency::Static,
                        source: EvaluatedSource::Shape {
                            shape_index,
                            sizing: vestra_core::plan::CompiledSizing::Original,
                        },
                        transform: mask.transform,
                        opacity: 1.0,
                        effects: Vec::new(),
                        masks: Vec::new(),
                        matte: None,
                        colour_transform: ColourTransform::default(),
                        blend_mode: crate::project::BlendMode::Normal,
                    };
                    super::raster::draw_layer(
                        self.surfaces.mask_local_surface(),
                        self.assets,
                        &mask_layer,
                        1.0,
                        ColourTransform::default(),
                        self.timings,
                        self.profiling_enabled,
                    );
                }
                EvaluatedMaskInput::Image { asset_index, mode } => {
                    let prepared = self.assets.raster_source(asset_index);
                    super::raster::draw_raster(
                        self.surfaces.mask_local_surface(),
                        prepared.pixels(),
                        prepared.intrinsic_size(),
                        crate::domain::Crop {
                            x: 0.0,
                            y: 0.0,
                            width: 1.0,
                            height: 1.0,
                        },
                        false,
                        &vestra_core::plan::CompiledSizing::Original,
                        mask.transform,
                        1.0,
                        ColourTransform::default(),
                    );
                    for pixel in self.surfaces.mask_local_surface().pixels_mut() {
                        let coverage = crate::project::mask_coverage(pixel.0, mode);
                        pixel[3] = (coverage * 255.0).round().clamp(0.0, 255.0) as u8;
                    }
                }
                EvaluatedMaskInput::Source { source, mode } => {
                    let mask_layer = EvaluatedLayer {
                        compiled_layer_index: usize::MAX,
                        visible: true,
                        content_dependency: TemporalDependency::Dynamic,
                        source: (*source).clone(),
                        transform: mask.transform,
                        opacity: 1.0,
                        effects: Vec::new(),
                        masks: Vec::new(),
                        matte: None,
                        colour_transform: ColourTransform::default(),
                        blend_mode: crate::project::BlendMode::Normal,
                    };
                    if let EvaluatedSource::Group { composition } = &mask_layer.source {
                        let owner_surface = self.surfaces.current().clone();
                        let mut group_surface = RgbaImage::new(
                            self.surfaces.mask_local_surface().width(),
                            self.surfaces.mask_local_surface().height(),
                        );
                        for pixel in group_surface.pixels_mut() {
                            *pixel = Rgba([0, 0, 0, 0]);
                        }
                        self.compose_layers(
                            &composition.layers,
                            group_surface.width(),
                            group_surface.height(),
                            &mut group_surface,
                            depth + 1,
                        );
                        crate::cpu::raster::draw_surface(
                            self.surfaces.mask_local_surface(),
                            &group_surface,
                            mask.transform,
                            ColourTransform::default(),
                        );
                        self.surfaces.begin_from(&owner_surface);
                    } else {
                        super::raster::draw_layer(
                            self.surfaces.mask_local_surface(),
                            self.assets,
                            &mask_layer,
                            1.0,
                            ColourTransform::default(),
                            self.timings,
                            self.profiling_enabled,
                        );
                    }
                    for pixel in self.surfaces.mask_local_surface().pixels_mut() {
                        pixel[3] = (crate::project::mask_coverage(pixel.0, mode) * 255.0)
                            .round()
                            .clamp(0.0, 255.0) as u8;
                    }
                }
            }
            self.surfaces.compose_mask_surface(layer.transform);
            self.surfaces.feather_mask_surface(mask.feather);
            self.surfaces
                .combine_mask_coverage(mask.operation, mask.invert, mask.strength);
        }
        self.surfaces.apply_mask_coverage();
        if let Some(started) = started {
            self.timings.transform_sampling += started.elapsed();
        }
    }
}

#[cfg(test)]
mod tests;
