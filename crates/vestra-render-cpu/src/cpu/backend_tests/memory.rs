use super::*;

#[test]
fn scratch_capacity_includes_nested_compositions_across_workers_and_resize() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    for workers in [1, 3] {
        plan.canvas.width = 4;
        plan.canvas.height = 4;
        plan.canvas.logical_width = plan.canvas.width;
        plan.canvas.logical_height = plan.canvas.height;
        let decoded = DecodedAssets::build(&plan).expect("fixture assets");
        let shared_bytes = decoded.stats().decoded_source_bytes;
        let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, workers);
        for (index, size) in [4, 6, 2, 2].into_iter().enumerate() {
            let mut frame = static_frame();
            frame.width = size;
            frame.height = size;
            let mut child = frame.layers[0].clone();
            child.content_dependency = TemporalDependency::Dynamic;
            let mut group = child.clone();
            group.source = EvaluatedSource::Group {
                composition: vestra_core::plan::EvaluatedComposition {
                    is_sized: false,
                    width: size,
                    height: size,
                    layers: vec![child],
                },
            };
            let mut parent = group.clone();
            parent.source = EvaluatedSource::Group {
                composition: vestra_core::plan::EvaluatedComposition {
                    is_sized: false,
                    width: size,
                    height: size,
                    layers: vec![group],
                },
            };
            frame.layers = vec![parent];
            let mut outputs = Vec::new();
            for number in 0..workers {
                backend.submit_frame(number as u64, &frame).expect("submit");
            }
            for _ in 0..workers {
                outputs.push(
                    backend
                        .poll_completed(PollMode::WaitForOne)
                        .expect("poll")
                        .expect("frame"),
                );
            }
            assert!(outputs.windows(2).all(|pair| pair[0].rgba == pair[1].rgba));
            let stats = backend.stats();
            assert_eq!(stats.cpu_scratch_buffers_retained, 3 * workers);
            assert_eq!(
                stats.cpu_scratch_bytes_retained,
                3 * u64::from(size) * u64::from(size) * 4 * workers as u64
            );
            assert_eq!(
                stats.cpu_scratch_allocations,
                3 * (index + 1).min(3) as u64 * workers as u64
            );
            assert_eq!(stats.decoded_source_bytes, shared_bytes);
        }
    }
}

#[test]
fn snapshot_memory_gauges_sum_worker_owners_but_keep_shared_assets_once() {
    let snapshot = || WorkerSnapshot {
        stats: PreparationStats {
            decoded_source_bytes: 100,
            video_cache_bytes_retained: 8,
            video_cache_bytes_peak: 12,
            video_current_frame_bytes_retained: 4,
            video_decoder_buffer_bytes_retained: 16,
            ..PreparationStats::default()
        },
        timings: PreparationTimings::default(),
        hot_path_timings: CpuHotPathTimings::default(),
    };
    let stats = aggregate_snapshots(&[snapshot(), snapshot(), snapshot()]);
    assert_eq!(stats.decoded_source_bytes, 100);
    assert_eq!(stats.video_cache_bytes_retained, 24);
    assert_eq!(stats.video_cache_bytes_peak, 36);
    assert_eq!(stats.video_current_frame_bytes_retained, 12);
    assert_eq!(stats.video_decoder_buffer_bytes_retained, 48);
}

#[test]
fn aggregate_snapshots_sums_independent_cache_gauges_and_keeps_shared_decode_once() {
    let worker = |budget, current_entries, peak_entries, current_bytes, peak_bytes, hits| {
        let mut stats = PreparationStats {
            decoded_image_count: 9,
            decoded_source_bytes: 900,
            peak_decoded_bytes: 1_000,
            cache_budget_bytes: budget,
            cache_current_entries: current_entries,
            peak_cache_entries: peak_entries,
            cache_current_bytes: current_bytes,
            cache_peak_bytes: peak_bytes,
            bitmap_cache_hits: hits,
            bitmap_cache_requests: 10,
            ..PreparationStats::default()
        };
        stats.bitmap_cache_misses = 10 - hits;
        WorkerSnapshot {
            stats,
            timings: PreparationTimings::default(),
            hot_path_timings: CpuHotPathTimings::default(),
        }
    };

    let aggregate =
        aggregate_snapshots(&[worker(100, 2, 3, 40, 60, 3), worker(100, 1, 4, 20, 70, 5)]);

    assert_eq!(aggregate.cache_budget_bytes, 200);
    assert_eq!(aggregate.cache_current_entries, 3);
    assert_eq!(aggregate.peak_cache_entries, 7);
    assert_eq!(aggregate.cache_current_bytes, 60);
    assert_eq!(aggregate.cache_peak_bytes, 130);
    assert_eq!(aggregate.decoded_image_count, 9);
    assert_eq!(aggregate.decoded_source_bytes, 900);
    assert_eq!(aggregate.peak_decoded_bytes, 1_000);
    assert_eq!(aggregate.bitmap_cache_hit_rate, Some(0.4));
}

