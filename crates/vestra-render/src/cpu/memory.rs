//! Pixel storage estimates for reporting, not worker selection or process memory.

use std::collections::{BTreeMap, BTreeSet};

use vestra_core::plan::{
    CompiledLayer, CompiledMaskInput, CompiledVisualSource, EffectResource, RenderPlan,
    TimedEffect, compiled_effect_pass_plan,
};

pub(super) fn frame_bytes(width: u32, height: u32) -> u64 {
    u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4)
}

pub(super) fn estimated_worker_bytes(plan: &RenderPlan) -> u64 {
    let mut composition = storage_for_plan(plan);
    // The SDK retains at most capacity ready frames plus capacity worker frames.
    let canvas_buffers = 1_u64
        .saturating_add(composition.effect_buffers)
        .saturating_add(u64::from(composition.mask_local))
        .saturating_add(2 * u64::from(composition.masks))
        .saturating_add(2 * u64::from(composition.feather))
        .saturating_add(2)
        .saturating_add(composition.depths.len() as u64)
        .saturating_add(peak_distinct(std::mem::take(&mut composition.mattes)))
        .saturating_add(composition.temporary_frames);
    let mut bytes =
        frame_bytes(plan.canvas.width, plan.canvas.height).saturating_mul(canvas_buffers);
    // Cursors retain current RGBA and scaler output; layer requests are serial,
    // so only one incoming RGBA conversion overlaps those retained buffers.
    // Native YUV/lookahead/codec state has no format-independent bound here.
    // Cache budgets and shared decoded assets are separate.
    let mut incoming_bytes = 0;
    for (asset, events) in composition.videos {
        let cursors = peak_distinct(events).max(1);
        if let Some(video) = plan.videos.get(asset) {
            let source_bytes = frame_bytes(video.width, video.height);
            bytes = bytes.saturating_add(source_bytes.saturating_mul(2).saturating_mul(cursors));
            incoming_bytes = incoming_bytes.max(source_bytes);
        }
    }
    bytes.saturating_add(incoming_bytes)
}

// Matching origins and source clocks produce exactly the same requested time.
type VideoClock = (u128, u64, u64);
type VideoEvents = BTreeMap<usize, Vec<(u128, bool, VideoClock)>>;

fn peak_distinct<K: Ord>(mut events: Vec<(u128, bool, K)>) -> u64 {
    // Deactivation precedes activation at half-open boundaries.
    events.sort_unstable();
    let mut active = BTreeMap::<K, usize>::new();
    let mut peak = 0;
    for (_, activate, key) in events {
        if activate {
            *active.entry(key).or_default() += 1;
        } else if let Some(count) = active.get_mut(&key) {
            *count -= 1;
            if *count == 0 {
                active.remove(&key);
            }
        }
        peak = peak.max(active.len() as u64);
    }
    peak
}

fn storage_for_plan(plan: &RenderPlan) -> CompositionStorage {
    let output_end = vestra_core::timeline::frame_time_nanos(
        plan.frame_count,
        plan.frame_rate.0,
        plan.frame_rate.1,
    )
    .unwrap_or(u128::MAX);
    let mut storage = CompositionStorage {
        inactive_roots: plan
            .layers
            .iter()
            .filter(|layer| layer.start_frame >= layer.end_frame)
            .map(|layer| layer.compiled_identity)
            .collect(),
        ..CompositionStorage::default()
    };
    storage.record_effects(&plan.post_effects);
    storage.visit_layers(
        &plan.layers,
        Visit {
            origin: 0,
            span: (0, output_end),
            depth: 0,
            temporary: 0,
            isolated: false,
        },
    );
    storage
}

#[derive(Clone, Copy)]
struct Visit {
    origin: u128,
    span: (u128, u128),
    depth: usize,
    temporary: u64,
    isolated: bool,
}

