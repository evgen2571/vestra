use super::*;

#[test]
fn selected_range_submits_absolute_frames_and_writes_only_rebased_pixels() {
    for static_visual in [false, true] {
        let mut plan = super::example_plan();
        if static_visual {
            plan.visual_dependency = TemporalDependency::Static;
        }
        let directory = tempfile::tempdir().unwrap();
        let written = Arc::new(Mutex::new(Vec::new()));
        let backend_written = Arc::clone(&written);
        let pixels = Arc::new(Mutex::new(Vec::new()));
        let sink_pixels = Arc::clone(&pixels);
        let options = RenderOptions {
            frame_range: Some(3..6),
            output_override: Some(directory.path().join("selected.mp4")),
            overwrite: false,
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            backend_preference: RenderBackendPreference::Wgpu,
        };
        let summary = render_with_backend_builder_and_sink(
            &plan,
            &options,
            &mut |_| RenderObserverControl::Continue,
            |_, _, _| {
                Ok((
                    Box::new(MockStagedBackend::new(
                        3,
                        if static_visual {
                            vec![3]
                        } else {
                            vec![5, 3, 4]
                        },
                        backend_written,
                    )) as Box<dyn RenderBackend>,
                    None,
                ))
            },
            |settings, path| {
                assert_eq!(settings.frame_count, 3);
                let seconds = plan.frame_rate.1 as f64 / plan.frame_rate.0 as f64;
                assert_eq!(settings.audio_range, Some(3.0 * seconds..6.0 * seconds));
                Ok(PixelSink {
                    frames: sink_pixels,
                    temporary_path: path.to_owned(),
                })
            },
        )
        .expect("selected render");
        assert_eq!(summary.frame_count, 3);
        assert_eq!(
            summary.performance.submitted_frames,
            if static_visual { 1 } else { 3 }
        );
        assert_eq!(*written.lock().unwrap(), vec![3, 4, 5]);
        assert_eq!(
            pixels
                .lock()
                .unwrap()
                .iter()
                .map(|frame| frame[0])
                .collect::<Vec<_>>(),
            if static_visual {
                vec![3; 3]
            } else {
                vec![3, 4, 5]
            }
        );
    }
}

#[test]
fn selected_range_raw_cpu_pixels_match_full_render_and_allow_another_operation() {
    let plan = super::example_plan();
    let directory = tempfile::tempdir().unwrap();
    let mut prepared = prepare(&plan, RenderBackendPreference::Cpu, |_, plan, decoded| {
        Ok((
            Box::new(CpuBackend::new(plan, Arc::clone(decoded))) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .unwrap();
    let mut render = |range: Option<std::ops::Range<u64>>, name: &str| {
        let pixels = Arc::new(Mutex::new(Vec::new()));
        let sink_pixels = Arc::clone(&pixels);
        render_prepared_with_sink(
            &mut prepared,
            &RenderOptions {
                frame_range: range,
                output_override: Some(directory.path().join(name)),
                overwrite: false,
                cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                backend_preference: RenderBackendPreference::Cpu,
            },
            &mut |_| RenderObserverControl::Continue,
            |_, path| {
                Ok(PixelSink {
                    frames: sink_pixels,
                    temporary_path: path.to_owned(),
                })
            },
        )
        .unwrap();
        Arc::try_unwrap(pixels).unwrap().into_inner().unwrap()
    };
    let full = render(None, "full.mp4");
    let range = 21..29;
    assert_eq!(render(Some(range.clone()), "clip.mp4"), full[21..29]);
    assert_eq!(render(Some(0..2), "beginning.mp4"), full[0..2]);
}

#[test]
fn engine_writes_out_of_order_mock_completions_in_frame_order() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let written = Arc::new(Mutex::new(Vec::new()));
    let order = (0..plan.frame_count)
        .collect::<Vec<_>>()
        .chunks(3)
        .flat_map(|chunk| chunk.iter().rev().copied())
        .collect::<Vec<_>>();
    let backend_written = Arc::clone(&written);
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output_dir.path().join("mock.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let sink_frames = Arc::new(Mutex::new(Vec::new()));
    let sink_probe = SinkProbe {
        frames: Arc::clone(&sink_frames),
        ..SinkProbe::default()
    };
    let result = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(3, order, backend_written))
                    as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(temporary_path.to_path_buf(), sink_probe))
        },
    );
    result.expect("mock staged render succeeds");
    assert_eq!(
        *written.lock().expect("mock metrics lock"),
        (0..plan.frame_count).collect::<Vec<_>>()
    );
    assert_eq!(
        *sink_frames.lock().expect("sink lock"),
        (0..plan.frame_count).collect::<Vec<_>>()
    );
}

#[test]
fn engine_rejects_duplicate_mock_completion() {
    assert_eq!(
        run_failure_case(MockMode::DuplicateCompletion)
            .diagnostic
            .code,
        "VESTRA-DUPLICATE-FRAME"
    );
}
