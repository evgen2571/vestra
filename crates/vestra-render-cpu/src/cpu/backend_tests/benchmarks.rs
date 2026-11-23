use super::*;

#[test]
#[ignore = "manual release CPU Video worker-count benchmark"]
fn cpu_video_worker_count_benchmark() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "fixture.mp4", "width": 320, "height": 180,
                "frame_rate": "30/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "fixture.mp4"}],
            "visual": {"clips": [{
                "id": "video-layer", "source": {"type": "video", "asset": "video"},
                "start": 0, "duration": 4, "layer": 0,
                "source_start": 0, "playback_rate": 1,
                "opacity": {"base_value": 1}
            }]}
        }"##,
    )
    .expect("benchmark project parses");
    let paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("fixture.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 4.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (320, 180))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (320, 180),
            2,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("benchmark project compiles");
    let frames = (0_u64..120)
        .map(|number| {
            vestra_core::plan::evaluate(
                &plan,
                &[ScheduledItem(0)],
                u128::from(number) * 1_000_000_000 / 30,
            )
            .expect("renderer fixture evaluates")
        })
        .collect::<Vec<_>>();

    for worker_count in [1, 2, 4, 8] {
        let decoded = DecodedAssets::build_with_video_factory(
            &plan,
            Some(Arc::new(WorkerBenchmarkVideoFactory)),
        )
        .expect("benchmark assets decode");
        let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, worker_count);
        let started = std::time::Instant::now();
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("benchmark poll")
                    .expect("benchmark completion");
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("benchmark submission");
        }
        while backend.in_flight() > 0 {
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("benchmark drain poll")
                .expect("benchmark drain completion");
        }
        let wall_ms = started.elapsed().as_secs_f64() * 1_000.0;
        let stats = backend.stats();
        let staged = backend.staged_metrics();
        println!(
            "cpu_video_worker_benchmark workers={worker_count} wall_ms={wall_ms:.3} frame_render_ms={:.3} sessions={} requests={} actual_decodes={} cache_hits={} cache_misses={} decode_ms={:.3}",
            staged.frame_render_work_duration.as_secs_f64() * 1_000.0,
            stats.video_decoder_session_count,
            stats.video_frame_requests,
            stats.video_actual_decodes,
            stats.video_cache_hits,
            stats.video_cache_misses,
            stats.video_decode_time_us as f64 / 1_000.0,
        );
    }
}