#[derive(Default)]
struct CompositionStorage {
    effect_buffers: u64,
    mask_local: bool,
    masks: bool,
    feather: bool,
    depths: BTreeSet<usize>,
    mattes: Vec<(u128, bool, usize)>,
    videos: VideoEvents,
    inactive_roots: BTreeSet<usize>,
    temporary_frames: u64,
    visited: BTreeMap<(usize, usize, bool, u128, u128, u128), u64>,
}

impl CompositionStorage {
    fn record_effects(&mut self, effects: &[TimedEffect]) {
        for effect in effects {
            let passes = compiled_effect_pass_plan(&effect.effect);
            if !passes.as_slice().is_empty() {
                // A pass needs a distinct destination. Retained originals and
                // two-temporary topologies can pin the third physical image.
                let third = passes.as_slice().iter().any(|pass| {
                    pass.inputs.uses_original()
                        || pass.inputs.primary() == EffectResource::Temporary1
                        || pass.inputs.secondary() == Some(EffectResource::Temporary1)
                        || pass.output == EffectResource::Temporary1
                });
                self.effect_buffers = self.effect_buffers.max(1 + u64::from(third));
            }
        }
    }

    fn visit_layers(&mut self, layers: &[CompiledLayer], visit: Visit) {
        for layer in layers.iter().filter(|layer| layer.visible) {
            self.visit_layer(layer, layers, visit);
        }
    }

    fn visit_layer(&mut self, layer: &CompiledLayer, scope: &[CompiledLayer], visit: Visit) {
        if self.inactive_roots.contains(&layer.compiled_identity) {
            return;
        }
        let origin = visit.origin.saturating_add(layer.start_nanos);
        let span = (
            visit.span.0.max(origin),
            visit
                .span
                .1
                .min(origin.saturating_add(layer.duration_nanos)),
        );
        if span.0 >= span.1 {
            return;
        }
        let key = (
            layer.compiled_identity,
            visit.depth,
            visit.isolated,
            origin,
            span.0,
            span.1,
        );
        if self
            .visited
            .get(&key)
            .is_some_and(|previous| *previous >= visit.temporary)
        {
            return;
        }
        self.visited.insert(key, visit.temporary);
        self.record_effects(&layer.effects);
        // Optional capacities survive between frames, even in disjoint phases.
        self.mask_local |= !layer.masks.is_empty() || layer.matte.is_some();
        self.masks |= !layer.masks.is_empty();
        self.feather |= layer.masks.iter().any(|mask| {
            mask.feather.authored_track.base_value > 0.0
                || !mask.feather.authored_track.keyframes.is_empty()
                || !mask.feather.modifiers.is_empty()
        });
        self.temporary_frames = self.temporary_frames.max(visit.temporary);
        if let CompiledVisualSource::Group(group) = &layer.source {
            // Normal groups use depth; isolated matte groups use depth + 1.
            let depth = visit.depth.saturating_add(usize::from(visit.isolated));
            self.depths.insert(depth);
            self.visit_layers(
                &group.layers,
                Visit {
                    origin,
                    span,
                    depth: depth.saturating_add(1),
                    isolated: false,
                    ..visit
                },
            );
        } else {
            self.record_video(&layer.source, origin, span);
        }
        for mask in &layer.masks {
            if let CompiledMaskInput::Source { source, .. } = &mask.input {
                if let CompiledVisualSource::Group(group) = source.as_ref() {
                    // A group mask keeps an owner copy and temporary group image.
                    let temporary = visit.temporary.saturating_add(2);
                    self.temporary_frames = self.temporary_frames.max(temporary);
                    self.visit_layers(
                        &group.layers,
                        Visit {
                            origin,
                            span,
                            depth: visit.depth.saturating_add(1),
                            temporary,
                            isolated: false,
                        },
                    );
                } else {
                    self.record_video(source, origin, span);
                }
            }
        }
        if let Some(source) = layer.matte.as_ref().and_then(|matte| {
            scope
                .iter()
                .find(|source| source.compiled_identity == matte.source_layer_identity)
        }) {
            let source_start = visit.origin.saturating_add(source.start_nanos);
            let source_span = (
                span.0.max(source_start),
                span.1
                    .min(source_start.saturating_add(source.duration_nanos)),
            );
            if source_span.0 < source_span.1
                && !self.inactive_roots.contains(&source.compiled_identity)
            {
                // Consumer save slots persist; matte-cache images last one frame.
                let depth = visit.depth.saturating_add(1);
                self.depths.insert(depth);
                self.mattes.extend([
                    (source_span.0, true, source.compiled_identity),
                    (source_span.1, false, source.compiled_identity),
                ]);
                self.visit_layer(
                    source,
                    scope,
                    Visit {
                        span: source_span,
                        depth,
                        isolated: true,
                        ..visit
                    },
                );
            }
        }
    }

