use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use crate::{
    Category, Diagnostic, Severity,
    plan::EvaluatedFrame,
    render::{
        AdapterMetadata, CompletedFrame, PollMode, RenderBackend, RenderBackendKind, StagedMetrics,
    },
};
use vestra_core::{
    animation::{Interpolation, Keyframe, Track},
    plan::{
        ActiveSchedule, AudioAnalysisRequirement, AudioAnalysisRequirements, AudioAnalysisTap,
        AudioFrequencyBand, AudioScalarFeature, AudioScalarSignal, ClampTransform, CompiledEffect,
        CompiledScalarModifier, CompiledScalarProperty, CompiledScalarSignal,
        CompiledScalarSignals, CompiledSignalTransform, CompiledVisualSource, CubicResponseCurve,
        EnvelopeTransform, EvaluatedEffect, EvaluatedSource, EvaluationContext, GainTransform,
        RemapTransform, RenderPlan, ScalarModifierOperation, ScalarPropertyConstraint,
        TemporalDependency, TimedEffect, evaluate_with_context,
    },
    plan_audio::{AudioClipPlan, AudioMixPlan, AudioTrackPlan, MASTER_AUDIO_SAMPLE_RATE},
    project::AudioFadeCurve,
};
use vestra_media::{EncoderSettings, FrameSink, MediaError, SinkResult};
use vestra_render::CpuBackend;

use super::super::{
    BackendFallback, RenderBackendPreference, RenderObserverControl, RenderOptions,
    runner::{
        audio_analysis_invocation_count, prepare, render_prepared_frame,
        render_with_backend_builder, render_with_backend_builder_and_sink,
        reset_audio_analysis_invocation_count,
    },
};
use super::render_prepared_with_sink;

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

