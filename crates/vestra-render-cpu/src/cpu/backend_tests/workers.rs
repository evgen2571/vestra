use super::*;

#[test]
fn aggregate_hot_path_timings_sums_worker_local_durations() {
    let first = CpuHotPathTimings {
        source_rasterization: Duration::from_millis(3),
        zoom_blur: Duration::from_millis(5),
        ..CpuHotPathTimings::default()
    };
    let second = CpuHotPathTimings {
        source_rasterization: Duration::from_millis(7),
        zoom_blur: Duration::from_millis(11),
        global_post_effect: Duration::from_millis(13),
        ..CpuHotPathTimings::default()
    };
    let snapshots = [
        WorkerSnapshot {
            stats: PreparationStats::default(),
            timings: PreparationTimings::default(),
            hot_path_timings: first,
        },
        WorkerSnapshot {
            stats: PreparationStats::default(),
            timings: PreparationTimings::default(),
            hot_path_timings: second,
        },
    ];

    let aggregate = aggregate_hot_path_timings(&snapshots);
    assert_eq!(aggregate.source_rasterization, Duration::from_millis(10));
    assert_eq!(aggregate.zoom_blur, Duration::from_millis(16));
    assert_eq!(aggregate.global_post_effect, Duration::from_millis(13));
    assert!(aggregate.layer_composition.is_zero());
}

#[test]
fn automatic_worker_policy_uses_one_worker_for_static_visuals() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/geometric-masks.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    assert_eq!(plan.visual_dependency, TemporalDependency::Static);
    for available in [0, 1, 2, 4, 64] {
        assert_eq!(automatic_worker_count(&plan, available), 1);
    }

    let decoded = DecodedAssets::build(&plan).expect("fixture assets prepare");
    let mut automatic = CpuBackend::new(&plan, Arc::clone(&decoded));
    let mut parallel = CpuBackend::new_with_worker_count(&plan, decoded, 8);
    assert_eq!(automatic.capacity(), 1);
    let mut expected = Vec::new();
    for number in 0..8 {
        let frame = evaluate(&plan, &[ScheduledItem(0)], u128::from(number) * 33_333_333)
            .expect("static frame evaluates");
        automatic.submit_frame(number, &frame).expect("submit");
        expected.push(
            automatic
                .poll_completed(PollMode::WaitForOne)
                .expect("poll")
                .expect("completion"),
        );
        parallel
            .submit_frame(number, &frame)
            .expect("parallel submit");
    }
    let mut actual = Vec::new();
    while parallel.in_flight() > 0 {
        actual.push(
            parallel
                .poll_completed(PollMode::WaitForOne)
                .expect("parallel poll")
                .expect("parallel completion"),
        );
    }
    actual.sort_by_key(|frame| frame.frame_number);
    assert_eq!(actual, expected);
    assert_eq!(automatic.stats().static_cache_misses, 1);
    assert_eq!(automatic.stats().static_cache_hits, 7);
    automatic.flush().expect("flush");
    automatic.verify_idle().expect("idle");
}

#[test]
fn automatic_worker_policy_preserves_legacy_canvas_heuristic() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");

    plan.canvas.width = 1_920;
    plan.canvas.height = 1_080;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    assert_eq!(automatic_worker_count(&plan, 1), 1);
    assert_eq!(automatic_worker_count(&plan, 2), 1);
    assert_eq!(automatic_worker_count(&plan, 4), 3);
    assert_eq!(automatic_worker_count(&plan, 64), 8);

    plan.canvas.width = 2_560;
    plan.canvas.height = 1_440;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    assert_eq!(automatic_worker_count(&plan, 64), 6);
    plan.canvas.width = 3_840;
    plan.canvas.height = 2_160;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    assert_eq!(automatic_worker_count(&plan, 64), 2);
}

#[test]
fn automatic_worker_policy_accounts_for_large_visual_and_mask_group_canvases() {
    use vestra_core::plan::CompiledVisualSource;
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 16;
    plan.canvas.height = 16;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    plan.visual_dependency = TemporalDependency::Dynamic;
    let group_source = |size| {
        CompiledVisualSource::Group(Arc::new(vestra_core::plan::CompiledComposition {
            size,
            layers: Vec::new(),
            schedule: vestra_core::plan::ActiveSchedule::compile(&plan),
            dependency: TemporalDependency::Dynamic,
        }))
    };
    let inherited = group_source(None);
    let large = group_source(Some(vestra_core::project::GroupSize {
        width: 8192.0,
        height: 8192.0,
    }));
    let thin = group_source(Some(vestra_core::project::GroupSize {
        width: 8192.0,
        height: 16.0,
    }));
    plan.layers.truncate(1);
    plan.layers[0].source = inherited;
    plan.layers[0].masks.clear();
    assert_eq!(
        automatic_worker_count(&plan, 64),
        8,
        "legacy inherited canvas"
    );
    plan.layers[0].source = thin;
    assert_eq!(
        automatic_worker_count(&plan, 64),
        8,
        "use area, not squared maximum axis"
    );
    plan.layers[0].source = large.clone();
    assert_eq!(automatic_worker_count(&plan, 64), 1, "large visual group");
    plan.layers[0].source = CompiledVisualSource::SolidColor { colour: [255; 4] };
    let transform = plan.layers[0].transform.clone();
    plan.layers[0].masks.push(vestra_core::plan::CompiledMask {
        input: vestra_core::plan::CompiledMaskInput::Source {
            source: Box::new(large),
            mode: crate::project::MaskCoverageMode::Alpha,
        },
        operation: crate::project::MaskOperation::Intersect,
        invert: false,
        strength: vestra_core::plan::CompiledScalarProperty::authored(
            vestra_core::animation::Track::new(1.0),
        ),
        feather: vestra_core::plan::CompiledScalarProperty::authored(
            vestra_core::animation::Track::new(0.0),
        ),
        transform,
    });
    assert_eq!(
        automatic_worker_count(&plan, 64),
        1,
        "large owned group mask"
    );
}

