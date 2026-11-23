use super::*;

#[test]
fn preparation_routes_audible_master_response_curve_to_brightness_at_global_time() {
    let mut plan = super::example_plan();
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let source = directory.path().join("audible-master.wav");
    let source_frames =
        usize::try_from(vestra_media::seconds_to_samples(plan.duration).expect("fixture duration"))
            .expect("fixture sample count fits usize");
    write_stepped_tone_wav(&source, source_frames);

    let band = AudioFrequencyBand::new(40.0, 160.0).expect("valid band");
    let signal = CompiledScalarSignal::new(
        vestra_core::plan::RawScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(band),
        }),
        vec![
            CompiledSignalTransform::Gain(GainTransform::new(1.0).expect("finite gain")),
            CompiledSignalTransform::Remap(
                RemapTransform::new(0.0, 1.0, 0.0, 1.0).expect("valid remap"),
            ),
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).expect("valid clamp")),
            CompiledSignalTransform::Envelope(EnvelopeTransform::new(180_000_000, 180_000_000)),
            CompiledSignalTransform::ResponseCurve(
                CubicResponseCurve::new(0.42, 0.0, 1.0, 1.0).expect("valid response curve"),
            ),
        ],
    );
    plan.scalar_signals = CompiledScalarSignals::from_signals(vec![signal]);
    plan.audio_analysis_requirements =
        AudioAnalysisRequirements::from_requirements([AudioAnalysisRequirement::Master(
            AudioScalarFeature::BandEnergy(band),
        )]);
    plan.audio_output_enabled = false;
    plan.encoder.audio_mix = None;
    let signal_id = plan.scalar_signals.iter().next().expect("signal ID").0;
    let layer = &mut plan.layers[0];
    layer.duration_nanos =
        vestra_core::plan_time::to_nanos(2.0, "test layer duration").expect("valid layer duration");
    let layer_start =
        vestra_core::plan_time::to_nanos(4.0, "test layer start").expect("valid layer start");
    let layer_end = layer_start
        .checked_add(layer.duration_nanos)
        .expect("test layer end fits timeline");
    layer.start_nanos = layer_start;
    layer.start_frame =
        vestra_core::plan_time::first_frame_at_or_after(layer_start, plan.frame_rate)
            .expect("valid start frame");
    layer.end_frame = vestra_core::plan_time::first_frame_at_or_after(layer_end, plan.frame_rate)
        .expect("valid end frame");
    layer.draw_key.start_nanos = layer_start;
    layer.effects.push(TimedEffect {
        start: 0,
        end: layer.duration_nanos,
        effect: CompiledEffect::Brightness {
            amount: CompiledScalarProperty {
                authored_track: Track {
                    base_value: 0.0,
                    keyframes: vec![Keyframe {
                        time: 1_000_000_000,
                        value: 0.25,
                        interpolation: Interpolation::Linear,
                    }],
                },
                modifiers: vec![CompiledScalarModifier {
                    operation: ScalarModifierOperation::Add,
                    signal: signal_id,
                }],
                constraint: ScalarPropertyConstraint::Finite,
            },
        },
        dependency: TemporalDependency::Dynamic,
    });
    plan.audio_mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "audible".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![AudioClipPlan {
                id: "audible-clip".to_owned(),
                asset: "audible".to_owned(),
                path: source,
                start: 0.0,
                trim_start: 0.0,
                selected_duration: plan.duration,
                processed_duration: plan.duration,
                mute: false,
                gain: 1.0,
                gain_automation: None,
                fade_in: 0.0,
                fade_out: 0.0,
                fade_in_curve: AudioFadeCurve::Linear,
                fade_out_curve: AudioFadeCurve::Linear,

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],

        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let prepared = prepare(&plan, RenderBackendPreference::Wgpu, |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                1,
                vec![],
                Arc::new(Mutex::new(Vec::new())),
            )),
            None,
        ))
    })
    .expect("audible analysis prepares even when output mux audio is disabled");
    let project_time =
        vestra_core::plan_time::to_nanos(5.0, "test project time").expect("valid project time");
    let frame_number =
        vestra_core::plan_time::first_frame_at_or_after(project_time, plan.frame_rate)
            .expect("valid project frame");
    let active = ActiveSchedule::compile(&plan).active_at(&plan, frame_number);
    let frame = evaluate_with_context(
        &plan,
        &active,
        project_time,
        &EvaluationContext::new(prepared.scalar_signals()),
    )
    .expect("prepared signal evaluates at project time");
    let signal_value = prepared
        .scalar_signals()
        .get(signal_id)
        .expect("prepared signal")
        .sample(project_time);
    let layer = frame
        .layers
        .iter()
        .find(|layer| layer.compiled_layer_index == 0)
        .expect("staged layer is active");
    let amount = match layer.effects.last() {
        Some(vestra_core::plan::EvaluatedEffect::Brightness { amount }) => *amount,
        effect => panic!("expected brightness effect, found {effect:?}"),
    };
    assert!((amount - (0.25 + signal_value)).abs() < 1e-12);
}

