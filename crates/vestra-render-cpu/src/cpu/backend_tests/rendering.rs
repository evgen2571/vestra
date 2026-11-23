use super::*;

#[test]
fn spectrum2d_uses_normal_opacity_and_bloom_pipeline() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("fixture validates");
    let plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let spectrum = EvaluatedSource::Spectrum2D {
        wave: None,
        bands: vec![1.0],
        x: 0.0,
        y: 0.0,
        width: 0.25,
        height: 1.0,
        bar_gap_ratio: 0.0,
        min_bar_height_ratio: 0.0,
        layout: crate::project::Spectrum2DLayout::default(),
        gradient: None,
        colour: [255, 255, 255, 255],
    };
    let frame = EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 0],
        width: 8,
        height: 8,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 99,
            visible: true,
            content_dependency: TemporalDependency::Dynamic,
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
            source: spectrum,
            opacity: 1.0,
            effects: Vec::new(),
            masks: Vec::new(),
            matte: None,
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    };
    let mut backend = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    backend
        .submit_frame(0, &frame)
        .expect("spectrum submission");
    let without_bloom = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("spectrum poll")
        .expect("spectrum completion");
    let mut bloom_frame = frame.clone();
    bloom_frame.layers[0].effects = vec![EvaluatedEffect::Bloom {
        threshold: 0.1,
        radius: 2.0,
        intensity: 1.0,
    }];
    backend
        .submit_frame(1, &bloom_frame)
        .expect("bloom submission");
    let with_bloom = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("bloom poll")
        .expect("bloom completion");
    assert_eq!(&without_bloom.rgba[2 * 4..3 * 4], &[0, 0, 0, 0]);
    assert!(with_bloom.rgba[2 * 4] > 0);
    assert!(with_bloom.rgba[2 * 4 + 3] > 0);
    assert!(with_bloom.rgba[0..4].iter().any(|value| *value > 0));
}

#[test]
fn preview_preserves_authored_shape_geometry() {
    let project: vestra_core::project::Project = serde_json::from_value(serde_json::json!({
        "schema_version": 1,
        "output": {"path": "resolution.mp4", "width": 1280, "height": 720,
            "frame_rate": "24/1", "background": "#000000", "quality": "preview", "audio": false, "duration_mode": "explicit", "duration": 1.0},
        "assets": [],
        "visual": {"clips": [{"id": "shape", "source": {"type": "shape",
            "geometry": {"type": "rectangle", "width": 640, "height": 360}, "fill": "#ff0000"},
            "start": 0, "duration": 1, "layer": 0, "opacity": {"base_value": 1}}]}
    }))
    .expect("shape fixture");
    let assets = std::collections::BTreeMap::new();
    let durations = std::collections::BTreeMap::new();
    let plan = compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &assets,
            &durations,
            1.0,
            (24, 1),
            24,
            &[],
        ),
        CompileOptions {
            preview: true,
            ..CompileOptions::default()
        },
    )
    .expect("compile");
    let decoded = DecodedAssets::build(&plan).expect("shape raster");
    let frame = evaluate(&plan, &[ScheduledItem(0)], 0).expect("evaluate");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut output = image::RgbaImage::new(frame.width, frame.height);
    backend.render_frame(&frame, &mut output).expect("render");
    assert_eq!(output.dimensions(), (640, 360));
    assert_eq!(output.get_pixel(80, 180).0, [0, 0, 0, 255]);
    assert_eq!(output.get_pixel(320, 180).0, [255, 0, 0, 255]);
}