#[test]
fn automatic_worker_policy_is_never_zero_and_handles_overflow() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = u32::MAX;
    plan.canvas.height = u32::MAX;
    plan.canvas.logical_width = plan.canvas.width;
    plan.canvas.logical_height = plan.canvas.height;
    assert_eq!(automatic_worker_count(&plan, 0), 1);
    assert_eq!(automatic_worker_count(&plan, usize::MAX), 1);
}

#[test]
fn worker_budget_partitions_preserve_total_and_differ_by_at_most_one() {
    for (total, workers) in [(0, 1), (1, 4), (256 * 1024 * 1024, 2), (257, 4), (257, 8)] {
        let parts = (0..workers)
            .map(|worker_id| worker_budget(total, workers, worker_id))
            .collect::<Vec<_>>();
        assert_eq!(parts.iter().sum::<u64>(), total);
        let min = parts.iter().min().copied().unwrap_or(0);
        let max = parts.iter().max().copied().unwrap_or(0);
        assert!(max - min <= 1);
    }
}

#[test]
fn completion_accounting_sums_render_work_and_reset_clears_it() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    backend.worker_busy.fill(true);
    backend
        .consume_completion(WorkerCompletion::Frame {
            worker_id: 0,
            frame: CompletedFrame {
                frame_number: 0,
                rgba: Vec::new(),
            },
            render_duration: Duration::from_millis(7),
        })
        .expect("first synthetic completion");
    backend
        .consume_completion(WorkerCompletion::Frame {
            worker_id: 1,
            frame: CompletedFrame {
                frame_number: 1,
                rgba: Vec::new(),
            },
            render_duration: Duration::from_millis(11),
        })
        .expect("second synthetic completion");
    assert_eq!(
        backend.staged_metrics().frame_render_work_duration,
        Duration::from_millis(18)
    );
    backend.reset_operation_metrics();
    assert_eq!(
        backend.staged_metrics().frame_render_work_duration,
        Duration::ZERO
    );
}

#[test]
fn worker_panic_preserves_the_failing_frame_number() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    backend
        .submit_panicking_frame(10, &static_frame())
        .expect("panic job submission");
    let error = backend
        .poll_completed(PollMode::WaitForOne)
        .expect_err("worker panic reaches backend");
    assert_eq!(error.code, "CPU-WORKER-PANIC");
    assert_eq!(backend.failed_frame_number(), Some(10));
    backend.abort();
}

#[test]
fn completed_pixels_remain_owned_after_later_submission_and_polling() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame = evaluate(&plan, &[ScheduledItem(0)], 0).expect("renderer fixture evaluates");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);

    backend.submit_frame(3, &frame).expect("first submission");
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll")
        .expect("first completion");
    let first_pixels = first.rgba.clone();

    backend.submit_frame(4, &frame).expect("second submission");
    let second = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll")
        .expect("second completion");

    assert_eq!(first.frame_number, 3);
    assert_eq!(second.frame_number, 4);
    assert_eq!(first.rgba, first_pixels);
}

#[test]
fn explicit_workers_accept_multiple_frames_and_reuse_slots() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame = evaluate(&plan, &[ScheduledItem(0)], 0).expect("renderer fixture evaluates");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    assert_eq!(backend.capacity(), 2);
    backend.submit_frame(0, &frame).expect("first submission");
    backend.submit_frame(1, &frame).expect("second submission");
    assert_eq!(backend.in_flight(), 2);
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll")
        .expect("completion");
    let second = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll")
        .expect("completion");
    assert_eq!(backend.in_flight(), 0);
    assert_ne!(first.frame_number, second.frame_number);
    backend
        .submit_frame(2, &frame)
        .expect("reused worker submission");
    assert_eq!(
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("reuse poll")
            .expect("reuse completion")
            .frame_number,
        2
    );
    backend.flush().expect("flush");
    backend.verify_idle().expect("idle after flush");
}

