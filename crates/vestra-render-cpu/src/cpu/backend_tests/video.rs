use super::*;

#[test]
fn cpu_video_renderer_uses_layer_local_source_timing_end_to_end() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "fixture.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "fixture.mp4"}],
            "visual": {"clips": [{
                "id": "video-layer", "source": {"type": "video", "asset": "video"},
                "start": 0, "duration": 2, "layer": 0,
                "source_start": 0.25, "playback_rate": 0.5,
                "opacity": {"base_value": 1}
            }]}
        }"##,
    )
    .expect("video timing fixture parses");
    let asset_paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("fixture.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 2.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &asset_paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            2,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("video timing fixture compiles");
    let time = 1_000_000_000;
    let frame = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], time)
        .expect("renderer fixture evaluates");
    let requested_times = Arc::new(Mutex::new(Vec::new()));
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&requested_times),
        })),
    )
    .expect("video timing fixture decodes");
    let mut backend = CpuBackend::new(&plan, decoded);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("video timing fixture renders");

    assert_eq!(requested_times.lock().unwrap().last().copied(), Some(0.75));
    assert_eq!(output.get_pixel(0, 0), &image::Rgba([75, 0, 0, 255]));
    let first_pixels = output.clone();
    for time in [200_000_000, 700_000_000, 1_000_000_000] {
        let frame = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], time)
            .expect("renderer fixture evaluates");
        backend
            .render_frame(&frame, &mut output)
            .expect("random-access video frame renders");
    }
    assert_eq!(output, first_pixels);
}

#[test]
fn cpu_video_to_video_transition_advances_both_endpoints() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "video-transition.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [
                {"id": "red", "type": "video", "source": "red.mp4"},
                {"id": "blue", "type": "video", "source": "blue.mp4"}
            ],
            "visual": {
                "clips": [
                    {"id": "out", "source": {"type": "video", "asset": "red"},
                     "start": 0, "duration": 2, "layer": 0,
                     "source_start": 0, "playback_rate": 1,
                     "opacity": {"base_value": 1}},
                    {"id": "in", "source": {"type": "video", "asset": "blue"},
                     "start": 0, "duration": 2, "layer": 1,
                     "source_start": 1, "playback_rate": 2,
                     "opacity": {"base_value": 1}}
                ],
                "transitions": [{"id": "fade", "outgoing": "out", "incoming": "in",
                    "start": 0.5, "duration": 1.0,
                    "definition": {
                        "outgoing": {"opacity": {"keyframes": [
                            {"progress": 0, "value": 1, "interpolation": "linear"},
                            {"progress": 1, "value": 0, "interpolation": "linear"}
                        ]}},
                        "incoming": {"opacity": {"keyframes": [
                            {"progress": 0, "value": 0, "interpolation": "linear"},
                            {"progress": 1, "value": 1, "interpolation": "linear"}
                        ]}}
                    }}]
            }
        }"##,
    )
    .expect("Video transition fixture parses");
    let paths = std::collections::BTreeMap::from([
        ("red".to_owned(), std::path::PathBuf::from("red.mp4")),
        ("blue".to_owned(), std::path::PathBuf::from("blue.mp4")),
    ]);
    let durations =
        std::collections::BTreeMap::from([("red".to_owned(), 4.0), ("blue".to_owned(), 4.0)]);
    let dimensions =
        std::collections::BTreeMap::from([("red".to_owned(), (1, 1)), ("blue".to_owned(), (1, 1))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            2,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("Video transition fixture compiles");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(ColourVideoFactory {
            requests: Arc::clone(&requests),
        })),
    )
    .expect("Video transition fixture decodes");
    let frame =
        vestra_core::plan::evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 1_000_000_000)
            .expect("renderer fixture evaluates");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("Video transition fixture renders");

    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|(id, time)| id == "red" && *time == 1.0)
    );
    assert!(
        requests
            .iter()
            .any(|(id, time)| id == "blue" && *time == 3.0)
    );
    let pixel = output.get_pixel(0, 0);
    assert!(pixel[0] > 0 && pixel[2] > 0 && pixel[1] == 0);
}