#[test]
#[ignore = "manual release CPU Spectrum2D benchmark"]
fn spectrum2d_cpu_benchmark() {
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
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let available = thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);

    let automatic = automatic_worker_count(&plan, available);
    let cases = [
        (
            "linear_24",
            24,
            1,
            crate::project::Spectrum2DLayout::default(),
            None,
            false,
        ),
        (
            "linear_24_auto",
            24,
            automatic,
            crate::project::Spectrum2DLayout::default(),
            None,
            false,
        ),
        (
            "center_out_24_auto",
            24,
            automatic,
            crate::project::Spectrum2DLayout::Linear(crate::project::Spectrum2DLinearLayout {
                anchor: crate::project::Spectrum2DLinearAnchor::Bottom,
                band_mapping: crate::project::Spectrum2DBandMapping::CenterOut,
            }),
            None,
            false,
        ),
        (
            "radial_24",
            24,
            1,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "radial_24_auto",
            24,
            automatic,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "radial_48",
            48,
            1,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "radial_48_auto",
            48,
            automatic,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "neon_circle_48_auto",
            48,
            automatic,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            Some((
                crate::project::Spectrum2DGradientDirection::AcrossBands,
                [0, 255, 255, 255],
                [255, 0, 255, 255],
            )),
            true,
        ),
    ];

    for (name, band_count, worker_count, layout, gradient, neon_circle) in cases {
        let frames: Vec<_> = (0..24)
            .map(|frame_number| {
                let mut frame = evaluate(&plan, &[ScheduledItem(0)], frame_number)
                    .expect("renderer fixture evaluates");
                if let EvaluatedSource::Image { .. } = frame.layers[0].source {
                    frame.layers[0].source = EvaluatedSource::Spectrum2D {
                        wave: None,
                        bands: (0..band_count)
                            .map(|band| {
                                (((band + frame_number as usize) % band_count) as f32
                                    / band_count as f32)
                                    .max(0.05)
                            })
                            .collect(),
                        x: if neon_circle { 0.20 } else { 0.0 },
                        y: if neon_circle { 0.20 } else { 0.1 },
                        width: if neon_circle { 0.60 } else { 1.0 },
                        height: if neon_circle { 0.60 } else { 0.8 },
                        bar_gap_ratio: if neon_circle { 0.12 } else { 0.1 },
                        min_bar_height_ratio: if neon_circle { 0.035 } else { 0.0 },
                        layout: layout.clone(),
                        gradient,
                        colour: [255, 255, 255, 255],
                    };
                }
                if neon_circle {
                    frame.layers[0].effects = vec![
                        EvaluatedEffect::Glow {
                            threshold: 0.35,
                            radius: 3.0,
                            intensity: 0.85,
                            colour: [255, 255, 255, 255],
                        },
                        EvaluatedEffect::Bloom {
                            threshold: 0.55,
                            radius: 4.0,
                            intensity: 0.65,
                        },
                    ];
                }
                frame
            })
            .collect();
        let mut backend =
            CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), worker_count);
        let started = Instant::now();
        for (frame_number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("benchmark poll")
                    .expect("benchmark completion");
            }
            backend
                .submit_frame(frame_number as u64, frame)
                .expect("benchmark submission");
        }
        while backend.in_flight() > 0 {
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("benchmark drain poll")
                .expect("benchmark drain completion");
        }
        let elapsed = started.elapsed();
        println!(
            "spectrum2d_cpu case={} workers={} resolution={}x{} analysis_bands={} displayed_bars={} frames={} wall_ms={:.3} effective_fps={:.2} effect_bundle={} gradient={} release=true renderer_only=true",
            name,
            worker_count,
            plan.canvas.width,
            plan.canvas.height,
            band_count,
            if matches!(layout, crate::project::Spectrum2DLayout::Linear(value) if matches!(value.band_mapping, crate::project::Spectrum2DBandMapping::CenterOut))
            {
                band_count * 2
            } else {
                band_count
            },
            frames.len(),
            elapsed.as_secs_f64() * 1_000.0,
            frames.len() as f64 / elapsed.as_secs_f64(),
            if neon_circle { "glow+bloom" } else { "none" },
            if gradient.is_some() {
                "enabled"
            } else {
                "solid"
            },
        );
    }
}

#[test]
#[ignore = "manual release CPU scaling benchmark"]
fn cpu_parallel_scaling_benchmark() {
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
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame_count = plan.frame_count.min(60);
    let frames: Vec<_> = (0..frame_count)
        .map(|number| {
            evaluate(&plan, &[ScheduledItem(0)], u128::from(number))
                .expect("renderer fixture evaluates")
        })
        .collect();
    let automatic = automatic_worker_count(
        &plan,
        thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
    );

    for (label, worker_count) in [("1", 1), ("2", 2), ("4", 4), ("auto", automatic)] {
        let mut backend = if label == "auto" {
            CpuBackend::new(&plan, Arc::clone(&decoded))
        } else {
            CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), worker_count)
        };
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("warmup poll")
                    .expect("warmup completion");
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("warmup submission");
        }
        while backend.in_flight() > 0 {
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("warmup drain poll")
                .expect("warmup drain completion");
        }
        backend.reset_operation_metrics();
        let started = Instant::now();
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("measured poll")
                    .expect("measured completion");
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("measured submission");
        }
        while backend.in_flight() > 0 {
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("measured drain poll")
                .expect("measured drain completion");
        }
        let elapsed = started.elapsed();
        let stats = backend.stats();
        let staged = backend.staged_metrics();
        let effective_fps = frame_count as f64 / elapsed.as_secs_f64();
        println!(
            "cpu_parallel_scaling workers={label} actual_workers={} resolution={}x{} frames={} wall_ms={:.3} effective_fps={effective_fps:.2} aggregate_frame_render_ms={} peak_in_flight={} cache_current_bytes={} cache_peak_bytes={} scratch_retained_bytes={}",
            backend.capacity(),
            plan.canvas.width,
            plan.canvas.height,
            frame_count,
            elapsed.as_secs_f64() * 1_000.0,
            staged.frame_render_work_duration.as_secs_f64() * 1_000.0,
            staged.peak_frames_in_flight,
            stats.cache_current_bytes,
            stats.cache_peak_bytes,
            stats.cpu_scratch_bytes_retained,
        );
    }
}