#[test]
fn preparation_evaluates_bass_scale_rms_glow_and_high_band_chromatic_reference_targets() {
    let mut plan = super::example_plan();
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let source = directory.path().join("reference-master.wav");
    let source_frames =
        usize::try_from(vestra_media::seconds_to_samples(plan.duration).expect("fixture duration"))
            .expect("fixture sample count fits usize");
    write_stepped_multiband_wav(&source, source_frames);

    let bass_band = AudioFrequencyBand::new(40.0, 160.0).expect("valid bass band");
    let high_band = AudioFrequencyBand::new(2_000.0, 12_000.0).expect("valid high band");
    let bass = CompiledScalarSignal::new(
        vestra_core::plan::RawScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(bass_band),
        }),
        vec![
            CompiledSignalTransform::Gain(GainTransform::new(20.0).expect("finite gain")),
            CompiledSignalTransform::Remap(
                RemapTransform::new(0.0, 1.0, 0.0, 1.0).expect("valid remap"),
            ),
            CompiledSignalTransform::Clamp(ClampTransform::new(0.0, 1.0).expect("valid clamp")),
            CompiledSignalTransform::Envelope(EnvelopeTransform::new(20_000_000, 180_000_000)),
            CompiledSignalTransform::ResponseCurve(
                CubicResponseCurve::new(0.42, 0.0, 0.58, 1.0).expect("valid curve"),
            ),
            CompiledSignalTransform::Remap(
                RemapTransform::new(0.0, 1.0, 1.0, 1.08).expect("valid scale remap"),
            ),
        ],
    );
    let rms = CompiledScalarSignal::new(
        vestra_core::plan::RawScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::Rms,
        }),
        vec![CompiledSignalTransform::Gain(
            GainTransform::new(30.0).expect("finite gain"),
        )],
    );
    let high = CompiledScalarSignal::new(
        vestra_core::plan::RawScalarSignal::Audio(AudioScalarSignal {
            tap: AudioAnalysisTap::Master,
            feature: AudioScalarFeature::BandEnergy(high_band),
        }),
        vec![CompiledSignalTransform::Gain(
            GainTransform::new(100.0).expect("finite gain"),
        )],
    );
    plan.scalar_signals = CompiledScalarSignals::from_signals(vec![bass, rms, high]);
    plan.audio_analysis_requirements = AudioAnalysisRequirements::from_requirements([
        AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(bass_band)),
        AudioAnalysisRequirement::Master(AudioScalarFeature::Rms),
        AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(high_band)),
    ]);
    let signal_ids = plan
        .scalar_signals
        .iter()
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    let [bass_id, rms_id, high_id] = signal_ids.as_slice() else {
        panic!("expected three complete signals");
    };
    let (bass_id, rms_id, high_id) = (*bass_id, *rms_id, *high_id);

    let layer = &mut plan.layers[0];
    let layer_start =
        vestra_core::plan_time::to_nanos(4.0, "reference layer start").expect("start");
    let layer_end = vestra_core::plan_time::to_nanos(6.0, "reference layer end").expect("end");
    layer.start_nanos = layer_start;
    layer.duration_nanos = layer_end - layer_start;
    layer.start_frame =
        vestra_core::plan_time::first_frame_at_or_after(layer_start, plan.frame_rate)
            .expect("start frame");
    layer.end_frame = vestra_core::plan_time::first_frame_at_or_after(layer_end, plan.frame_rate)
        .expect("end frame");
    layer.draw_key.start_nanos = layer_start;
    layer.transform.scale = Track::new(vestra_core::domain::Point { x: 1.0, y: 1.0 });
    let uniform_scale = CompiledScalarModifier {
        operation: ScalarModifierOperation::Multiply,
        signal: bass_id,
    };
    layer.transform.scale_x_modifiers = vec![uniform_scale];
    layer.transform.scale_y_modifiers = vec![uniform_scale];
    layer.transform_contributions.clear();
    layer.effects.clear();
    layer.effects.extend([
        TimedEffect {
            start: 0,
            end: layer.duration_nanos,
            effect: CompiledEffect::Glow {
                threshold: CompiledScalarProperty::constrained(
                    Track::new(0.25),
                    ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 1.0 },
                ),
                radius: CompiledScalarProperty::constrained(
                    Track::new(8.0),
                    ScalarPropertyConstraint::ClosedRange {
                        min: 0.0,
                        max: 32.0,
                    },
                ),
                intensity: CompiledScalarProperty {
                    authored_track: Track::new(0.0),
                    modifiers: vec![CompiledScalarModifier {
                        operation: ScalarModifierOperation::Replace,
                        signal: rms_id,
                    }],
                    constraint: ScalarPropertyConstraint::ClosedRange { min: 0.0, max: 4.0 },
                },
                colour: [255, 255, 255, 255],
            },
            dependency: TemporalDependency::Dynamic,
        },
        TimedEffect {
            start: 0,
            end: layer.duration_nanos,
            effect: CompiledEffect::ChromaticAberration {
                amount: CompiledScalarProperty {
                    authored_track: Track::new(0.0),
                    modifiers: vec![CompiledScalarModifier {
                        operation: ScalarModifierOperation::Replace,
                        signal: high_id,
                    }],
                    constraint: ScalarPropertyConstraint::ClosedRange {
                        min: 0.0,
                        max: 32.0,
                    },
                },
                angle_degrees: CompiledScalarProperty::authored(Track::new(0.0)),
            },
            dependency: TemporalDependency::Dynamic,
        },
    ]);
    layer.content_dependency = TemporalDependency::Dynamic;
    plan.visual_dependency = TemporalDependency::Dynamic;
    attach_test_master_audio(&mut plan, source);

    let prepared = prepare(&plan, RenderBackendPreference::Wgpu, |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                1,
                vec![],
                Arc::new(Mutex::new(Vec::new())),
            )),
            None,
        ))
    })
    .expect("reference audio analysis prepares");
    let project_time =
        vestra_core::plan_time::to_nanos(5.0, "reference project time").expect("time");
    let frame_number =
        vestra_core::plan_time::first_frame_at_or_after(project_time, plan.frame_rate)
            .expect("frame");
    let active = ActiveSchedule::compile(&plan).active_at(&plan, frame_number);
    let frame = evaluate_with_context(
        &plan,
        &active,
        project_time,
        &EvaluationContext::new(prepared.scalar_signals()),
    )
    .expect("reference frame evaluates");
    let layer = frame
        .layers
        .iter()
        .find(|layer| layer.compiled_layer_index == 0)
        .expect("reference layer is active");
    let EvaluatedSource::Image { .. } = &layer.source else {
        panic!("expected image reference layer, found {:?}", layer.source);
    };
    let transform = &layer.transform;
    let bass_value = prepared
        .scalar_signals()
        .get(bass_id)
        .expect("bass signal")
        .sample(project_time);
    assert!(bass_value > 1.0 && bass_value <= 1.08);
    assert!((transform.scale.x - bass_value).abs() < 1e-12);
    assert!((transform.scale.y - bass_value).abs() < 1e-12);

    let rms_value = prepared
        .scalar_signals()
        .get(rms_id)
        .expect("RMS signal")
        .sample(project_time);
    let high_value = prepared
        .scalar_signals()
        .get(high_id)
        .expect("high-band signal")
        .sample(project_time);
    let glow = layer.effects.iter().find_map(|effect| match effect {
        EvaluatedEffect::Glow { intensity, .. } => Some(*intensity),
        _ => None,
    });
    let chromatic = layer.effects.iter().find_map(|effect| match effect {
        EvaluatedEffect::ChromaticAberration { amount, .. } => Some(*amount),
        _ => None,
    });
    assert_eq!(glow, Some(rms_value.clamp(0.0, 4.0)));
    assert!(glow.expect("Glow reference") > 0.0);
    assert_eq!(chromatic, Some(high_value.clamp(0.0, 32.0)));
    assert!(chromatic.expect("chromatic reference") > 0.0);
}

