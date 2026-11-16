use super::*;

#[test]
fn cpu_frame_render_timing_comes_from_backend_work_duration() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("timing.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    };
    let summary = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        2,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .with_cpu_backend()
                    .with_frame_render_work_duration(Duration::from_millis(18)),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("mock CPU render succeeds");
    assert_eq!(summary.timings.frame_render_ms, 18);
}

#[test]
fn wgpu_frame_request_uses_the_shared_staged_completion_contract() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let written = Arc::new(Mutex::new(Vec::new()));
    let backend_written = Arc::clone(&written);
    let mut prepared = prepare(
        plan.clone(),
        RenderBackendPreference::Wgpu,
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    1,
                    (0..total_frames).collect(),
                    backend_written,
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect("prepared WGPU seam");
    let frame = prepared.render_frame(0).expect("WGPU frame completes");
    assert_eq!(frame.frame_number, 0);
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(workspace.path().join("still-usable.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("video remains usable");
}

#[test]
fn prepared_state_reuses_one_backend_for_two_video_operations() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let backend_creations = Arc::new(AtomicUsize::new(0));
    let creation_counter = Arc::clone(&backend_creations);
    let mut prepared = prepare(
        plan.clone(),
        RenderBackendPreference::Wgpu,
        move |_, _, _| {
            creation_counter.fetch_add(1, Ordering::Relaxed);
            Ok((
                Box::new(MockStagedBackend::new(
                    3,
                    (0..total_frames).collect(),
                    Arc::new(Mutex::new(Vec::new())),
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect("one visual preparation succeeds");

    for name in ["first.mp4", "second.mp4"] {
        let options = RenderOptions {
            frame_range: None,
            output_override: Some(workspace.path().join(name)),
            overwrite: true,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Wgpu,
        };
        let result = render_prepared_with_sink(
            &mut prepared,
            &options,
            &mut |_| RenderObserverControl::Continue,
            |_settings: &EncoderSettings, temporary_path| {
                Ok(RecordingSink::new(
                    temporary_path.to_path_buf(),
                    SinkProbe::default(),
                ))
            },
        )
        .expect("prepared state renders a complete video");
        assert_eq!(result.performance.submitted_frames, total_frames);
        assert_eq!(result.performance.backend_completed_frames, total_frames);
        assert_eq!(result.performance.written_frames_staged, total_frames);
        assert!(workspace.path().join(name).exists());
    }
    assert_eq!(backend_creations.load(Ordering::Relaxed), 1);
}

#[test]
fn prepared_visual_snapshot_ignores_later_source_file_changes() {
    let workspace = tempfile::tempdir().expect("temporary project directory");
    let projects = workspace.path().join("examples/projects");
    let assets = workspace.path().join("assets/fixtures");
    fs::create_dir_all(&projects).expect("project directory");
    fs::create_dir_all(&assets).expect("asset directory");
    let manifest = projects.join("project.json");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/projects/animation-effects.json"),
        &manifest,
    )
    .expect("copy project fixture");
    let source_assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixtures");
    fs::copy(source_assets.join("red.png"), assets.join("red.png")).expect("copy red image");
    fs::copy(source_assets.join("blue.png"), assets.join("blue.png")).expect("copy blue image");

    let initial =
        crate::project::load_and_validate(&manifest, &crate::project::ValidationOptions::default())
            .expect("initial project validates");
    let initial_plan = crate::plan::compile(&initial, crate::plan::CompileOptions::default())
        .expect("initial plan compiles");
    let mut prepared = prepare(
        &initial_plan,
        RenderBackendPreference::Cpu,
        |_, plan, decoded| {
            Ok((
                Box::new(CpuBackend::new(plan, Arc::clone(decoded))) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect("initial visual preparation succeeds");

    let render_pixels = |prepared: &mut _, output: PathBuf| {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let sink_pixels = Arc::clone(&captured);
        let options = RenderOptions {
            frame_range: None,
            output_override: Some(output),
            overwrite: true,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Cpu,
        };
        render_prepared_with_sink(
            prepared,
            &options,
            &mut |_| RenderObserverControl::Continue,
            move |_settings: &EncoderSettings, temporary_path| {
                Ok(PixelSink {
                    frames: sink_pixels,
                    temporary_path: temporary_path.to_path_buf(),
                })
            },
        )
        .expect("render succeeds");
        captured.lock().expect("pixel sink lock").clone()
    };
    let before_change = render_pixels(&mut prepared, workspace.path().join("before.mp4"));

    fs::copy(assets.join("blue.png"), assets.join("red.png")).expect("replace source image");
    let retained_snapshot = render_pixels(&mut prepared, workspace.path().join("retained.mp4"));
    assert_eq!(retained_snapshot, before_change);

    let changed =
        crate::project::load_and_validate(&manifest, &crate::project::ValidationOptions::default())
            .expect("changed project validates");
    let changed_plan = crate::plan::compile(&changed, crate::plan::CompileOptions::default())
        .expect("changed plan compiles");
    let mut changed_prepared = prepare(
        &changed_plan,
        RenderBackendPreference::Cpu,
        |_, plan, decoded| {
            Ok((
                Box::new(CpuBackend::new(plan, Arc::clone(decoded))) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect("changed visual preparation succeeds");
    let new_snapshot = render_pixels(&mut changed_prepared, workspace.path().join("changed.mp4"));
    assert_ne!(new_snapshot, before_change);
}

#[test]
fn preparation_fallback_is_retained_on_successful_render() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("fallback-success.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Auto,
    };
    let result = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    3,
                    (0..total_frames).collect(),
                    Arc::new(Mutex::new(Vec::new())),
                )) as Box<dyn RenderBackend>,
                Some(BackendFallback {
                    code: "WGPU-PIPELINE-CREATION".to_owned(),
                    stage: "wgpu_preparation".to_owned(),
                    message: "injected pipeline preparation failure".to_owned(),
                }),
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("CPU fallback render succeeds");

    assert!(matches!(
        result.backend_fallback,
        Some(BackendFallback { code, .. }) if code == "WGPU-PIPELINE-CREATION"
    ));
}