fn write_stepped_tone_wav(path: &std::path::Path, samples: usize) {
    let mut bytes = Vec::new();
    let data_length = u32::try_from(samples.checked_mul(2).expect("WAV data length"))
        .expect("WAV fixture fits u32");
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&MASTER_AUDIO_SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(MASTER_AUDIO_SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_length.to_le_bytes());
    for index in 0..samples {
        let time = index as f32 / MASTER_AUDIO_SAMPLE_RATE as f32;
        let amplitude = if time >= 4.9 { 0.25 } else { 0.0 };
        let pcm =
            (amplitude * (2.0 * std::f32::consts::PI * 100.0 * time).sin() * f32::from(i16::MAX))
                .round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    fs::write(path, bytes).expect("write fixture WAV");
}

fn write_stepped_multiband_wav(path: &std::path::Path, samples: usize) {
    let mut bytes = Vec::new();
    let data_length = u32::try_from(samples.checked_mul(2).expect("WAV data length"))
        .expect("WAV fixture fits u32");
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&MASTER_AUDIO_SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(MASTER_AUDIO_SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_length.to_le_bytes());
    for index in 0..samples {
        let time = index as f32 / MASTER_AUDIO_SAMPLE_RATE as f32;
        let sample = if time >= 4.9 {
            0.22 * (2.0 * std::f32::consts::PI * 100.0 * time).sin()
                + 0.18 * (2.0 * std::f32::consts::PI * 4_000.0 * time).sin()
        } else {
            0.0
        };
        let pcm = (sample * f32::from(i16::MAX)).round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    fs::write(path, bytes).expect("write multiband fixture WAV");
}

fn attach_test_master_audio(plan: &mut RenderPlan, source: PathBuf) {
    plan.audio_output_enabled = false;
    plan.encoder.audio_mix = None;
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

    render_prepared_frame(&mut prepared, 0).expect("first random-access frame");
    render_prepared_frame(&mut prepared, plan.frame_count / 2).expect("later random-access frame");
    let output = directory.path().join("prepared-reuse.mp4");
    let options = RenderOptions {
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
    render_prepared_frame(&mut prepared, 0).expect("frame after video");
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

struct RecordingSink {
    probe: SinkProbe,
    temporary_path: PathBuf,
    fail_on_frame: Option<u64>,
    fail_finish: bool,
    fail_abort: bool,
}

struct PixelSink {
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    temporary_path: PathBuf,
}

impl FrameSink for PixelSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
        self.frames
            .lock()
            .expect("pixel sink lock")
            .push(frame.rgba.clone());
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        fs::write(&self.temporary_path, b"fake encoded output").map_err(MediaError::Publication)?;
        Ok(SinkResult {
            frames_written: self.frames.lock().expect("pixel sink lock").len() as u64,
        })
    }

    fn abort(&mut self) -> Result<(), MediaError> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct SinkProbe {
    frames: Arc<Mutex<Vec<u64>>>,
    abort_count: Arc<AtomicUsize>,
    finish_count: Arc<AtomicUsize>,
    reported_frames: Option<u64>,
}

impl RecordingSink {
    fn new(temporary_path: PathBuf, probe: SinkProbe) -> Self {
        Self {
            probe,
            temporary_path,
            fail_on_frame: None,
            fail_finish: false,
            fail_abort: false,
        }
    }

    fn failing_on_frame(mut self, frame_number: u64) -> Self {
        self.fail_on_frame = Some(frame_number);
        self
    }

    fn failing_abort(mut self) -> Self {
        self.fail_abort = true;
        self
    }
}

impl FrameSink for RecordingSink {
    fn write_frame(&mut self, frame: &CompletedFrame) -> Result<(), MediaError> {
        if self.fail_on_frame == Some(frame.frame_number) {
            return Err(MediaError::FrameInputClosed);
        }
        self.probe
            .frames
            .lock()
            .expect("sink lock")
            .push(frame.frame_number);
        Ok(())
    }

    fn finish(&mut self) -> Result<SinkResult, MediaError> {
        self.probe.finish_count.fetch_add(1, Ordering::Relaxed);
        if self.fail_finish {
            return Err(MediaError::FrameInputClosed);
        }
        fs::write(&self.temporary_path, b"fake encoded output").map_err(MediaError::Publication)?;
        Ok(SinkResult {
            frames_written: self
                .probe
                .reported_frames
                .unwrap_or_else(|| self.probe.frames.lock().expect("sink lock").len() as u64),
        })
    }

    fn abort(&mut self) -> Result<(), MediaError> {
        self.probe.abort_count.fetch_add(1, Ordering::Relaxed);
        if self.fail_abort {
            Err(MediaError::FrameInputClosed)
        } else {
            Ok(())
        }
    }
}

struct MockStagedBackend {
    capacity: usize,
    completion_order: VecDeque<u64>,
    pending: HashMap<u64, CompletedFrame>,
    written: Arc<Mutex<Vec<u64>>>,
    metrics: StagedMetrics,
    mode: MockMode,
    duplicate: Option<CompletedFrame>,
    cancel_after_submit: Option<Arc<std::sync::atomic::AtomicBool>>,
    cancel_after_poll: Option<Arc<std::sync::atomic::AtomicBool>>,
    cancel_on_idle_verify: Option<Arc<std::sync::atomic::AtomicBool>>,
    abort_count: Arc<AtomicUsize>,
    failed_frame_number: Option<u64>,
    backend_kind: RenderBackendKind,
    configured_frame_render_work_duration: Duration,
    final_drain_failure_after_submissions: Option<u64>,
    final_drain_completion_returned: bool,
}

#[derive(Clone, Copy)]
enum MockMode {
    Normal,
    SubmitFailure,
    RichSubmitFailure,
    PollFailure,
    MissingCompletion,
    DuplicateCompletion,
    InvalidFrameLayout,
    FlushFailure,
    IdleFailure,
    FinalDrainFailure,
}

impl MockStagedBackend {
    fn new(capacity: usize, completion_order: Vec<u64>, written: Arc<Mutex<Vec<u64>>>) -> Self {
        Self {
            capacity,
            completion_order: completion_order.into(),
            pending: HashMap::new(),
            written,
            metrics: StagedMetrics {
                configured_pipeline_depth: capacity,
                allocated_slot_count: capacity,
                ..StagedMetrics::default()
            },
            mode: MockMode::Normal,
            duplicate: None,
            cancel_after_submit: None,
            cancel_after_poll: None,
            cancel_on_idle_verify: None,
            abort_count: Arc::new(AtomicUsize::new(0)),
            failed_frame_number: None,
            backend_kind: RenderBackendKind::Wgpu,
            configured_frame_render_work_duration: Duration::ZERO,
            final_drain_failure_after_submissions: None,
            final_drain_completion_returned: false,
        }
    }

    fn failing(mut self, mode: MockMode) -> Self {
        self.mode = mode;
        self
    }

    fn cancel_after_submit(mut self, cancelled: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel_after_submit = Some(cancelled);
        self
    }

    fn cancel_after_poll(mut self, cancelled: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel_after_poll = Some(cancelled);
        self
    }

    fn cancel_on_idle_verify(mut self, cancelled: Arc<std::sync::atomic::AtomicBool>) -> Self {
        self.cancel_on_idle_verify = Some(cancelled);
        self
    }

    fn abort_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.abort_count)
    }

    fn with_failed_frame_number(mut self, frame_number: u64) -> Self {
        self.failed_frame_number = Some(frame_number);
        self
    }

    fn with_cpu_backend(mut self) -> Self {
        self.backend_kind = RenderBackendKind::Cpu;
        self
    }

    fn with_frame_render_work_duration(mut self, duration: Duration) -> Self {
        self.configured_frame_render_work_duration = duration;
        self
    }

    fn fails_in_final_drain(mut self, after_submissions: u64) -> Self {
        self.mode = MockMode::FinalDrainFailure;
        self.final_drain_failure_after_submissions = Some(after_submissions);
        self
    }

    fn next_pending_frame(&mut self) -> Option<u64> {
        while let Some(frame_number) = self.completion_order.pop_front() {
            if self.pending.contains_key(&frame_number) {
                return Some(frame_number);
            }
        }
        self.pending.keys().copied().min()
    }
}

impl RenderBackend for MockStagedBackend {
    fn kind(&self) -> RenderBackendKind {
        self.backend_kind
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn in_flight(&self) -> usize {
        self.pending.len()
    }

    fn failed_frame_number(&self) -> Option<u64> {
        self.failed_frame_number
    }

    fn submit_frame(
        &mut self,
        frame_number: u64,
        frame: &EvaluatedFrame,
    ) -> Result<(), Diagnostic> {
        if matches!(self.mode, MockMode::RichSubmitFailure) {
            return Err(Diagnostic::error(
                "WGPU-DEVICE-LOST",
                Category::Backend,
                "injected device loss",
                "/readback/slot/2",
            )
            .with_hint("adapter=mock; generation=7")
            .with_related_id("frame-100"));
        }
        if matches!(self.mode, MockMode::SubmitFailure) {
            return Err(Diagnostic::error(
                "MOCK-SUBMIT",
                Category::Backend,
                "mock submission failed",
                "",
            ));
        }
        let rgba = if matches!(self.mode, MockMode::InvalidFrameLayout) {
            vec![0]
        } else {
            vec![frame_number as u8; frame.width as usize * frame.height as usize * 4]
        };
        self.pending
            .insert(frame_number, CompletedFrame { frame_number, rgba });
        self.metrics.submitted_frames += 1;
        self.metrics.peak_frames_in_flight =
            self.metrics.peak_frames_in_flight.max(self.pending.len());
        if let Some(cancelled) = &self.cancel_after_submit {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    fn poll_completed(&mut self, _mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic> {
        if matches!(self.mode, MockMode::FinalDrainFailure)
            && self.final_drain_failure_after_submissions == Some(self.metrics.submitted_frames)
        {
            if self.final_drain_completion_returned {
                return Err(Diagnostic::error(
                    "CPU-WORKER-PANIC",
                    Category::Backend,
                    "injected CPU worker failure during final drain",
                    "",
                ));
            }
            self.final_drain_completion_returned = true;
        }
        if matches!(self.mode, MockMode::PollFailure) {
            return Err(Diagnostic::error(
                "MOCK-POLL",
                Category::Backend,
                "mock polling failed",
                "",
            ));
        }
        if matches!(self.mode, MockMode::MissingCompletion) {
            return Ok(None);
        }
        if let Some(duplicate) = self.duplicate.take() {
            return Ok(Some(duplicate));
        }
        let Some(frame_number) = self.next_pending_frame() else {
            return Ok(None);
        };
        let frame = self.pending.remove(&frame_number).ok_or_else(|| {
            Diagnostic::error(
                "MOCK-COMPLETION",
                Category::Backend,
                "mock completion disappeared",
                "",
            )
        })?;
        if matches!(self.mode, MockMode::DuplicateCompletion) && self.duplicate.is_none() {
            self.duplicate = Some(CompletedFrame {
                frame_number: frame.frame_number,
                rgba: frame.rgba.clone(),
            });
        }
        self.metrics.backend_completed_frames += 1;
        if let Some(cancelled) = self.cancel_after_poll.take() {
            cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(Some(frame))
    }

    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic> {
        if matches!(self.mode, MockMode::FlushFailure) {
            return Err(Diagnostic::error(
                "MOCK-FLUSH",
                Category::Backend,
                "mock flush failed",
                "",
            ));
        }
        let mut frames = Vec::new();
        while let Some(frame) = self.poll_completed(PollMode::Drain)? {
            frames.push(frame);
        }
        Ok(frames)
    }

    fn abort(&mut self) {
        self.abort_count.fetch_add(1, Ordering::Relaxed);
        self.pending.clear();
    }

    fn verify_idle(&self) -> Result<(), Diagnostic> {
        if let Some(cancelled) = &self.cancel_on_idle_verify {
            cancelled.store(true, Ordering::Relaxed);
        }
        if matches!(self.mode, MockMode::IdleFailure) {
            Err(Diagnostic::error(
                "MOCK-NOT-IDLE",
                Category::Backend,
                "mock backend retained an unsafe readback state",
                "",
            ))
        } else if self.pending.is_empty() {
            Ok(())
        } else {
            Err(Diagnostic::error(
                "VESTRA-BACKEND-NOT-IDLE",
                Category::Backend,
                "mock backend retained pending work",
                "",
            ))
        }
    }

    fn stats(&mut self) -> crate::render::PreparationStats {
        crate::render::PreparationStats::default()
    }

    fn timings(&self) -> crate::render::PreparationTimings {
        crate::render::PreparationTimings::default()
    }

    fn staged_metrics(&self) -> StagedMetrics {
        self.metrics
    }

    fn reset_operation_metrics(&mut self) {
        self.metrics = StagedMetrics {
            configured_pipeline_depth: self.capacity,
            allocated_slot_count: self.capacity,
            frame_render_work_duration: self.configured_frame_render_work_duration,
            ..StagedMetrics::default()
        };
    }

    fn record_written(&mut self, frame_number: u64) {
        self.written
            .lock()
            .expect("mock metrics lock")
            .push(frame_number);
        self.metrics.written_frames += 1;
    }

    fn record_ready_queue(&mut self, length: usize, out_of_order: bool) {
        self.metrics.ordered_ready_queue_peak = self.metrics.ordered_ready_queue_peak.max(length);
        if out_of_order {
            self.metrics.out_of_order_completion_count += 1;
        }
    }

    fn adapter(&self) -> Option<AdapterMetadata> {
        None
    }
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
fn cpu_frame_render_timing_comes_from_backend_work_duration() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
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
    let frame = render_prepared_frame(&mut prepared, 0).expect("WGPU frame completes");
    assert_eq!(frame.frame_number, 0);
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
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
fn malformed_wgpu_completion_invalidates_the_prepared_backend() {
    let plan = super::example_plan();
    let mut prepared = prepare(plan, RenderBackendPreference::Wgpu, |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(1, vec![0], Arc::new(Mutex::new(Vec::new())))
                    .failing(MockMode::InvalidFrameLayout),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("prepared WGPU seam");

    let error = render_prepared_frame(&mut prepared, 0).expect_err("invalid backend output");
    assert_eq!(error.diagnostic.code, "VESTRA-BACKEND-CONTRACT");
    let later = render_prepared_frame(&mut prepared, 0).expect_err("state invalidated");
    assert_eq!(later.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}

#[test]
fn frame_failures_preserve_complete_backend_diagnostics() {
    let plan = super::example_plan();
    let mut prepared = prepare(plan, RenderBackendPreference::Wgpu, |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(1, vec![], Arc::new(Mutex::new(Vec::new())))
                    .failing(MockMode::RichSubmitFailure),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("prepared WGPU seam");

    let error = render_prepared_frame(&mut prepared, 0).expect_err("device loss");
    assert_eq!(error.diagnostic.code, "WGPU-DEVICE-LOST");
    assert_eq!(error.diagnostic.category, Category::Backend);
    assert_eq!(error.diagnostic.severity, Severity::Fatal);
    assert_eq!(
        error.diagnostic.pointer.as_deref(),
        Some("/readback/slot/2")
    );
    assert_eq!(
        error.diagnostic.hint.as_deref(),
        Some("adapter=mock; generation=7")
    );
    assert_eq!(error.diagnostic.related_id.as_deref(), Some("frame-100"));
    assert_eq!(error.diagnostic.message, "injected device loss");
    let later = render_prepared_frame(&mut prepared, 0).expect_err("invalidated state");
    assert_eq!(later.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
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
fn idle_failure_prevents_publication_and_invalidates_prepared_state() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let output = workspace.path().join("must-not-publish.mp4");
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        let backend = MockStagedBackend::new(
            1,
            (0..plan.frame_count).collect(),
            Arc::new(Mutex::new(Vec::new())),
        )
        .failing(MockMode::IdleFailure);
        Ok((Box::new(backend) as Box<dyn RenderBackend>, None))
    })
    .expect("prepared state");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let mut events = Vec::new();
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |event| {
            events.push(event);
            RenderObserverControl::Continue
        },
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("idle failure rejects the operation");
    assert_eq!(error.diagnostic.code, "MOCK-NOT-IDLE");
    assert!(!output.exists());
    assert!(matches!(
        events.first(),
        Some(vestra_progress::RenderEvent::Started { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(vestra_progress::RenderEvent::Failed { .. })
    ));
    assert!(
        events
            .iter()
            .all(|event| event.operation_id() == events[0].operation_id())
    );
    let next = render_prepared_with_sink(
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
    .expect_err("idle failure invalidates the prepared state");
    assert_eq!(next.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}

#[test]
fn already_cancelled_operation_keeps_prepared_backend_reusable() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let creations = Arc::new(AtomicUsize::new(0));
    let creation_probe = Arc::clone(&creations);
    let backend = MockStagedBackend::new(
        3,
        (0..total_frames).collect(),
        Arc::new(Mutex::new(Vec::new())),
    );
    let aborts = backend.abort_count();
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        creation_probe.fetch_add(1, Ordering::Relaxed);
        Ok((Box::new(backend) as Box<dyn RenderBackend>, None))
    })
    .expect("preparation succeeds");
    let cancelled = RenderOptions {
        output_override: Some(workspace.path().join("cancelled.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &cancelled,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect_err("already-cancelled operation stops before submit");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(aborts.load(Ordering::Relaxed), 0);
    let fresh = RenderOptions {
        output_override: Some(workspace.path().join("fresh.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    };
    let summary = render_prepared_with_sink(
        &mut prepared,
        &fresh,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("fresh operation reuses the prepared backend");
    assert_eq!(creations.load(Ordering::Relaxed), 1);
    assert_eq!(
        summary.requested_render_backend,
        RenderBackendPreference::Wgpu
    );
    assert_eq!(summary.render_backend, RenderBackendKind::Wgpu);
    assert!(workspace.path().join("fresh.mp4").exists());
}

#[test]
fn prepared_state_rejects_reuse_after_submission_failure() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((
            Box::new(
                MockStagedBackend::new(
                    3,
                    (0..total_frames).collect(),
                    Arc::new(Mutex::new(Vec::new())),
                )
                .failing(MockMode::SubmitFailure),
            ) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("preparation succeeds before submission");
    let options = RenderOptions {
        output_override: Some(workspace.path().join("failed.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let first = render_prepared_with_sink(
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
    .expect_err("submission failure invalidates the prepared backend");
    assert_eq!(first.diagnostic.code, "MOCK-SUBMIT");

    let second = render_prepared_with_sink(
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
    .expect_err("invalidated state cannot be silently rebuilt");
    assert_eq!(second.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}

#[test]
fn output_precheck_failure_leaves_prepared_state_ready() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                3,
                (0..total_frames).collect(),
                Arc::new(Mutex::new(Vec::new())),
            )) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("preparation succeeds without an output path or encoder");
    let invalid = RenderOptions {
        output_override: Some(workspace.path().join("missing-parent/out.mp4")),
        overwrite: false,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &invalid,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, _temporary_path| -> Result<RecordingSink, MediaError> {
            panic!("sink must not start before output validation")
        },
    )
    .expect_err("output failure happens before renderer submission");
    assert_eq!(error.diagnostic.code, "VESTRA-OUTPUT-PREPARE");

    let corrected = RenderOptions {
        output_override: Some(workspace.path().join("recovered.mp4")),
        ..invalid
    };
    render_prepared_with_sink(
        &mut prepared,
        &corrected,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                SinkProbe::default(),
            ))
        },
    )
    .expect("pre-submission failure leaves state reusable");
}

#[test]
fn encoder_startup_failure_leaves_prepared_state_ready() {
    let plan = super::example_plan();
    let workspace = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((
            Box::new(MockStagedBackend::new(
                3,
                (0..total_frames).collect(),
                Arc::new(Mutex::new(Vec::new())),
            )) as Box<dyn RenderBackend>,
            None,
        ))
    })
    .expect("preparation succeeds");
    let options = RenderOptions {
        output_override: Some(workspace.path().join("recovered.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_settings: &EncoderSettings, _temporary_path| -> Result<RecordingSink, MediaError> {
            Err(MediaError::FrameInputClosed)
        },
    )
    .expect_err("encoder startup fails before submission");
    assert_eq!(error.diagnostic.code, "VESTRA-BACKEND-START");

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
    .expect("encoder startup failure leaves state reusable");
}

#[test]
fn prepared_visual_snapshot_ignores_later_source_file_changes() {
    let workspace = tempfile::tempdir().expect("temporary project directory");
    let projects = workspace.path().join("projects");
    let assets = workspace.path().join("assets");
    fs::create_dir_all(&projects).expect("project directory");
    fs::create_dir_all(&assets).expect("asset directory");
    let manifest = projects.join("project.json");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/projects/animation-effects.json"),
        &manifest,
    )
    .expect("copy project fixture");
    let source_assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets");
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

#[test]
fn preparation_fallback_is_retained_on_later_render_failure() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let total_frames = plan.frame_count;
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("fallback-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Auto,
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        3,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .failing(MockMode::SubmitFailure),
                ) as Box<dyn RenderBackend>,
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
    .expect_err("later render failure propagates");

    assert_eq!(error.diagnostic.code, "MOCK-SUBMIT");
    assert_eq!(error.warnings.len(), 1);
    assert_eq!(error.warnings[0].code, "VESTRA-WGPU-FALLBACK");
    assert_eq!(
        error.warnings[0].message,
        "WGPU fallback to CPU: injected pipeline preparation failure"
    );
}

#[test]
fn engine_rejects_a_sink_frame_count_mismatch_before_publication() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("mismatch.mp4");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let total_frames = plan.frame_count;
    let sink_probe = SinkProbe {
        reported_frames: Some(total_frames - 1),
        ..SinkProbe::default()
    };
    let error = render_with_backend_builder_and_sink(
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
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(temporary_path.to_path_buf(), sink_probe))
        },
    )
    .expect_err("short sink result rejects publication");
    assert_eq!(error.diagnostic.code, "VESTRA-SINK-FRAME-COUNT");
    assert!(!output.exists());
}

fn run_failure_case(mode: MockMode) -> crate::render::RenderError {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("mock-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        3,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .failing(mode),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("configured mock failure propagates")
}

#[test]
fn final_drain_cpu_failure_reports_the_worker_frame_not_the_last_project_frame() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("final-drain-failure.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Cpu,
    };
    let failed_frame = 7;
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        3,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .with_cpu_backend()
                    .with_failed_frame_number(failed_frame)
                    .fails_in_final_drain(total_frames),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("final-drain worker failure propagates");

    assert_eq!(error.diagnostic.code, "CPU-WORKER-PANIC");
    assert_eq!(error.context.attempted_frame, Some(failed_frame));
    assert_ne!(error.context.attempted_frame, Some(total_frames - 1));
}

fn run_failure_with_recording_sink(
    mode: MockMode,
    fail_on_frame: Option<u64>,
    fail_abort: bool,
) -> (
    crate::render::RenderError,
    SinkProbe,
    Arc<AtomicUsize>,
    PathBuf,
) {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("recording-sink-failure.mp4");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let probe = SinkProbe::default();
    let sink_probe = probe.clone();
    let backend = MockStagedBackend::new(
        3,
        (0..plan.frame_count).collect(),
        Arc::new(Mutex::new(Vec::new())),
    )
    .failing(mode);
    let backend_aborts = backend.abort_count();
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
        move |_settings: &EncoderSettings, temporary_path| {
            let mut sink = RecordingSink::new(temporary_path.to_path_buf(), sink_probe);
            if let Some(frame_number) = fail_on_frame {
                sink = sink.failing_on_frame(frame_number);
            }
            if fail_abort {
                sink = sink.failing_abort();
            }
            Ok(sink)
        },
    )
    .expect_err("configured failure propagates");
    (error, probe, backend_aborts, output)
}

#[test]
fn submission_failure_aborts_the_sink_without_finishing_or_publishing() {
    let (error, probe, backend_aborts, output) =
        run_failure_with_recording_sink(MockMode::SubmitFailure, None, false);
    assert_eq!(error.diagnostic.code, "MOCK-SUBMIT");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert!(!output.exists());
}

#[test]
fn poll_failure_aborts_the_sink_without_finishing_or_publishing() {
    let (error, probe, backend_aborts, output) =
        run_failure_with_recording_sink(MockMode::PollFailure, None, false);
    assert_eq!(error.diagnostic.code, "MOCK-POLL");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert!(!output.exists());
}

#[test]
fn asynchronous_backend_failure_uses_the_backend_frame_identity() {
    let plan = super::example_plan();
    let total_frames = plan.frame_count;
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("frame-identity.mp4")),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        3,
                        (0..total_frames).collect(),
                        Arc::new(Mutex::new(Vec::new())),
                    )
                    .failing(MockMode::PollFailure)
                    .with_failed_frame_number(0),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("configured asynchronous failure propagates");
    assert_eq!(error.context.attempted_frame, Some(0));
}

#[test]
fn sink_write_failure_aborts_renderer_and_sink_without_publishing() {
    let (error, probe, backend_aborts, output) =
        run_failure_with_recording_sink(MockMode::Normal, Some(0), false);
    assert_eq!(error.diagnostic.code, "VESTRA-RENDER-WRITE");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert!(probe.frames.lock().expect("sink lock").is_empty());
    assert!(!output.exists());
}

#[test]
fn sink_abort_failure_is_a_hint_without_replacing_the_primary_failure() {
    let (error, probe, _, output) =
        run_failure_with_recording_sink(MockMode::SubmitFailure, None, true);
    assert_eq!(error.diagnostic.code, "MOCK-SUBMIT");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert!(
        error
            .diagnostic
            .hint
            .as_deref()
            .is_some_and(|hint| hint.contains("encoder cleanup"))
    );
    assert!(!output.exists());
}

#[test]
fn cancellation_aborts_the_sink_and_keeps_cleanup_failure_as_a_hint() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("recording-sink-cancel.mp4");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let probe = SinkProbe::default();
    let sink_probe = probe.clone();
    let backend = MockStagedBackend::new(3, Vec::new(), Arc::new(Mutex::new(Vec::new())));
    let backend_aborts = backend.abort_count();
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| Ok((Box::new(backend) as Box<dyn RenderBackend>, None)),
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(temporary_path.to_path_buf(), sink_probe).failing_abort())
        },
    )
    .expect_err("cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(probe.abort_count.load(Ordering::Relaxed), 1);
    assert_eq!(probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 0);
    assert!(
        error
            .diagnostic
            .hint
            .as_deref()
            .is_some_and(|hint| hint.contains("encoder cleanup"))
    );
    assert!(!output.exists());
}

#[test]
fn engine_propagates_submit_poll_missing_and_flush_failures() {
    assert_eq!(
        run_failure_case(MockMode::SubmitFailure).diagnostic.code,
        "MOCK-SUBMIT"
    );
    assert_eq!(
        run_failure_case(MockMode::PollFailure).diagnostic.code,
        "MOCK-POLL"
    );
    assert_eq!(
        run_failure_case(MockMode::MissingCompletion)
            .diagnostic
            .code,
        "VESTRA-POLL-STALLED"
    );
    assert_eq!(
        run_failure_case(MockMode::FlushFailure).diagnostic.code,
        "MOCK-FLUSH"
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

#[test]
fn engine_cancellation_stops_before_mock_submission() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let options = RenderOptions {
        output_override: Some(output_dir.path().join("mock-cancel.mp4")),
        overwrite: true,
        cancelled,
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    3,
                    Vec::new(),
                    Arc::new(Mutex::new(Vec::new())),
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
}

#[test]
fn engine_cancellation_after_submission_discards_in_flight_work() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("mock-cancel-in-flight.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let written = Arc::new(Mutex::new(Vec::new()));
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend_written = Arc::clone(&written);
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(3, Vec::new(), backend_written)
                        .cancel_after_submit(cancelled),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("in-flight cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert!(written.lock().expect("mock metrics lock").is_empty());
    assert!(!output.exists());
}

#[test]
fn engine_cancellation_during_final_drain_discards_polled_frame() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("mock-cancel-final-drain.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let written = Arc::new(Mutex::new(Vec::new()));
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend_written = Arc::clone(&written);
    let capacity = usize::try_from(plan.frame_count).expect("frame count fits usize");
    let error = render_with_backend_builder(
        &plan,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_, _, _| {
            Ok((
                Box::new(
                    MockStagedBackend::new(
                        capacity,
                        (0..plan.frame_count).collect(),
                        backend_written,
                    )
                    .cancel_after_poll(cancelled),
                ) as Box<dyn RenderBackend>,
                None,
            ))
        },
    )
    .expect_err("final-drain cancellation propagates");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert!(written.lock().expect("mock metrics lock").is_empty());
    assert!(!output.exists());
}

#[test]
fn observer_cancellation_stops_ready_queue_drain_before_another_frame_write() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("ready-queue-cancel.mp4");
    let written = Arc::new(Mutex::new(Vec::new()));
    let backend_written = Arc::clone(&written);
    let sink_probe = SinkProbe::default();
    let sink_probe_for_start = sink_probe.clone();
    let capacity = usize::try_from(plan.frame_count).expect("frame count fits usize");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |event| {
            if matches!(event, vestra_progress::RenderEvent::Progress { .. }) {
                RenderObserverControl::Cancel
            } else {
                RenderObserverControl::Continue
            }
        },
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    capacity,
                    (0..plan.frame_count).collect(),
                    backend_written,
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                sink_probe_for_start,
            ))
        },
    )
    .expect_err("observer cancellation must stop ready queue draining");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(*written.lock().expect("mock metrics lock"), vec![0]);
    assert_eq!(*sink_probe.frames.lock().expect("sink lock"), vec![0]);
    assert_eq!(sink_probe.finish_count.load(Ordering::Relaxed), 0);
    assert!(!output.exists());
}

#[test]
fn callback_token_cancellation_stops_ready_queue_drain_before_another_frame_write() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("ready-queue-token-cancel.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let callback_token = Arc::clone(&cancelled);
    let written = Arc::new(Mutex::new(Vec::new()));
    let backend_written = Arc::clone(&written);
    let sink_probe = SinkProbe::default();
    let sink_probe_for_start = sink_probe.clone();
    let capacity = usize::try_from(plan.frame_count).expect("frame count fits usize");
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled,
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let error = render_with_backend_builder_and_sink(
        &plan,
        &options,
        &mut |event| {
            if matches!(event, vestra_progress::RenderEvent::Progress { .. }) {
                callback_token.store(true, Ordering::Relaxed);
            }
            RenderObserverControl::Continue
        },
        move |_, _, _| {
            Ok((
                Box::new(MockStagedBackend::new(
                    capacity,
                    (0..plan.frame_count).collect(),
                    backend_written,
                )) as Box<dyn RenderBackend>,
                None,
            ))
        },
        move |_settings: &EncoderSettings, temporary_path| {
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                sink_probe_for_start,
            ))
        },
    )
    .expect_err("callback token cancellation must stop ready queue draining");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert_eq!(*written.lock().expect("mock metrics lock"), vec![0]);
    assert_eq!(*sink_probe.frames.lock().expect("sink lock"), vec![0]);
    assert_eq!(sink_probe.finish_count.load(Ordering::Relaxed), 0);
    assert!(!output.exists());
}

#[test]
fn cancellation_after_frame_loop_stops_before_encoder_finalization() {
    let plan = super::example_plan();
    let output_dir = tempfile::tempdir().expect("temporary output directory");
    let output = output_dir.path().join("pre-finalization-cancel.mp4");
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sink_probe = SinkProbe::default();
    let sink_probe_for_start = sink_probe.clone();
    let total_frames = plan.frame_count;
    let options = RenderOptions {
        output_override: Some(output.clone()),
        overwrite: true,
        cancelled: Arc::clone(&cancelled),
        backend_preference: RenderBackendPreference::Wgpu,
    };
    let backend = MockStagedBackend::new(
        1,
        (0..total_frames).collect(),
        Arc::new(Mutex::new(Vec::new())),
    )
    .cancel_on_idle_verify(cancelled);
    let backend_aborts = backend.abort_count();
    let mut prepared = prepare(&plan, RenderBackendPreference::Wgpu, move |_, _, _| {
        Ok((Box::new(backend) as Box<dyn RenderBackend>, None))
    })
    .expect("preparation succeeds");
    let error = render_prepared_with_sink(
        &mut prepared,
        &options,
        &mut |_| RenderObserverControl::Continue,
        move |_settings: &EncoderSettings, temporary_path| {
            fs::write(temporary_path, b"partial encoded output")
                .map_err(MediaError::Publication)?;
            Ok(RecordingSink::new(
                temporary_path.to_path_buf(),
                sink_probe_for_start,
            ))
        },
    )
    .expect_err("cancellation before finalization must stop the operation");
    assert_eq!(error.diagnostic.code, "VESTRA-CANCELLED");
    assert!(error.temporary_removed);
    assert_eq!(backend_aborts.load(Ordering::Relaxed), 1);
    assert_eq!(sink_probe.finish_count.load(Ordering::Relaxed), 0);
    assert_eq!(sink_probe.abort_count.load(Ordering::Relaxed), 1);
    assert!(!output.exists());
    assert!(
        fs::read_dir(output_dir.path())
            .expect("output directory remains readable")
            .next()
            .is_none(),
        "cancellation removes the encoder temporary output"
    );
    let later = render_prepared_frame(&mut prepared, 0)
        .expect_err("post-submission cancellation invalidates prepared state");
    assert_eq!(later.diagnostic.code, "VESTRA-PREPARED-INVALIDATED");
}