#[test]
fn cpu_same_video_asset_can_render_two_source_times_in_one_frame() {
    let plan = same_video_asset_plan();
    let single_worker_requests = Arc::new(Mutex::new(Vec::new()));
    let multi_worker_requests = Arc::new(Mutex::new(Vec::new()));
    let single_worker_decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&single_worker_requests),
        })),
    )
    .expect("same-asset Video fixture decodes");
    let multi_worker_decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&multi_worker_requests),
        })),
    )
    .expect("same-asset Video fixture decodes for multiple workers");
    let frame =
        vestra_core::plan::evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 500_000_000)
            .expect("renderer fixture evaluates");
    let mut single_worker = CpuBackend::new_with_worker_count(&plan, single_worker_decoded, 1);
    let mut multi_worker = CpuBackend::new_with_worker_count(&plan, multi_worker_decoded, 2);
    let mut single_output = image::RgbaImage::new(1, 1);
    let mut multi_output = image::RgbaImage::new(1, 1);
    single_worker
        .render_frame(&frame, &mut single_output)
        .expect("same-asset Video fixture renders with one worker");
    multi_worker
        .render_frame(&frame, &mut multi_output)
        .expect("same-asset Video fixture renders with multiple workers");

    single_worker
        .render_frame(&frame, &mut single_output)
        .expect("repeated frame reuses independent source cursors");
    let stats = single_worker.stats();
    assert_eq!(
        stats.video_seek_count, 0,
        "source timelines must not alternate on one cursor"
    );
    assert_eq!(stats.video_decoder_session_count, 2);
    assert_eq!(stats.video_decoder_open_count, 2);
    assert_eq!(stats.video_cache_bytes_retained, 16);
    assert_eq!(stats.video_cache_bytes_peak, 24);
    assert_eq!(stats.video_current_frame_bytes_retained, 8);
    assert_eq!(stats.video_decoder_buffer_bytes_retained, 32);

    let single_requests = single_worker_requests.lock().unwrap();
    let multi_requests = multi_worker_requests.lock().unwrap();
    assert!(single_requests.contains(&0.5));
    assert!(single_requests.contains(&1.5));
    assert!(multi_requests.contains(&0.5));
    assert!(multi_requests.contains(&1.5));
    assert_eq!(single_output, multi_output);
    let pixel = single_output.get_pixel(0, 0);
    assert!(
        pixel[0] > 0 && pixel[2] > 0,
        "same-asset layers should both contribute, got {pixel:?}"
    );
    drop(single_requests);
    drop(multi_requests);

    // Serial clips reuse the primary asset cursor; only unused extra cursors
    // retire, with their work preserved in cumulative metrics.
    for item in [0, 1] {
        let frame = vestra_core::plan::evaluate(&plan, &[ScheduledItem(item)], 500_000_000)
            .expect("single active source evaluates");
        single_worker
            .render_frame(&frame, &mut single_output)
            .expect("single active source renders");
        let stats = single_worker.stats();
        assert_eq!(stats.video_decoder_session_count, 1);
        assert_eq!(stats.video_decoder_open_count, 2);
        assert_eq!(stats.video_cache_bytes_retained, 8);
        assert_eq!(stats.video_cache_bytes_peak, 24);
        assert_eq!(stats.video_current_frame_bytes_retained, 4);
        assert_eq!(stats.video_decoder_buffer_bytes_retained, 16);
    }
    assert_eq!(single_worker.stats().video_frame_requests, 6);
    let empty =
        vestra_core::plan::evaluate(&plan, &[], 500_000_000).expect("empty frame evaluates");
    single_worker
        .render_frame(&empty, &mut single_output)
        .expect("empty frame renders");
    assert_eq!(single_output.get_pixel(0, 0).0, [0, 0, 0, 0]);
    let stats = single_worker.stats();
    assert_eq!(stats.video_decoder_session_count, 1);
    assert_eq!(stats.video_frame_requests, 6);
    let frame = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], 500_000_000)
        .expect("reopened source evaluates");
    single_worker
        .render_frame(&frame, &mut single_output)
        .expect("source reopens");
    let stats = single_worker.stats();
    assert_eq!(stats.video_decoder_session_count, 1);
    assert_eq!(stats.video_decoder_open_count, 2);
    assert_eq!(stats.video_frame_requests, 7);

    let mut reversed = evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 500_000_000)
        .expect("reordered frame evaluates");
    reversed.layers.reverse();
    single_worker
        .render_frame(&reversed, &mut single_output)
        .expect("cursors handle reordered sources");
    let fresh_assets = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::new(Mutex::new(Vec::new())),
        })),
    )
    .expect("fresh reference prepares");
    let mut fresh = CpuBackend::new_with_worker_count(&plan, fresh_assets, 1);
    fresh
        .render_frame(&reversed, &mut multi_output)
        .expect("fresh reference renders");
    assert_eq!(single_output, multi_output);
    for layer in &mut reversed.layers {
        if let EvaluatedSource::Video { source_time, .. } = &mut layer.source {
            *source_time = 0.5;
        }
    }
    fresh
        .render_frame(&reversed, &mut multi_output)
        .expect("identical-time sources render");
    assert_eq!(
        fresh.stats().video_decoder_session_count,
        1,
        "identical asset/time reads share the primary cursor"
    );
}