#[test]
fn one_and_two_workers_produce_identical_frame_pixels() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frames: Vec<_> = (0..4)
        .map(|number| {
            evaluate(&plan, &[ScheduledItem(0)], number).expect("renderer fixture evaluates")
        })
        .collect();
    let mut single = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    let mut multi = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    let mut single_pixels = Vec::new();
    for (number, frame) in frames.iter().enumerate() {
        single
            .submit_frame(number as u64, frame)
            .expect("single submission");
        single_pixels.push(
            single
                .poll_completed(PollMode::WaitForOne)
                .expect("single poll")
                .expect("single completion"),
        );
    }
    let mut multi_pixels = Vec::new();
    for (number, frame) in frames.iter().enumerate() {
        if multi.in_flight() == multi.capacity() {
            multi_pixels.push(
                multi
                    .poll_completed(PollMode::WaitForOne)
                    .expect("multi poll")
                    .expect("multi completion"),
            );
        }
        multi
            .submit_frame(number as u64, frame)
            .expect("multi submission");
    }
    while multi.in_flight() > 0 {
        multi_pixels.push(
            multi
                .poll_completed(PollMode::WaitForOne)
                .expect("multi poll")
                .expect("multi completion"),
        );
    }
    single_pixels.sort_by_key(|frame| frame.frame_number);
    multi_pixels.sort_by_key(|frame| frame.frame_number);
    assert_eq!(single_pixels, multi_pixels);
}

#[test]
fn one_and_four_workers_produce_identical_frame_pixels() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frames: Vec<_> = (0..8)
        .map(|number| {
            evaluate(&plan, &[ScheduledItem(0)], number).expect("renderer fixture evaluates")
        })
        .collect();
    let collect = |worker_count, decoded: Arc<DecodedAssets>| {
        let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, worker_count);
        let mut completed = Vec::new();
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                completed.push(
                    backend
                        .poll_completed(PollMode::WaitForOne)
                        .expect("poll")
                        .expect("completion"),
                );
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("submission");
        }
        while backend.in_flight() > 0 {
            completed.push(
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("drain poll")
                    .expect("drain completion"),
            );
        }
        completed.sort_by_key(|frame| frame.frame_number);
        completed
    };
    let single = collect(1, Arc::clone(&decoded));
    let four = collect(4, decoded);
    assert_eq!(single, four);
}

#[test]
fn spectrum2d_one_two_and_four_workers_are_byte_identical() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frames: Vec<_> = (0..6)
        .map(|frame_number| {
            let mut frame = evaluate(&plan, &[ScheduledItem(0)], frame_number)
                .expect("renderer fixture evaluates");
            frame.layers[0].source = EvaluatedSource::Spectrum2D {
                bands: vec![
                    (frame_number as f32 * 0.17).sin().abs(),
                    0.2 + frame_number as f32 * 0.03,
                    0.7 - frame_number as f32 * 0.04,
                    1.0,
                ],
                x: 0.0,
                y: 0.1,
                width: 1.0,
                height: 0.8,
                bar_gap_ratio: 0.08,
                min_bar_height_ratio: 0.10,
                layout: crate::project::Spectrum2DLayout::Radial(
                    crate::project::Spectrum2DRadialLayout {
                        inner_radius_ratio: 0.35,
                        start_angle_degrees: 450.0,
                        sweep_angle_degrees: 270.0,
                        direction: crate::project::Spectrum2DRadialDirection::Outward,
                        band_mapping: crate::project::Spectrum2DBandMapping::Reverse,
                    },
                ),
                gradient: Some((
                    crate::project::Spectrum2DGradientDirection::AcrossBands,
                    [20, 90, 180, 80],
                    [220, 40, 240, 220],
                )),
                colour: [40, 90, 180, 160],
            };
            frame.layers[0].effects = vec![EvaluatedEffect::Bloom {
                threshold: 0.4,
                radius: 2.0,
                intensity: 0.5,
            }];
            frame
        })
        .collect();
    let collect = |worker_count, decoded: Arc<DecodedAssets>| {
        let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, worker_count);
        let mut completed = Vec::new();
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                completed.push(
                    backend
                        .poll_completed(PollMode::WaitForOne)
                        .expect("poll")
                        .expect("completion"),
                );
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("submission");
        }
        while backend.in_flight() > 0 {
            completed.push(
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("drain poll")
                    .expect("drain completion"),
            );
        }
        completed.sort_by_key(|frame| frame.frame_number);
        completed
    };
    let one = collect(1, Arc::clone(&decoded));
    let two = collect(2, Arc::clone(&decoded));
    let four = collect(4, decoded);
    assert_eq!(one, two);
    assert_eq!(one, four);
}

#[test]
fn cancellation_does_not_block_on_cpu_completion() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame = evaluate(&plan, &[ScheduledItem(0)], 0).expect("renderer fixture evaluates");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    backend.submit_frame(0, &frame).expect("submission");
    let cancelled = AtomicBool::new(true);
    assert!(
        backend
            .poll_completed_cancellable(PollMode::WaitForOne, &cancelled)
            .expect("cancelled poll")
            .is_none()
    );
    backend.abort();
    backend.abort();
    assert!(backend.verify_idle().is_err());
}