    fn record_video(&mut self, source: &CompiledVisualSource, origin: u128, span: (u128, u128)) {
        if let CompiledVisualSource::Video {
            asset_index,
            source_start,
            playback_rate,
            ..
        } = source
        {
            let clock = (origin, source_start.to_bits(), playback_rate.to_bits());
            self.videos
                .entry(*asset_index)
                .or_default()
                .extend([(span.0, true, clock), (span.1, false, clock)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{ValidationOptions, load_and_validate};
    use vestra_core::plan::{
        ActiveSchedule, CompileOptions, CompiledComposition, TemporalDependency,
    };

    fn fixture_plan() -> RenderPlan {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/geometric-masks.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("fixture validates");
        vestra_core::plan::compile(validated, CompileOptions::default()).expect("fixture compiles")
    }

    fn group(plan: &RenderPlan, layers: Vec<CompiledLayer>) -> CompiledVisualSource {
        CompiledVisualSource::Group(std::sync::Arc::new(CompiledComposition {
            layers,
            schedule: ActiveSchedule::compile(plan),
            dependency: TemporalDependency::Dynamic,
        }))
    }

    #[test]
    fn estimate_unions_optional_storage_across_timeline_phases_and_global_effects() {
        use vestra_core::animation::{Interpolation, Keyframe, Track};
        use vestra_core::plan::{CompiledEffect, CompiledScalarProperty};
        let mut plan = fixture_plan();
        let bytes = frame_bytes(plan.canvas.width, plan.canvas.height);
        let mut plain = plan.layers[0].clone();
        plain.masks.clear();
        plan.layers = vec![plain.clone()];
        assert_eq!(estimated_worker_bytes(&plan), 3 * bytes);
        let blur = TimedEffect {
            start: 500_000_000,
            end: 1_000_000_000,
            effect: CompiledEffect::GaussianBlur {
                radius: CompiledScalarProperty::authored(Track::new(2.0)),
            },
            dependency: TemporalDependency::Dynamic,
        };
        plan.post_effects = vec![blur.clone()];
        assert_eq!(estimated_worker_bytes(&plan), 4 * bytes);
        plan.post_effects.clear();
        plain.effects = vec![blur];
        plain.duration_nanos = 500_000_000;
        let mut masked = fixture_plan().layers.remove(0);
        masked.compiled_identity = 10;
        masked.start_nanos = 500_000_000;
        masked.start_frame = 15;
        masked.duration_nanos = 500_000_000;
        masked.masks[0].feather.authored_track.keyframes = vec![Keyframe {
            time: 250_000_000,
            value: 2.0,
            interpolation: Interpolation::Linear,
        }];
        plan.layers = vec![plain, masked.clone()];
        assert_eq!(
            estimated_worker_bytes(&plan),
            9 * bytes,
            "effect images, masks and animated feather persist across disjoint phases"
        );
        plan.layers[1].visible = false;
        assert_eq!(estimated_worker_bytes(&plan), 4 * bytes);
        plan.layers[0].matte = Some(vestra_core::plan::CompiledTrackMatte {
            source_layer_id: masked.id.clone(),
            source_layer_identity: 10,
            mode: crate::project::MatteMode::Alpha,
            invert: false,
        });
        plan.layers[0].duration_nanos = 1_000_000_000;
        assert_eq!(
            estimated_worker_bytes(&plan),
            11 * bytes,
            "reachable hidden mattes contribute masks/feather and save/cache storage"
        );
    }

    #[test]
    fn estimate_reuses_sibling_composition_depths_and_counts_group_mask_temporaries() {
        let mut plan = fixture_plan();
        let bytes = frame_bytes(plan.canvas.width, plan.canvas.height);
        let mut child = plan.layers[0].clone();
        child.compiled_identity = 10;
        child.masks.clear();
        assert_eq!(estimated_worker_bytes(&plan), 6 * bytes);
        let mut nested = child.clone();
        nested.compiled_identity = 11;
        nested.source = group(&plan, vec![child.clone()]);
        let mut parent = child.clone();
        parent.compiled_identity = 12;
        parent.source = group(&plan, vec![nested]);
        plan.layers = vec![parent.clone()];
        assert_eq!(estimated_worker_bytes(&plan), 5 * bytes);
        parent.compiled_identity = 13;
        plan.layers.push(parent);
        assert_eq!(estimated_worker_bytes(&plan), 5 * bytes);
        let mut mask = fixture_plan().layers[0].masks[0].clone();
        mask.input = CompiledMaskInput::Source {
            source: Box::new(group(&plan, vec![child.clone()])),
            mode: crate::project::MaskCoverageMode::Alpha,
        };
        child.masks = vec![mask];
        plan.layers = vec![child];
        assert_eq!(estimated_worker_bytes(&plan), 8 * bytes);
    }

    #[test]
    fn estimate_counts_matte_save_slots_and_one_cached_image_per_source() {
        let validated = load_and_validate(
            std::path::Path::new("examples/projects/track-matte.json"),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("matte fixture validates");
        let mut plan =
            vestra_core::plan::compile(validated, CompileOptions::default()).expect("plan");
        let bytes = frame_bytes(plan.canvas.width, plan.canvas.height);
        assert_eq!(estimated_worker_bytes(&plan), 6 * bytes);
        let mut consumer = plan.layers[0].clone();
        consumer.compiled_identity = 100;
        plan.layers.push(consumer);
        assert_eq!(estimated_worker_bytes(&plan), 6 * bytes);
        let mut child = plan.layers[1].clone();
        child.compiled_identity = 101;
        child.matte = None;
        plan.layers[1].source = group(&plan, vec![child]);
        assert_eq!(estimated_worker_bytes(&plan), 7 * bytes);
    }

    fn video_plan() -> RenderPlan {
        let mut plan = fixture_plan();
        plan.visual_dependency = TemporalDependency::Dynamic;
        plan.videos.push(vestra_core::plan::VideoAsset {
            id: "video".to_owned(),
            path: "video.mp4".into(),
            duration_seconds: 1.0,
            width: 17,
            height: 9,
        });
        plan.layers[0].source = CompiledVisualSource::Video {
            asset_index: 0,
            video_slot_index: 0,
            source_start: 0.0,
            playback_rate: 1.0,
            crop: vestra_core::animation::Track::new(crate::domain::Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }),
            sizing: vestra_core::plan::CompiledSizing::Original,
        };
        plan
    }

    #[test]
    fn resource_estimates_do_not_change_legacy_worker_selection() {
        let mut plan = video_plan();
        for (width, height, workers) in [(1_920, 1_080, 8), (2_560, 1_440, 6), (3_840, 2_160, 2)] {
            plan.canvas.width = width;
            plan.canvas.height = height;
            plan.videos[0].width = 17;
            plan.videos[0].height = 9;
            let small = estimated_worker_bytes(&plan);
            assert_eq!(
                super::super::backend::automatic_worker_count(&plan, 64),
                workers
            );
            plan.videos[0].width = u32::MAX;
            plan.videos[0].height = u32::MAX;
            assert!(small < estimated_worker_bytes(&plan));
            assert_eq!(estimated_worker_bytes(&plan), u64::MAX);
            assert_eq!(
                super::super::backend::automatic_worker_count(&plan, 64),
                workers
            );
        }
    }

    #[test]
    fn video_estimate_reuses_serial_cursors_and_counts_distinct_overlapping_times() {
        let mut plan = video_plan();
        plan.frame_count = 300;
        let mut layer = plan.layers.remove(0);
        layer.masks.clear();
        layer.start_nanos = 0;
        layer.duration_nanos = 1_000_000_000;
        plan.layers = vec![layer.clone()];
        let one = estimated_worker_bytes(&plan);
        for index in 1..5 {
            let mut serial = layer.clone();
            serial.compiled_identity = index;
            serial.start_nanos = index as u128 * 1_000_000_000;
            plan.layers.push(serial);
        }
        assert_eq!(estimated_worker_bytes(&plan), one);
        let mut duplicate = layer.clone();
        duplicate.compiled_identity = 11;
        plan.layers = vec![layer.clone(), duplicate];
        assert_eq!(
            estimated_worker_bytes(&plan),
            one,
            "same asset/time shares a cursor"
        );
        if let CompiledVisualSource::Video { source_start, .. } = &mut plan.layers[1].source {
            *source_start = 0.5;
        }
        let video_bytes = frame_bytes(plan.videos[0].width, plan.videos[0].height);
        assert_eq!(estimated_worker_bytes(&plan), one + 2 * video_bytes);
        let nested = group(&plan, plan.layers.clone());
        layer.source = nested;
        layer.start_nanos = 5_000_000_000;
        layer.duration_nanos = 500_000_000;
        plan.layers = vec![layer.clone()];
        assert_eq!(
            peak_distinct(storage_for_plan(&plan).videos.remove(&0).expect("video")),
            2
        );
        // A child outside its parent's local span never requests a decoder.
        layer.source = group(
            &plan,
            vec![{
                let mut child = video_plan().layers.remove(0);
                child.start_nanos = 1_000_000_000;
                child
            }],
        );
        plan.layers = vec![layer];
        assert!(storage_for_plan(&plan).videos.is_empty());
    }

    #[test]
    fn video_estimate_counts_only_one_incoming_conversion_across_sources() {
        let mut plan = video_plan();
        plan.canvas.width = 1_920;
        plan.canvas.height = 1_080;
        plan.videos[0].width = 1_280;
        plan.videos[0].height = 720;
        let layer = plan.layers[0].clone();
        for index in 1..3 {
            plan.videos.push(plan.videos[0].clone());
            let mut next = layer.clone();
            next.compiled_identity = index;
            if let CompiledVisualSource::Video { asset_index, .. } = &mut next.source {
                *asset_index = index;
            }
            plan.layers.push(next);
        }
        let canvas = frame_bytes(1_920, 1_080);
        let source = frame_bytes(1_280, 720);
        assert_eq!(estimated_worker_bytes(&plan), 6 * canvas + 7 * source);
        assert_eq!(super::super::backend::automatic_worker_count(&plan, 64), 7);
        // Keep uncached current frames and every retained scaler in the bound.
        plan.limits.maximum_cache_bytes = 0;
        assert_eq!(estimated_worker_bytes(&plan), 6 * canvas + 7 * source);
        plan.videos[2].width = 2_560;
        plan.videos[2].height = 1_440;
        let larger = frame_bytes(2_560, 1_440);
        assert_eq!(
            estimated_worker_bytes(&plan),
            6 * canvas + 4 * source + 3 * larger
        );
    }

    #[test]
    fn estimate_uses_source_sized_rgba_decode_storage_and_saturates() {
        let mut plan = video_plan();
        let canvas_bytes = frame_bytes(plan.canvas.width, plan.canvas.height);
        assert_eq!(
            estimated_worker_bytes(&plan),
            6 * canvas_bytes + 3 * 17 * 9 * 4
        );
        plan.limits.maximum_cache_bytes = u64::MAX;
        assert_eq!(
            estimated_worker_bytes(&plan),
            6 * canvas_bytes + 3 * 17 * 9 * 4
        );
        plan.videos[0].width = u32::MAX;
        plan.videos[0].height = u32::MAX;
        assert_eq!(estimated_worker_bytes(&plan), u64::MAX);
        assert_eq!(frame_bytes(u32::MAX, u32::MAX), u64::MAX);
    }

    #[test]
    fn unused_hidden_and_out_of_output_layers_do_not_reduce_workers() {
        let mut plan = video_plan();
        plan.canvas.width = 1_920;
        plan.canvas.height = 1_080;
        let bytes = estimated_worker_bytes(&plan);
        let workers = super::super::backend::automatic_worker_count(&plan, 64);
        let mut unused = plan.layers[0].clone();
        unused.compiled_identity = 100;
        for identity in 101..106 {
            let source = group(&plan, vec![unused.clone()]);
            unused.compiled_identity = identity;
            unused.source = source;
        }
        unused.visible = false;
        plan.layers.push(unused.clone());
        assert_eq!(estimated_worker_bytes(&plan), bytes);
        unused.visible = true;
        unused.compiled_identity = 200;
        unused.start_nanos = 10_000_000_000;
        plan.layers.push(unused.clone());
        assert_eq!(estimated_worker_bytes(&plan), bytes);
        unused.compiled_identity = 201;
        unused.start_nanos = 0;
        unused.start_frame = 1;
        unused.end_frame = 1;
        plan.layers.push(unused);
        assert_eq!(estimated_worker_bytes(&plan), bytes);
        assert_eq!(
            super::super::backend::automatic_worker_count(&plan, 64),
            workers
        );
    }

    #[test]
    fn disjoint_consumers_share_hidden_matte_cursor_and_frame_cache_allowance() {
        let mut plan = video_plan();
        plan.frame_count = 150;
        let mut source = plan.layers[0].clone();
        source.visible = false;
        source.masks.clear();
        source.start_nanos = 0;
        source.duration_nanos = 5_000_000_000;
        source.start_frame = 0;
        source.end_frame = 150;
        let mut consumer = fixture_plan().layers.remove(0);
        consumer.masks.clear();
        consumer.duration_nanos = 1_000_000_000;
        plan.layers.clear();
        let mut one = 0;
        for index in 0..5 {
            source.compiled_identity = 100 + index;
            if let CompiledVisualSource::Video { source_start, .. } = &mut source.source {
                *source_start = index as f64;
            }
            consumer.compiled_identity = 200 + index;
            consumer.start_nanos = index as u128 * 1_000_000_000;
            consumer.start_frame = index as u64 * 30;
            consumer.end_frame = consumer.start_frame + 30;
            consumer.matte = Some(vestra_core::plan::CompiledTrackMatte {
                source_layer_id: source.id.clone(),
                source_layer_identity: source.compiled_identity,
                mode: crate::project::MatteMode::Alpha,
                invert: false,
            });
            plan.layers.extend([source.clone(), consumer.clone()]);
            if index == 0 {
                one = estimated_worker_bytes(&plan);
            }
            assert_eq!(estimated_worker_bytes(&plan), one);
        }
        let mut storage = storage_for_plan(&plan);
        assert_eq!(
            peak_distinct(storage.videos.remove(&0).expect("referenced video")),
            1
        );
        assert_eq!(peak_distinct(storage.mattes), 1);
        for layer in &mut plan.layers {
            layer.matte = None;
        }
        assert!(
            storage_for_plan(&plan).videos.is_empty(),
            "hidden sources alone do not decode"
        );
    }
}