#[test]
fn cpu_video_reopen_failure_fails_the_frame() {
    #[derive(Default)]
    struct Factory {
        opens: std::sync::atomic::AtomicUsize,
        budgets: Mutex<Vec<u64>>,
    }
    impl crate::VideoDecoderFactory for Factory {
        fn open(
            &self,
            _asset: &vestra_core::plan::VideoAsset,
            cache_budget_bytes: u64,
        ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
            self.budgets.lock().unwrap().push(cache_budget_bytes);
            if self.opens.fetch_add(1, Ordering::Relaxed) > 1 {
                return Err("mock video reopen failed".to_owned());
            }
            Ok(Box::new(VideoFixtureSession {
                frame: crate::VideoFrame {
                    pts: 0,
                    pixels: Arc::new(image::RgbaImage::from_pixel(
                        1,
                        1,
                        image::Rgba([255, 0, 0, 255]),
                    )),
                },
            }))
        }
    }
    let plan = same_video_asset_plan();
    let factory = Arc::new(Factory::default());
    let decoded = DecodedAssets::build_with_video_factory(&plan, Some(factory.clone()))
        .expect("video fixture prepares");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let frame = evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 500_000_000)
        .expect("video evaluates");
    let empty = evaluate(&plan, &[], 500_000_000).expect("empty frame evaluates");
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("initial open succeeds");
    backend
        .render_frame(&empty, &mut output)
        .expect("idle video retires");
    let error = backend
        .render_frame(&frame, &mut output)
        .expect_err("reopen failure must not omit video silently");
    assert!(error.message.contains("mock video reopen failed"));
    let budgets = factory.budgets.lock().unwrap();
    assert!(budgets[0] > 0, "primary cursor keeps its cache allowance");
    assert!(
        budgets.iter().sum::<u64>() <= backend.worker_cache_budgets[0].video_cache_budget_bytes
    );
}

#[test]
fn cpu_nested_video_renderer_uses_nested_local_time() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "nested-video.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "video.mp4"}],
            "visual": {"clips": [{
                "id": "group", "source": {"type": "group", "clips": [{
                    "id": "child", "source": {"type": "video", "asset": "video"},
                    "start": 0.25, "duration": 1.0, "layer": 0,
                    "source_start": 0.1, "playback_rate": 2,
                    "opacity": {"base_value": 1}
                }]}, "start": 0.5, "duration": 2.0, "layer": 0,
                "opacity": {"base_value": 1}
            }]}
        }"##,
    )
    .expect("nested Video fixture parses");
    let paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("video.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 4.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            2,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("nested Video fixture compiles");
    let requested_times = Arc::new(Mutex::new(Vec::new()));
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&requested_times),
        })),
    )
    .expect("nested Video fixture decodes");
    let frame = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], 1_000_000_000)
        .expect("renderer fixture evaluates");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("nested Video fixture renders");

    assert_eq!(requested_times.lock().unwrap().last().copied(), Some(0.6));
    assert_eq!(output.get_pixel(0, 0), &image::Rgba([60, 0, 0, 255]));
}

