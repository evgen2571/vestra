//! CPU worker profiling, kept with execution.

use std::time::Duration;

/// Coarse CPU execution timings collected by each renderer worker.
/// This is emitted only by the opt-in benchmark profile path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CpuHotPathTimings {
    pub(crate) source_rasterization: Duration,
    pub(crate) transform_sampling: Duration,
    pub(crate) layer_composition: Duration,
    pub(crate) effect_execution: Duration,
    pub(crate) motion_tile: Duration,
    pub(crate) gaussian_blur: Duration,
    pub(crate) zoom_blur: Duration,
    pub(crate) radial_blur: Duration,
    pub(crate) bloom_glow: Duration,
    pub(crate) bloom_highlight_extract: Duration,
    pub(crate) bloom_gaussian_blur: Duration,
    pub(crate) bloom_composite: Duration,
    pub(crate) chromatic_aberration: Duration,
    pub(crate) vignette: Duration,
    pub(crate) color_adjust: Duration,
    pub(crate) sharpen: Duration,
    pub(crate) sharpen_gaussian: Duration,
    pub(crate) sharpen_unsharp_composite: Duration,
    pub(crate) other_effects: Duration,
    pub(crate) global_post_effect: Duration,
    pub(crate) surface_copy: Duration,
    pub(crate) composition_cases: super::blend::CompositionCaseCounts,
}

impl CpuHotPathTimings {
    pub(crate) fn add_assign(&mut self, other: Self) {
        self.source_rasterization += other.source_rasterization;
        self.transform_sampling += other.transform_sampling;
        self.layer_composition += other.layer_composition;
        self.effect_execution += other.effect_execution;
        self.motion_tile += other.motion_tile;
        self.gaussian_blur += other.gaussian_blur;
        self.zoom_blur += other.zoom_blur;
        self.radial_blur += other.radial_blur;
        self.bloom_glow += other.bloom_glow;
        self.bloom_highlight_extract += other.bloom_highlight_extract;
        self.bloom_gaussian_blur += other.bloom_gaussian_blur;
        self.bloom_composite += other.bloom_composite;
        self.chromatic_aberration += other.chromatic_aberration;
        self.vignette += other.vignette;
        self.color_adjust += other.color_adjust;
        self.sharpen += other.sharpen;
        self.sharpen_gaussian += other.sharpen_gaussian;
        self.sharpen_unsharp_composite += other.sharpen_unsharp_composite;
        self.other_effects += other.other_effects;
        self.global_post_effect += other.global_post_effect;
        self.surface_copy += other.surface_copy;
        self.composition_cases.source_alpha_zero += other.composition_cases.source_alpha_zero;
        self.composition_cases.source_alpha_opaque += other.composition_cases.source_alpha_opaque;
        self.composition_cases.destination_alpha_zero +=
            other.composition_cases.destination_alpha_zero;
        self.composition_cases.destination_alpha_opaque +=
            other.composition_cases.destination_alpha_opaque;
        self.composition_cases.general_partial_alpha +=
            other.composition_cases.general_partial_alpha;
    }

    pub(crate) fn report_line(&self, workers: usize, frames: u64) -> String {
        format!(
            "vestra_cpu_profile workers={workers} frames={frames} source_rasterization_ns={} transform_sampling_ns={} layer_composition_ns={} effect_execution_ns={} motion_tile_ns={} gaussian_blur_ns={} zoom_blur_ns={} radial_blur_ns={} bloom_glow_ns={} bloom_highlight_extract_ns={} bloom_gaussian_blur_ns={} bloom_composite_ns={} chromatic_aberration_ns={} vignette_ns={} color_adjust_ns={} sharpen_ns={} sharpen_gaussian_ns={} sharpen_unsharp_composite_ns={} other_effects_ns={} global_post_effect_ns={} surface_copy_ns={} composition_source_alpha_zero={} composition_source_alpha_opaque={} composition_destination_alpha_zero={} composition_destination_alpha_opaque={} composition_general_partial_alpha={}",
            self.source_rasterization.as_nanos(),
            self.transform_sampling.as_nanos(),
            self.layer_composition.as_nanos(),
            self.effect_execution.as_nanos(),
            self.motion_tile.as_nanos(),
            self.gaussian_blur.as_nanos(),
            self.zoom_blur.as_nanos(),
            self.radial_blur.as_nanos(),
            self.bloom_glow.as_nanos(),
            self.bloom_highlight_extract.as_nanos(),
            self.bloom_gaussian_blur.as_nanos(),
            self.bloom_composite.as_nanos(),
            self.chromatic_aberration.as_nanos(),
            self.vignette.as_nanos(),
            self.color_adjust.as_nanos(),
            self.sharpen.as_nanos(),
            self.sharpen_gaussian.as_nanos(),
            self.sharpen_unsharp_composite.as_nanos(),
            self.other_effects.as_nanos(),
            self.global_post_effect.as_nanos(),
            self.surface_copy.as_nanos(),
            self.composition_cases.source_alpha_zero,
            self.composition_cases.source_alpha_opaque,
            self.composition_cases.destination_alpha_zero,
            self.composition_cases.destination_alpha_opaque,
            self.composition_cases.general_partial_alpha,
        )
    }
}