#[test]
fn multiple_workers_partition_both_cache_classes_without_multiplying_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.limits.maximum_cache_bytes = 257;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 4);

    assert_eq!(backend.worker_cache_budgets.len(), 4);
    let crop_budget = backend
        .worker_cache_budgets
        .iter()
        .map(|budget| budget.crop_cache_budget_bytes)
        .sum::<u64>();
    let static_budget = backend
        .worker_cache_budgets
        .iter()
        .map(|budget| budget.static_cache_budget_bytes)
        .sum::<u64>();
    let video_budget = backend
        .worker_cache_budgets
        .iter()
        .map(|budget| budget.video_cache_budget_bytes)
        .sum::<u64>();
    assert!(crop_budget + static_budget + video_budget <= 257);
    assert_eq!(video_budget, 0);
    assert_eq!(backend.stats().cache_budget_bytes, 257);
}

#[test]
fn reuses_complete_static_layer_surfaces_without_mutating_them() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 4;
    plan.canvas.height = 4;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    assert_eq!(backend.capacity(), 1);
    let mut frame = static_frame();
    frame.layers[0].effects = vec![
        EvaluatedEffect::Brightness { amount: 0.1 },
        EvaluatedEffect::Tint {
            colour: [0, 0, 255, 255],
            amount: 0.5,
        },
    ];
    frame.layers[0].colour_transform =
        ColourTransform::from_effects(frame.layers[0].effects.clone());

    backend.submit_frame(0, &frame).expect("first submission");
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll")
        .expect("first completion");
    backend.submit_frame(1, &frame).expect("second submission");
    let second = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll")
        .expect("second completion");

    assert_eq!(first.rgba, second.rgba);
    let stats = backend.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_cache_hits, 1);
    assert_eq!(stats.static_cache_entries, 1);
    assert_eq!(stats.static_cache_population_renders, 1);
    assert_eq!(stats.static_layers_rendered, 1);
    assert_eq!(stats.cpu_scratch_allocations, 2);
}

#[test]
fn static_layer_renders_once_across_one_hundred_frames() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut frame = static_frame();
    frame.layers[0].effects = vec![
        EvaluatedEffect::Brightness { amount: 0.1 },
        EvaluatedEffect::Tint {
            colour: [0, 0, 255, 255],
            amount: 0.5,
        },
    ];
    frame.layers[0].colour_transform =
        ColourTransform::from_effects(frame.layers[0].effects.clone());

    for frame_number in 0..100 {
        backend
            .submit_frame(frame_number, &frame)
            .expect("frame submits");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion");
    }

    let stats = backend.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_cache_hits, 99);
    assert_eq!(stats.static_cache_population_renders, 1);
    assert_eq!(stats.static_layers_rendered, 1);
}

#[test]
fn static_layer_activity_does_not_affect_its_cache_identity() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut inactive = static_frame();
    inactive.layers.clear();
    let active = static_frame();

    for (number, frame) in [
        (10, &inactive),
        (30, &active),
        (45, &active),
        (100, &inactive),
    ] {
        backend.submit_frame(number, frame).expect("frame submits");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion");
    }

    let stats = backend.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_cache_hits, 1);
    assert_eq!(stats.static_cache_entries, 1);
}

#[test]
fn static_cache_matches_the_dynamic_reference_path() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut frame = static_frame();
    frame.layers[0].effects = vec![
        EvaluatedEffect::Brightness { amount: 0.1 },
        EvaluatedEffect::Tint {
            colour: [0, 0, 255, 255],
            amount: 0.5,
        },
    ];
    frame.layers[0].colour_transform =
        ColourTransform::from_effects(frame.layers[0].effects.clone());
    let mut cached = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    let mut reference = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut reference_frame = frame.clone();
    reference_frame.layers[0].content_dependency = TemporalDependency::Dynamic;

    cached.submit_frame(0, &frame).expect("cached submission");
    reference
        .submit_frame(0, &reference_frame)
        .expect("reference submission");
    let cached = cached
        .poll_completed(PollMode::WaitForOne)
        .expect("cached poll")
        .expect("cached completion");
    let reference = reference
        .poll_completed(PollMode::WaitForOne)
        .expect("reference poll")
        .expect("reference completion");

    assert_eq!(cached.rgba, reference.rgba);
}