#[test]
fn cpu_video_renderer_samples_the_authored_crop_from_a_dynamic_frame() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "fixture.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "fixture.mp4"}],
            "visual": {"clips": [{
                "id": "video-layer", "source": {"type": "video", "asset": "video"},
                "start": 0, "duration": 1, "layer": 0,
                "opacity": {"base_value": 1},
                "crop": {"base_value": {"x": 0.5, "y": 0, "width": 0.5, "height": 1}}
            }]}
        }"##,
    )
    .expect("video fixture project parses");
    let asset_paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("fixture.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 1.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (2, 1))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &asset_paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            1,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("video fixture compiles");
    let frame = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], 0)
        .expect("renderer fixture evaluates");
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(std::sync::Arc::new(VideoFixtureFactory {
            frame: crate::VideoFrame {
                pts: 0,
                pixels: std::sync::Arc::new(image::RgbaImage::from_fn(2, 1, |x, _| {
                    if x == 0 {
                        image::Rgba([255, 0, 0, 255])
                    } else {
                        image::Rgba([0, 0, 255, 255])
                    }
                })),
            },
        })),
    )
    .expect("video fixture decodes");
    let mut backend = CpuBackend::new(&plan, decoded);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("video fixture renders");
    assert_eq!(output.get_pixel(0, 0), &image::Rgba([0, 0, 255, 255]));
}

#[test]
fn cpu_mixed_static_and_video_sources_render_through_one_layer_pipeline() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "mixed.mp4", "width": 4, "height": 4,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [
                {"id": "image", "type": "image", "source": "red.png"},
                {"id": "video", "type": "video", "source": "fixture.mp4"},
                {"id": "font", "type": "font", "source": "VestraTest-Regular.ttf"}
            ],
            "visual": {"clips": [
                {"id": "image", "source": {"type": "image", "asset": "image"},
                 "start": 0, "duration": 1, "layer": 0,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 1}},
                {"id": "shape", "source": {"type": "shape",
                 "geometry": {"type": "rectangle", "width": 2, "height": 2},
                 "fill": "#00ff00"}, "start": 0, "duration": 1, "layer": 1,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 1}},
                {"id": "text", "source": {"type": "text", "text": "A",
                 "font": "font", "font_size": 2, "fill": "#ffffff"},
                 "start": 0, "duration": 1, "layer": 2,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 1}},
                {"id": "video", "source": {"type": "video", "asset": "video"},
                 "start": 0, "duration": 1, "layer": 3,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 0.5}}
            ]}
        }"##,
    )
    .expect("mixed source fixture parses");
    let image_path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixtures/red.png");
    let asset_paths = std::collections::BTreeMap::from([
        ("image".to_owned(), image_path),
        ("video".to_owned(), std::path::PathBuf::from("fixture.mp4")),
        (
            "font".to_owned(),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/assets/VestraTest-Regular.ttf"),
        ),
    ]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 1.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &asset_paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            1,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("mixed source fixture compiles");
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(VideoFixtureFactory {
            frame: crate::VideoFrame {
                pts: 0,
                pixels: Arc::new(image::RgbaImage::from_pixel(
                    1,
                    1,
                    image::Rgba([0, 0, 255, 255]),
                )),
            },
        })),
    )
    .expect("mixed source fixture decodes");
    let frame = vestra_core::plan::evaluate(&plan, &[ScheduledItem(0)], 0)
        .expect("renderer fixture evaluates");
    let mut backend = CpuBackend::new(&plan, decoded);
    let mut output = image::RgbaImage::new(4, 4);
    backend
        .render_frame(&frame, &mut output)
        .expect("mixed source fixture renders");

    assert!(output.pixels().any(|pixel| pixel[1] > 0));
    assert!(output.pixels().any(|pixel| pixel[2] > 0));
    assert!(output.pixels().any(|pixel| pixel[0] > 0));
}

#[test]
fn cache_budget_partition_covers_static_crop_video_and_tiny_cases() {
    assert_eq!(
        cache_class_budgets_for(100, true, false, false),
        CacheClassBudgets {
            static_layers: 100,
            crop: 0,
            video: 0,
        }
    );
    assert_eq!(
        cache_class_budgets_for(100, true, true, false),
        CacheClassBudgets {
            static_layers: 50,
            crop: 50,
            video: 0,
        }
    );
    assert_eq!(
        cache_class_budgets_for(100, false, false, true),
        CacheClassBudgets {
            static_layers: 0,
            crop: 0,
            video: 100,
        }
    );
    assert_eq!(
        cache_class_budgets_for(101, true, true, true),
        CacheClassBudgets {
            static_layers: 35,
            crop: 33,
            video: 33,
        }
    );
    assert_eq!(
        cache_class_budgets_for(2, true, true, true),
        CacheClassBudgets {
            static_layers: 2,
            crop: 0,
            video: 0,
        }
    );
}