#[test]
fn prepared_audio_analysis_is_reused_across_random_access_and_video_operations() {
    reset_audio_analysis_invocation_count();
    let mut plan = super::example_plan();
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let source = directory.path().join("prepared-reuse.wav");
    let source_frames =
        usize::try_from(vestra_media::seconds_to_samples(plan.duration).expect("fixture duration"))
            .expect("fixture sample count fits usize");
    write_stepped_tone_wav(&source, source_frames);
    let bands = [
        AudioFrequencyBand::new(40.0, 160.0).expect("low band"),
        AudioFrequencyBand::new(160.0, 640.0).expect("mid band"),
        AudioFrequencyBand::new(640.0, 2_560.0).expect("high band"),
    ];
    plan.scalar_signals = CompiledScalarSignals::from_signals(
        bands
            .iter()
            .copied()
            .map(|band| {
                CompiledScalarSignal::new(
                    vestra_core::plan::RawScalarSignal::Audio(AudioScalarSignal {
                        tap: AudioAnalysisTap::Master,
                        feature: AudioScalarFeature::BandEnergy(band),
                    }),
                    vec![],
                )
            })
            .collect(),
    );
    plan.audio_analysis_requirements = AudioAnalysisRequirements::from_requirements(
        bands
            .iter()
            .copied()
            .map(|band| AudioAnalysisRequirement::Master(AudioScalarFeature::BandEnergy(band))),
    );
    let band_signal_ids = plan.scalar_signals.iter().map(|(id, _)| id).collect();
    plan.layers[0].source = CompiledVisualSource::Spectrum2D {
        wave: None,
        band_signals: band_signal_ids,
        x: 0.1,
        y: 0.1,
        width: 0.8,
        height: 0.8,
        bar_gap_ratio: 0.2,
        min_bar_height_ratio: 0.0,
        layout: vestra_core::project::Spectrum2DLayout::default(),
        gradient: None,
        colour: [0, 255, 128, 255],
    };
    attach_test_master_audio(&mut plan, source);

    let total_frames = plan.frame_count;
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                3,
                (0..total_frames).collect(),
                Arc::new(Mutex::new(Vec::new())),
            )),
            None,
        ))
    })
    .expect("analysis preparation succeeds");
    assert_eq!(prepared.scalar_signals().len(), bands.len());
    assert_eq!(audio_analysis_invocation_count(), 1);

    prepared.render_frame(0).expect("first random-access frame");
    prepared
        .render_frame(plan.frame_count / 2)
        .expect("later random-access frame");
    let output = directory.path().join("prepared-reuse.mp4");
    let options = RenderOptions {
        frame_range: None,
        output_override: Some(output),
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
    .expect("video operation reuses prepared analysis");
    prepared.render_frame(0).expect("frame after video");
    assert_eq!(audio_analysis_invocation_count(), 1);
}