#[test]
fn dynamic_effect_scratch_allocations_stabilize_after_warmup() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 4;
    plan.canvas.height = 4;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut frame = static_frame();
    frame.layers[0].content_dependency = TemporalDependency::Dynamic;
    frame.layers[0].effects = vec![EvaluatedEffect::GaussianBlur { radius: 1.0 }];

    for frame_number in 0..100 {
        backend
            .submit_frame(frame_number, &frame)
            .expect("frame submits");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion");
    }

    let stats = backend.stats();
    assert_eq!(stats.cpu_full_frame_allocations, 100);
    assert_eq!(stats.cpu_scratch_allocations, 2);
    assert_eq!(stats.cpu_scratch_reuses, 199);
    assert_eq!(stats.cpu_scratch_buffers_retained, 2);
    assert_eq!(stats.cpu_scratch_bytes_retained, 128);
}

#[test]
fn dynamic_layers_bypass_the_whole_layer_cache() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut frame = static_frame();
    frame.layers[0].content_dependency = TemporalDependency::Dynamic;

    backend.submit_frame(0, &frame).expect("first submission");
    backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll");
    backend.submit_frame(1, &frame).expect("second submission");
    backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll");

    let stats = backend.stats();
    assert_eq!(stats.static_cache_hits, 0);
    assert_eq!(stats.static_cache_misses, 0);
    assert_eq!(stats.static_cache_entries, 0);
    assert_eq!(stats.static_layers_rendered, 0);
}

#[test]
fn over_budget_static_layers_render_without_retention() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.limits.maximum_cache_bytes = 1;
    plan.canvas.width = 4;
    plan.canvas.height = 4;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    let mut reference = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let frame = static_frame();
    let mut reference_frame = frame.clone();
    reference_frame.layers[0].content_dependency = TemporalDependency::Dynamic;

    let mut outputs = Vec::new();
    for frame_number in 0..100 {
        backend
            .submit_frame(frame_number, &frame)
            .expect("frame submits");
        let output = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion")
            .rgba;
        reference
            .submit_frame(frame_number, &reference_frame)
            .expect("reference frame submits");
        let reference = reference
            .poll_completed(PollMode::WaitForOne)
            .expect("reference frame poll")
            .expect("reference frame completion");
        assert_eq!(output, reference.rgba);
        outputs.push(output);
    }

    assert!(outputs.windows(2).all(|frames| frames[0] == frames[1]));
    let stats = backend.stats();
    assert_eq!(stats.static_cache_entries, 0);
    assert_eq!(stats.static_cache_hits, 0);
    assert_eq!(stats.static_cache_misses, 100);
    assert_eq!(stats.static_cache_budget_bypasses, 100);
    assert_eq!(stats.static_layers_rendered, 100);
    assert_eq!(stats.cpu_scratch_allocations, 1);
}

#[test]
fn distinct_static_layers_do_not_alias_or_mutate_under_dynamic_composition() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut base = static_frame();
    let mut second = base.layers[0].clone();
    second.compiled_layer_index = 8;
    second.source = EvaluatedSource::SolidColor {
        colour: [180, 20, 40, 255],
    };
    base.layers.push(second);

    backend.submit_frame(0, &base).expect("base submission");
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("base poll")
        .expect("base completion");

    let mut changed = base.clone();
    let mut dynamic = changed.layers[0].clone();
    dynamic.compiled_layer_index = 9;
    dynamic.content_dependency = TemporalDependency::Dynamic;
    dynamic.source = EvaluatedSource::SolidColor {
        colour: [5, 220, 30, 255],
    };
    dynamic.opacity = 0.4;
    changed.layers.push(dynamic);
    backend
        .submit_frame(1, &changed)
        .expect("dynamic submission");
    backend
        .poll_completed(PollMode::WaitForOne)
        .expect("dynamic poll")
        .expect("dynamic completion");

    backend.submit_frame(2, &base).expect("restored submission");
    let restored = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("restored poll")
        .expect("restored completion");

    assert_eq!(first.rgba, restored.rgba);
    let stats = backend.stats();
    assert_eq!(stats.static_cache_entries, 2);
    assert_eq!(stats.static_cache_misses, 2);
    assert_eq!(stats.static_cache_hits, 4);
}