#[test]
fn preparation_without_analysis_records_zero_audio_analysis_time() {
    let prepared = prepare(
        super::example_plan(),
        RenderBackendPreference::Wgpu,
        |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    1,
                    vec![],
                    Arc::new(Mutex::new(Vec::new())),
                )),
                None,
            ))
        },
    )
    .expect("ordinary preparation");
    assert_eq!(prepared.audio_analysis_duration(), Duration::ZERO);
}

#[test]
fn ordinary_output_audio_without_visual_signals_skips_audio_analysis() {
    reset_audio_analysis_invocation_count();
    let mut plan = super::example_plan();
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let source = directory.path().join("ordinary-output-audio.wav");
    let source_frames =
        usize::try_from(vestra_media::seconds_to_samples(plan.duration).expect("fixture duration"))
            .expect("fixture sample count fits usize");
    write_stepped_tone_wav(&source, source_frames);
    attach_test_master_audio(&mut plan, source);
    plan.audio_output_enabled = true;
    plan.encoder.audio_mix = Some(plan.audio_mix.clone());

    assert!(plan.scalar_signals.is_empty());
    assert!(plan.audio_analysis_requirements.is_empty());
    assert!(plan.encoder.audio_mix.is_some());

    let prepared = prepare(&plan, RenderBackendPreference::Wgpu, |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                1,
                vec![],
                Arc::new(Mutex::new(Vec::new())),
            )),
            None,
        ))
    })
    .expect("ordinary output-audio preparation");

    assert_eq!(audio_analysis_invocation_count(), 0);
    assert_eq!(prepared.audio_analysis_duration(), Duration::ZERO);
}
