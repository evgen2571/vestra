use crate::test_support::{write_mono_wav, write_mono_wav_samples, write_stereo_wav};

use super::{
    AudioGraphCompileOptions, automation_expression, compile, compile_with_options,
    seconds_to_samples,
};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};
use vestra_core::plan_audio::{
    AudioClipPlan, AudioEffectOperation, AudioEffectPassPlan, AudioMixPlan, AudioTrackPlan,
};

#[test]
fn selected_audio_pads_after_project_end_and_rebases_exact_sample_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("constant.wav");
    write_mono_wav_samples(&path, 48_000, &vec![0.25; 9_600]);
    let mut clip = automation_test_clip();
    clip.path = path;
    clip.gain_automation = None;
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".into(),
            mute: false,
            gain: 1.0,
            clips: vec![clip],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    };
    let graph = compile_with_options(
        &mix,
        0.1,
        AudioGraphCompileOptions {
            first_audio_input_index: 0,
            output_range: Some((0.09, 0.1001)),
        },
    )
    .unwrap();
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(&graph.input_paths[0])
        .args([
            "-filter_complex",
            &graph.filter_complex,
            "-map",
            "[audio]",
            "-ac",
            "1",
            "-f",
            "f32le",
            "-",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let samples: Vec<_> = output
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
        .collect();
    assert_eq!(samples.len(), 485);
    assert!(
        samples[..480]
            .iter()
            .all(|sample| (*sample - 0.25).abs() < 0.0001)
    );
    assert_eq!(&samples[480..], &[0.0; 5]);
}

#[test]
fn rounds_seconds_to_nearest_mixer_sample() {
    assert_eq!(
        seconds_to_samples(0.12345).expect("sample conversion"),
        5926
    );
    assert_eq!(seconds_to_samples(0.5 / 48_000.0).expect("half sample"), 1);
}

#[test]
fn balanced_automation_expressions_are_deterministic_and_parse_at_scale() {
    for keyframe_count in [100, 1_000] {
        let automation = automation_with_keyframes(keyframe_count);
        let clip = automation_test_clip();
        let first = automation_expression(&automation, &clip).expect("expression");
        let second = automation_expression(&automation, &clip).expect("same expression");
        assert_eq!(first, second);
        assert!(
            maximum_expression_depth(&first) <= 12,
            "depth must be logarithmic"
        );
        assert_expression_parses(&first);
    }
}

#[test]
fn configured_automation_limit_builds_with_logarithmic_depth() {
    let automation = automation_with_keyframes(16_384);
    let expression =
        automation_expression(&automation, &automation_test_clip()).expect("expression");
    assert!(
        maximum_expression_depth(&expression) <= 20,
        "depth must be logarithmic"
    );
    assert!(
        expression.len() > 1_000_000,
        "full configured limit was built"
    );
    assert_expression_parses(&expression);
}

#[test]
fn large_automation_executes_and_changes_the_signal() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("constant.wav");
    write_mono_wav(&source, 48_000, 60_000, 0.1);
    let automation = vestra_core::project::AudioGainAutomation {
        keyframes: (0..128)
            .map(|index| vestra_core::project::AudioGainKeyframe {
                time: index as f64 * 0.01,
                gain: if index % 2 == 0 { 0.25 } else { 0.75 },
                interpolation: vestra_core::project::AudioGainInterpolation::Hold,
            })
            .collect(),
    };
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "automation".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![AudioClipPlan {
                path: source,
                selected_duration: 1.4,
                processed_duration: 1.4,
                gain_automation: Some(automation),
                ..automation_test_clip()
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let samples = render_pcm(&compile(&mix, 1.4).expect("large automation graph"), 1.4);
    let amplitude = |frame: usize| samples[frame * 2].abs();
    assert!(
        amplitude(720) > amplitude(240) * 2.0,
        "second hold is louder"
    );
    assert!(amplitude(720) > 0.02, "FFmpeg produced audible audio");
}

#[test]
fn graph_order_and_normalization_are_deterministic() {
    let clip = |id: &str, path: &str| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path: PathBuf::from(path),
        start: 0.12345,
        trim_start: 0.0,
        selected_duration: 1.0,
        processed_duration: 1.0,
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![
            AudioTrackPlan {
                id: "first".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("a", "a.wav"), clip("b", "b.wav")],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
            AudioTrackPlan {
                id: "second".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("c", "c.wav")],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
        ],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 2.0).expect("graph");
    assert_eq!(
        graph.input_paths,
        vec![
            PathBuf::from("a.wav"),
            PathBuf::from("b.wav"),
            PathBuf::from("c.wav")
        ]
    );
    assert_eq!(graph.filter_complex.matches("normalize=0").count(), 2);
    assert!(graph.filter_complex.contains("adelay=5926S:all=1"));
    assert!(
        graph
            .filter_complex
            .contains("aformat=sample_rates=48000:sample_fmts=fltp:channel_layouts=stereo")
    );
}

#[test]
fn reused_source_has_one_input_and_declaration_ordered_branches() {
    let clip = |id: &str, path: &str| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path: PathBuf::from(path),
        start: 0.0,
        trim_start: 0.0,
        selected_duration: 1.0,
        processed_duration: 1.0,
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![
            AudioTrackPlan {
                id: "first".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("clip-b", "shared.wav"), clip("clip-a", "other.wav")],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
            AudioTrackPlan {
                id: "second".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("clip-c", "shared.wav"), clip("clip-d", "shared.wav")],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
        ],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("graph");
    assert_eq!(
        graph.input_paths,
        [PathBuf::from("shared.wav"), PathBuf::from("other.wav")]
    );
    assert_eq!(graph.clip_branch_count, 4);
    assert!(graph.filter_complex.contains("[0:a]aformat"));
    assert!(graph.filter_complex.contains("asplit=3"));
    assert!(!graph.filter_complex.contains("[3:a]"));
}

#[test]
fn configurable_input_base_shifts_only_source_references() {
    let clip = |id: &str, path: &str| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path: PathBuf::from(path),
        start: 0.25,
        trim_start: 0.0,
        selected_duration: 1.0,
        processed_duration: 1.0,
        mute: false,
        gain: 0.5,
        gain_automation: None,
        fade_in: 0.0,
        fade_out: 0.0,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mut sped = clip("first", "first.wav");
    sped.processed_duration = 0.5;
    sped.effects.operations = vec![AudioEffectOperation::PlaybackSpeed {
        rate: 2.0,
        link_pitch: false,
    }];
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 0.75,
            clips: vec![sped, clip("second", "second.wav")],

            effects: AudioEffectPassPlan {
                operations: vec![AudioEffectOperation::ParametricEq {
                    frequency_hz: 120.0,
                    gain_db: 6.0,
                    q: 0.8,
                }],
            },
        }],
        effects: AudioEffectPassPlan {
            operations: vec![AudioEffectOperation::ParametricEq {
                frequency_hz: 240.0,
                gain_db: -3.0,
                q: 1.2,
            }],
        },
    };
    let encoder = compile_with_options(
        &mix,
        2.0,
        AudioGraphCompileOptions {
            output_range: None,
            first_audio_input_index: 1,
        },
    )
    .expect("encoder graph");
    let analyzer = compile_with_options(
        &mix,
        2.0,
        AudioGraphCompileOptions {
            output_range: None,
            first_audio_input_index: 0,
        },
    )
    .expect("analyzer graph");
    assert_eq!(encoder.input_paths, analyzer.input_paths);
    assert!(encoder.filter_complex.contains("[1:a]aformat"));
    assert!(encoder.filter_complex.contains("[2:a]aformat"));
    assert!(analyzer.filter_complex.contains("[0:a]aformat"));
    assert!(analyzer.filter_complex.contains("[1:a]aformat"));
    assert_eq!(
        encoder
            .filter_complex
            .replacen("[1:a]", "[0:a]", 1)
            .replacen("[2:a]", "[1:a]", 1),
        analyzer.filter_complex
    );
    assert!(analyzer.filter_complex.ends_with("[audio]"));
    assert!(encoder.filter_complex.contains("equalizer=f=120"));
    assert!(encoder.filter_complex.contains("equalizer=f=240"));
    assert!(
        encoder
            .filter_complex
            .contains("atempo=2.00000000000000000")
    );
    assert!(
        encoder
            .filter_complex
            .contains("apad,atrim=end_sample=24000")
    );
    assert!(
        analyzer
            .filter_complex
            .contains("atempo=2.00000000000000000")
    );
    assert!(
        analyzer
            .filter_complex
            .contains("apad,atrim=end_sample=24000")
    );
}

#[test]
fn track_effects_follow_track_mix_and_precede_track_gain() {
    let mut first = automation_test_clip();
    first.id = "first".to_owned();
    let mut second = first.clone();
    second.id = "second".to_owned();
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 0.75,
            clips: vec![first, second],
            effects: AudioEffectPassPlan {
                operations: vec![
                    AudioEffectOperation::ParametricEq {
                        frequency_hz: 120.0,
                        gain_db: 3.0,
                        q: 0.8,
                    },
                    AudioEffectOperation::ParametricEq {
                        frequency_hz: 240.0,
                        gain_db: -3.0,
                        q: 1.0,
                    },
                ],
            },
        }],
        effects: AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("track effect graph");
    let mix_at = graph.filter_complex.find("amix=").expect("track mix");
    let first_eq = graph
        .filter_complex
        .find("equalizer=f=120")
        .expect("first eq");
    let second_eq = graph
        .filter_complex
        .find("equalizer=f=240")
        .expect("second eq");
    let gain = graph
        .filter_complex
        .rfind("volume=volume=0.750")
        .expect("track gain");
    assert!(mix_at < first_eq && first_eq < second_eq && second_eq < gain);
}

#[test]
fn master_effects_follow_track_mix_and_precede_padding_and_trim() {
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![automation_test_clip()],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan {
            operations: vec![AudioEffectOperation::ParametricEq {
                frequency_hz: 480.0,
                gain_db: 2.0,
                q: 1.0,
            }],
        },
    };
    let graph = compile(&mix, 1.0).expect("master effect graph");
    let track_gain = graph
        .filter_complex
        .rfind("volume=volume=1.000")
        .expect("track gain");
    let eq = graph
        .filter_complex
        .find("equalizer=f=480")
        .expect("master eq");
    let pad = graph.filter_complex.find("apad=").expect("padding");
    let trim = graph
        .filter_complex
        .find("atrim=end_sample=")
        .expect("trim");
    assert!(track_gain < eq && eq < pad && pad < trim);
}

#[test]
fn large_reused_graph_is_deterministic_and_has_unique_labels() {
    let tracks = (0..16)
        .map(|track| AudioTrackPlan {
            id: format!("track-{track}"),
            mute: false,
            gain: 1.0,
            clips: (0..16)
                .map(|clip| AudioClipPlan {
                    id: format!("clip-{track}-{clip}"),
                    asset: format!("asset-{}", clip % 4),
                    path: PathBuf::from(format!("source-{}.wav", clip % 4)),
                    start: clip as f64 / 100.0,
                    trim_start: 0.0,
                    selected_duration: 1.0,
                    processed_duration: 1.0,
                    mute: false,
                    gain: 1.0,
                    fade_in: 0.0,
                    fade_out: 0.0,
                    gain_automation: None,
                    fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
                    fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,

                    effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
                })
                .collect(),
            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        })
        .collect();
    let mix = AudioMixPlan {
        tracks,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let first = compile(&mix, 2.0).expect("first graph");
    let second = compile(&mix, 2.0).expect("second graph");
    assert_eq!(first.input_paths.len(), 4);
    assert_eq!(first.clip_branch_count, 256);
    assert_eq!(first.filter_complex, second.filter_complex);
    let labels = first
        .filter_complex
        .split('[')
        .skip(1)
        .filter_map(|part| part.split(']').next())
        .filter(|label| label.starts_with("a_"))
        .collect::<Vec<_>>();
    let unique = labels
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    // Labels occur once at creation and may later be consumed, so check
    // generated label numbers rather than requiring every reference unique.
    assert!(unique.len() >= 256, "labels must not collapse at scale");
}

#[test]
fn exact_boundary_fades_fit_after_sample_quantization() {
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![AudioClipPlan {
                id: "clip".to_owned(),
                asset: "asset".to_owned(),
                path: PathBuf::from("tone.wav"),
                start: 0.0,
                trim_start: 0.0,
                selected_duration: 1.0 / 48_000.0,
                processed_duration: 1.0 / 48_000.0,
                mute: false,
                gain: 1.0,
                fade_in: 0.5 / 48_000.0,
                fade_out: 0.5 / 48_000.0,
                gain_automation: None,
                fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
                fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("quantized fades remain executable");
    assert!(graph.filter_complex.contains("aeval=exprs="));
    assert!(graph.filter_complex.contains("min(1,(t)/"));
}

#[test]
fn automation_and_equal_power_fades_compile_to_sample_envelopes() {
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![AudioClipPlan {
                id: "clip".to_owned(),
                asset: "asset".to_owned(),
                path: PathBuf::from("tone.wav"),
                start: 0.0,
                trim_start: 0.0,
                selected_duration: 1.0,
                processed_duration: 1.0,
                mute: false,
                gain: 0.5,
                gain_automation: Some(vestra_core::project::AudioGainAutomation {
                    keyframes: vec![
                        vestra_core::project::AudioGainKeyframe {
                            time: 0.0,
                            gain: 0.0,
                            interpolation: vestra_core::project::AudioGainInterpolation::Linear,
                        },
                        vestra_core::project::AudioGainKeyframe {
                            time: 0.12345,
                            gain: 1.0,
                            interpolation: vestra_core::project::AudioGainInterpolation::Hold,
                        },
                        vestra_core::project::AudioGainKeyframe {
                            time: 0.5,
                            gain: 0.25,
                            interpolation: vestra_core::project::AudioGainInterpolation::Linear,
                        },
                    ],
                }),
                fade_in: 0.5,
                fade_out: 0.5,
                fade_in_curve: vestra_core::project::AudioFadeCurve::EqualPower,
                fade_out_curve: vestra_core::project::AudioFadeCurve::EqualPower,

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("automation graph");
    assert!(graph.filter_complex.contains("aeval=exprs="));
    assert!(graph.filter_complex.contains("0.12345833333333334"));
    assert!(graph.filter_complex.contains("sin(PI"));
    assert!(graph.filter_complex.contains("cos(PI"));
}

#[test]
fn production_graph_places_and_linearly_sums_pcm_without_normalization() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("constant.wav");
    write_mono_wav(&source, 48_000, 48_000, 0.1);
    let clip = |id: &str, start: f64| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path: source.clone(),
        start,
        trim_start: 0.0,
        selected_duration: 0.5,
        processed_duration: 0.5,
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![
            AudioTrackPlan {
                id: "one".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("a", 0.12345)],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
            AudioTrackPlan {
                id: "two".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("b", 0.12345)],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
        ],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("production graph");
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-t", "1"]);
    for path in &graph.input_paths {
        command.arg("-i").arg(path);
    }
    let output = command
        .args([
            "-filter_complex",
            &graph.filter_complex,
            "-map",
            "[audio]",
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("ffmpeg starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let samples = output
        .stdout
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("f32")))
        .collect::<Vec<_>>();
    let onset = seconds_to_samples(0.12345).expect("onset") as usize;
    assert!(
        samples[(onset - 20) * 2..onset * 2]
            .iter()
            .all(|sample| sample.abs() < 1e-6)
    );
    let measured = samples[(onset + 100) * 2];
    // FFmpeg's explicit mono-to-stereo remix uses equal-power channel
    // coefficients, so each channel is approximately 0.0707 per input.
    // With `amix` normalization accidentally enabled this would be half.
    assert!(
        (measured - 0.141_421).abs() < 0.002,
        "expected an unnormalized two-input sum, got {measured}"
    );
}

#[test]
fn pcm_graph_applies_track_gain_after_the_track_submix() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("constant.wav");
    write_mono_wav(&source, 48_000, 48_000, 0.1);
    let clip = |id: &str| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path: source.clone(),
        start: 0.0,
        trim_start: 0.0,
        selected_duration: 0.5,
        processed_duration: 0.5,
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 0.5,
            clips: vec![clip("a"), clip("b")],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let samples = render_pcm(&compile(&mix, 0.5).expect("production graph"), 0.5);
    // Two mono branches sum to about 0.1414 per channel before the track
    // gain. Applying 0.5 at the track produces about 0.0707.
    assert!((samples[2_000 * 2] - 0.070_710_5).abs() < 0.002);
}

#[test]
fn pcm_graph_trims_then_applies_clip_gain_and_linear_clip_local_fades() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("piecewise.wav");
    let source_samples = (0..48_000)
        .map(|index| if index < 12_000 { 0.05 } else { 0.1 })
        .collect::<Vec<_>>();
    write_mono_wav_samples(&source, 48_000, &source_samples);
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![AudioClipPlan {
                id: "clip".to_owned(),
                asset: "source".to_owned(),
                path: source,
                start: 0.12345,
                trim_start: 0.25,
                selected_duration: 0.5,
                processed_duration: 0.5,
                mute: false,
                gain: 0.5,
                fade_in: 0.1,
                fade_out: 0.1,
                gain_automation: None,
                fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
                fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let samples = render_pcm(&compile(&mix, 1.0).expect("production graph"), 1.0);
    let onset = seconds_to_samples(0.12345).expect("onset") as usize;
    let channel = |sample: usize| samples[sample * 2];
    let early = channel(onset + 480);
    let middle = channel(onset + 2_400);
    let steady = channel(onset + 9_600);
    let late = channel(onset + 21_600);
    assert!(
        early < middle && middle < steady,
        "fade-in must rise linearly"
    );
    assert!(
        (steady - 0.035_355).abs() < 0.002,
        "trim selected the 0.1 region: {steady}"
    );
    assert!(
        late < steady && late > 0.0,
        "fade-out must fall before clip end"
    );
}

#[test]
fn pcm_graph_applies_linear_hold_and_equal_power_envelopes_per_sample() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("constant.wav");
    write_mono_wav(&source, 48_000, 96_000, 0.1);
    let clip = |automation, fade_in, fade_out, fade_in_curve, fade_out_curve| AudioClipPlan {
        id: "clip".to_owned(),
        asset: "source".to_owned(),
        path: source.clone(),
        start: 0.0,
        trim_start: 0.0,
        selected_duration: 2.0,
        processed_duration: 2.0,
        mute: false,
        gain: 1.0,
        gain_automation: automation,
        fade_in,
        fade_out,
        fade_in_curve,
        fade_out_curve,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let automation = vestra_core::project::AudioGainAutomation {
        keyframes: vec![
            vestra_core::project::AudioGainKeyframe {
                time: 0.0,
                gain: 0.0,
                interpolation: vestra_core::project::AudioGainInterpolation::Linear,
            },
            vestra_core::project::AudioGainKeyframe {
                time: 1.0,
                gain: 1.0,
                interpolation: vestra_core::project::AudioGainInterpolation::Hold,
            },
            vestra_core::project::AudioGainKeyframe {
                time: 1.12345,
                gain: 0.25,
                interpolation: vestra_core::project::AudioGainInterpolation::Linear,
            },
        ],
    };
    let mix = |clip| AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let automated = render_pcm(
        &compile(
            &mix(clip(
                Some(automation),
                0.0,
                0.0,
                vestra_core::project::AudioFadeCurve::Linear,
                vestra_core::project::AudioFadeCurve::Linear,
            )),
            2.0,
        )
        .expect("automation graph"),
        2.0,
    );
    let channel = |samples: &[f32], frame: usize| samples[frame * 2].abs();
    let unity = channel(&automated, 48_000);
    assert!((channel(&automated, 12_000) / unity - 0.25).abs() < 0.02);
    assert!((channel(&automated, 36_000) / unity - 0.75).abs() < 0.02);
    let boundary = seconds_to_samples(1.12345).expect("boundary") as usize;
    assert!((channel(&automated, boundary - 1) / unity - 1.0).abs() < 0.02);
    assert!((channel(&automated, boundary + 1) / unity - 0.25).abs() < 0.02);

    let descending = vestra_core::project::AudioGainAutomation {
        keyframes: vec![
            vestra_core::project::AudioGainKeyframe {
                time: 0.0,
                gain: 1.0,
                interpolation: vestra_core::project::AudioGainInterpolation::Linear,
            },
            vestra_core::project::AudioGainKeyframe {
                time: 1.0,
                gain: 0.0,
                interpolation: vestra_core::project::AudioGainInterpolation::Linear,
            },
        ],
    };
    let descending_pcm = render_pcm(
        &compile(
            &mix(clip(
                Some(descending),
                0.0,
                0.0,
                vestra_core::project::AudioFadeCurve::Linear,
                vestra_core::project::AudioFadeCurve::Linear,
            )),
            2.0,
        )
        .expect("descending graph"),
        2.0,
    );
    let descending_unity = channel(&descending_pcm, 1);
    assert!((channel(&descending_pcm, 12_000) / descending_unity - 0.75).abs() < 0.02);
    assert!((channel(&descending_pcm, 36_000) / descending_unity - 0.25).abs() < 0.02);

    let equal = render_pcm(
        &compile(
            &mix(clip(
                None,
                1.0,
                0.0,
                vestra_core::project::AudioFadeCurve::EqualPower,
                vestra_core::project::AudioFadeCurve::Linear,
            )),
            2.0,
        )
        .expect("fade graph"),
        2.0,
    );
    let reference = channel(&equal, 72_000);
    assert!((channel(&equal, 24_000) / reference - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02);
    let linear = render_pcm(
        &compile(
            &mix(clip(
                None,
                1.0,
                0.0,
                vestra_core::project::AudioFadeCurve::Linear,
                vestra_core::project::AudioFadeCurve::Linear,
            )),
            2.0,
        )
        .expect("linear fade graph"),
        2.0,
    );
    let linear_reference = channel(&linear, 72_000);
    assert!((channel(&linear, 24_000) / linear_reference - 0.5).abs() < 0.02);
}

#[test]
fn pcm_graph_multiplies_track_clip_and_automation_gain() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("constant.wav");
    write_mono_wav(&source, 48_000, 48_000, 0.1);
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 0.5,
            clips: vec![AudioClipPlan {
                id: "clip".to_owned(),
                asset: "source".to_owned(),
                path: source,
                start: 0.0,
                trim_start: 0.0,
                selected_duration: 1.0,
                processed_duration: 1.0,
                mute: false,
                gain: 0.5,
                gain_automation: Some(vestra_core::project::AudioGainAutomation {
                    keyframes: vec![vestra_core::project::AudioGainKeyframe {
                        time: 0.0,
                        gain: 0.5,
                        interpolation: vestra_core::project::AudioGainInterpolation::Linear,
                    }],
                }),
                fade_in: 0.0,
                fade_out: 0.0,
                fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
                fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            }],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let samples = render_pcm(&compile(&mix, 1.0).expect("composition graph"), 1.0);
    // Mono normalization contributes 1/sqrt(2), then all three gains
    // multiply: 0.1 * 1/sqrt(2) * 0.5 * 0.5 * 0.5.
    assert!((samples[2_000 * 2] - 0.008_838_8).abs() < 0.001);
}

#[test]
fn equal_power_crossfade_keeps_squared_component_sum_near_one() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let low = directory.path().join("440.wav");
    let high = directory.path().join("880.wav");
    write_sine_wav(&low, 440.0, 0.05);
    write_sine_wav(&high, 880.0, 0.05);
    let clip = |id: &str, path: PathBuf, fade_in: f64, fade_out: f64| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path,
        start: 0.0,
        trim_start: 0.0,
        selected_duration: 0.5,
        processed_duration: 0.5,
        mute: false,
        gain: 1.0,
        gain_automation: None,
        fade_in,
        fade_out,
        fade_in_curve: vestra_core::project::AudioFadeCurve::EqualPower,
        fade_out_curve: vestra_core::project::AudioFadeCurve::EqualPower,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = |clips| AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips,

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let reference = render_pcm(
        &compile(
            &mix(vec![
                clip("low", low.clone(), 0.0, 0.0),
                clip("high", high.clone(), 0.0, 0.0),
            ]),
            0.5,
        )
        .expect("reference graph"),
        0.5,
    );
    let crossed = render_pcm(
        &compile(
            &mix(vec![
                clip("low", low, 0.0, 0.5),
                clip("high", high, 0.5, 0.0),
            ]),
            0.5,
        )
        .expect("crossfade graph"),
        0.5,
    );
    for (frame, expected_low, expected_high) in [
        (
            12_000,
            std::f64::consts::FRAC_1_SQRT_2,
            std::f64::consts::FRAC_1_SQRT_2,
        ),
        (23_000, 0.07, 1.0),
    ] {
        let low_gain = tone_window_correlation(&crossed, 440.0, frame, 1_000)
            / tone_window_correlation(&reference, 440.0, frame, 1_000);
        let high_gain = tone_window_correlation(&crossed, 880.0, frame, 1_000)
            / tone_window_correlation(&reference, 880.0, frame, 1_000);
        assert!((low_gain - expected_low).abs() < 0.03);
        assert!((high_gain - expected_high).abs() < 0.03);
        assert!((low_gain * low_gain + high_gain * high_gain - 1.0).abs() < 0.06);
    }
}

#[test]
fn reused_source_branches_keep_independent_trim_gain_fade_and_placement() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("piecewise.wav");
    let source_samples = (0..48_000)
        .map(|index| if index < 12_000 { 0.05 } else { 0.1 })
        .collect::<Vec<_>>();
    write_mono_wav_samples(&source, 48_000, &source_samples);
    let clip = |id: &str, start: f64, trim_start: f64, gain: f64, fade_in: f64| AudioClipPlan {
        id: id.to_owned(),
        asset: "shared".to_owned(),
        path: source.clone(),
        start,
        trim_start,
        selected_duration: 0.5,
        processed_duration: 0.5,
        mute: false,
        gain,
        fade_in,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![
                clip("a", 0.0, 0.0, 1.0, 0.0),
                clip("b", 0.25, 0.0, 0.5, 0.1),
                clip("c", 0.5, 0.25, 1.0, 0.0),
            ],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("production graph");
    assert_eq!(graph.input_paths, vec![source]);
    assert_eq!(graph.clip_branch_count, 3);
    assert!(graph.filter_complex.contains("asplit=3"));
    let samples = render_pcm(&graph, 1.0);
    let channel = |sample: usize| samples[sample * 2];
    assert!(
        channel(14_400) > channel(12_480),
        "clip B has its own fade-in"
    );
    assert!(
        channel(19_200) > channel(9_600),
        "clip B overlaps at its own gain"
    );
    assert!(
        channel(28_800) > channel(21_600),
        "clip C uses its own later trim"
    );
}

#[test]
fn pcm_graph_preserves_silent_gaps_and_trims_at_project_end() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("constant.wav");
    write_mono_wav(&source, 48_000, 48_000, 0.1);
    let clip = |id: &str, start: f64, selected_duration: f64| AudioClipPlan {
        id: id.to_owned(),
        asset: "source".to_owned(),
        path: source.clone(),
        start,
        trim_start: 0.0,
        selected_duration,
        processed_duration: selected_duration,
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip("early", 0.0, 0.1), clip("late", 0.5, 1.0)],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let samples = render_pcm(&compile(&mix, 0.75).expect("production graph"), 0.75);
    assert_eq!(
        samples.len(),
        36_000 * 2,
        "project end truncates PCM exactly"
    );
    let channel = |sample: usize| samples[sample * 2];
    assert!(channel(2_400).abs() > 0.05, "first clip is audible");
    assert!(channel(12_000).abs() < 1e-6, "gap stays silent");
    assert!(
        channel(26_400).abs() > 0.05,
        "later clip keeps its placement"
    );
}

#[test]
fn atempo_decomposition_is_conservative_and_deterministic() {
    assert_eq!(
        super::decompose_atempo(0.25).expect("stages"),
        vec![0.5, 0.5]
    );
    assert_eq!(super::decompose_atempo(0.5).expect("stages"), vec![0.5]);
    assert_eq!(
        super::decompose_atempo(1.0).expect("stages"),
        Vec::<f64>::new()
    );
    assert_eq!(super::decompose_atempo(2.0).expect("stages"), vec![2.0]);
    assert_eq!(
        super::decompose_atempo(4.0).expect("stages"),
        vec![2.0, 2.0]
    );
    assert_eq!(
        super::decompose_atempo(3.0).expect("stages"),
        vec![2.0, 1.5]
    );
}

#[test]
fn pcm_graph_normalizes_44100_mono_to_48000_stereo() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let mono = directory.path().join("mono-44100.wav");
    let stereo_source = directory.path().join("stereo-48000.wav");
    write_mono_wav(&mono, 44_100, 44_100, 0.08);
    write_stereo_wav(
        &stereo_source,
        48_000,
        std::iter::repeat_n([0.06; 2], 48_000),
    );
    let clip = |id: &str, path: PathBuf| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path,
        start: 0.0,
        trim_start: 0.0,
        selected_duration: 0.5,
        processed_duration: 0.5,
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![
            AudioTrackPlan {
                id: "mono".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("mono", mono)],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
            AudioTrackPlan {
                id: "other".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("other", stereo_source)],

                effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
            },
        ],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let samples = render_pcm(&compile(&mix, 0.5).expect("production graph"), 0.5);
    assert_eq!(samples.len(), 24_000 * 2, "final PCM is 48 kHz stereo");
    assert!(samples[2_000 * 2].abs() > 0.05);
    assert!((samples[2_000 * 2] - samples[2_000 * 2 + 1]).abs() < 1e-6);
}

#[test]
fn pcm_graph_keeps_two_frequencies_in_a_same_track_overlap() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let low = directory.path().join("440.wav");
    let high = directory.path().join("880.wav");
    write_sine_wav(&low, 440.0, 0.05);
    write_sine_wav(&high, 880.0, 0.05);
    let clip = |id: &str, path: PathBuf| AudioClipPlan {
        id: id.to_owned(),
        asset: id.to_owned(),
        path,
        start: 0.0,
        trim_start: 0.0,
        selected_duration: 0.5,
        processed_duration: 0.5,
        mute: false,
        gain: 1.0,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_automation: None,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "same-track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip("low", low), clip("high", high)],

            effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
        }],
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    };
    let samples = render_pcm(&compile(&mix, 0.5).expect("production graph"), 0.5);
    assert!(tone_correlation(&samples, 440.0) > 0.01);
    assert!(tone_correlation(&samples, 880.0) > 0.01);
}

fn automation_with_keyframes(count: usize) -> vestra_core::project::AudioGainAutomation {
    vestra_core::project::AudioGainAutomation {
        keyframes: (0..count)
            .map(|index| vestra_core::project::AudioGainKeyframe {
                time: index as f64 / 48_000.0,
                gain: if index % 2 == 0 { 0.25 } else { 0.75 },
                interpolation: if index % 3 == 0 {
                    vestra_core::project::AudioGainInterpolation::Hold
                } else {
                    vestra_core::project::AudioGainInterpolation::Linear
                },
            })
            .collect(),
    }
}

fn automation_test_clip() -> AudioClipPlan {
    AudioClipPlan {
        id: "automation-test".to_owned(),
        asset: "source".to_owned(),
        path: PathBuf::from("source.wav"),
        start: 0.0,
        trim_start: 0.0,
        selected_duration: 1.0,
        processed_duration: 1.0,
        mute: false,
        gain: 1.0,
        gain_automation: None,
        fade_in: 0.0,
        fade_out: 0.0,
        fade_in_curve: vestra_core::project::AudioFadeCurve::Linear,
        fade_out_curve: vestra_core::project::AudioFadeCurve::Linear,
        effects: vestra_core::plan_audio::AudioEffectPassPlan::default(),
    }
}

fn maximum_expression_depth(expression: &str) -> usize {
    let mut depth = 0;
    let mut maximum = 0;
    for character in expression.chars() {
        match character {
            '(' => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            ')' => depth -= 1,
            _ => {}
        }
    }
    maximum
}

fn assert_expression_parses(expression: &str) {
    let directory = tempfile::tempdir().expect("temporary graph directory");
    let graph_path = directory.path().join("automation-filtergraph.txt");
    fs::write(
        &graph_path,
        format!("[0:a]aeval=exprs='val(0)*({expression})|val(1)*({expression})'[audio]"),
    )
    .expect("graph file");
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-t",
            "0.01",
            "-i",
            "anullsrc=r=48000:cl=stereo",
            "-/filter_complex",
        ])
        .arg(&graph_path)
        .args(["-map", "[audio]", "-f", "null", "-"])
        .output()
        .expect("FFmpeg starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn render_pcm(graph: &super::FfmpegAudioGraph, duration: f64) -> Vec<f32> {
    let mut command = Command::new("ffmpeg");
    command
        .args(["-hide_banner", "-loglevel", "error", "-t"])
        .arg(duration.to_string());
    for path in &graph.input_paths {
        command.arg("-i").arg(path);
    }
    let output = command
        .args([
            "-filter_complex",
            &graph.filter_complex,
            "-map",
            "[audio]",
            "-f",
            "f32le",
            "-acodec",
            "pcm_f32le",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("ffmpeg starts");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
        .stdout
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("f32")))
        .collect()
}

fn write_sine_wav(path: &std::path::Path, frequency: f64, amplitude: f32) {
    let samples = (0..24_000)
        .map(|index| {
            (amplitude as f64 * (std::f64::consts::TAU * frequency * index as f64 / 48_000.0).sin())
                as f32
        })
        .collect::<Vec<_>>();
    write_mono_wav_samples(path, 48_000, &samples);
}

fn tone_correlation(samples: &[f32], frequency: f64) -> f64 {
    let frames = samples.len() / 2;
    (0..frames)
        .map(|index| {
            samples[index * 2] as f64
                * (std::f64::consts::TAU * frequency * index as f64 / 48_000.0).sin()
        })
        .sum::<f64>()
        .abs()
        / frames as f64
}

#[test]
fn parametric_eq_is_lowered_after_trim_before_clip_fader() {
    let mut clip = automation_test_clip();
    clip.effects
        .operations
        .push(AudioEffectOperation::ParametricEq {
            frequency_hz: 120.0,
            gain_db: -12.0,
            q: 0.8,
        });
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("effect graph");
    let trim = graph.filter_complex.find("atrim=").expect("trim");
    let eq = graph.filter_complex.find("equalizer=").expect("eq");
    let fader = graph
        .filter_complex
        .find("volume=volume=1.000")
        .expect("fader");
    let delay = graph.filter_complex.find("adelay=").expect("delay");
    assert!(trim < eq && eq < fader && fader < delay);
}

#[test]
fn playback_speed_preserves_effect_order_and_normalizes_before_fader() {
    let mut clip = automation_test_clip();
    clip.processed_duration = 0.5;
    clip.effects.operations = vec![
        AudioEffectOperation::ParametricEq {
            frequency_hz: 120.0,
            gain_db: -6.0,
            q: 0.8,
        },
        AudioEffectOperation::PlaybackSpeed {
            rate: 2.0,
            link_pitch: false,
        },
        AudioEffectOperation::ParametricEq {
            frequency_hz: 800.0,
            gain_db: 3.0,
            q: 1.0,
        },
    ];
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    };
    let graph = compile(&mix, 1.0).expect("effect graph");
    let first_eq = graph
        .filter_complex
        .find("equalizer=f=120")
        .expect("first eq");
    let tempo = graph.filter_complex.find("atempo=2").expect("tempo");
    let second_eq = graph
        .filter_complex
        .find("equalizer=f=800")
        .expect("second eq");
    let duration = graph
        .filter_complex
        .find("apad,atrim=end_sample=24000")
        .expect("duration");
    let fader = graph
        .filter_complex
        .find("volume=volume=1.000")
        .expect("fader");
    assert!(first_eq < tempo && tempo < second_eq && second_eq < duration && duration < fader);
}

#[test]
fn playback_speed_executes_to_the_planned_sample_count() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let source = directory.path().join("speed.wav");
    write_mono_wav(&source, 48_000, 96_000, 0.1);
    for (rate, processed_duration, expected_samples) in [(2.0, 1.0, 48_000), (0.5, 4.0, 192_000)] {
        let mut clip = automation_test_clip();
        clip.path = source.clone();
        clip.selected_duration = 2.0;
        clip.processed_duration = processed_duration;
        clip.effects.operations = vec![AudioEffectOperation::PlaybackSpeed {
            rate,
            link_pitch: false,
        }];
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip],
                effects: AudioEffectPassPlan::default(),
            }],
            effects: AudioEffectPassPlan::default(),
        };
        let samples = render_pcm(
            &compile(&mix, processed_duration).expect("speed graph"),
            processed_duration,
        );
        assert_eq!(samples.len(), expected_samples * 2);
    }
}

#[test]
fn playback_speed_keeps_fade_and_automation_in_processed_local_time() {
    let mut clip = automation_test_clip();
    clip.selected_duration = 10.0;
    clip.processed_duration = 5.0;
    clip.fade_out = 1.0;
    clip.gain_automation = Some(vestra_core::project::AudioGainAutomation {
        keyframes: vec![
            vestra_core::project::AudioGainKeyframe {
                time: 0.0,
                gain: 1.0,
                interpolation: vestra_core::project::AudioGainInterpolation::Linear,
            },
            vestra_core::project::AudioGainKeyframe {
                time: 2.0,
                gain: 0.5,
                interpolation: vestra_core::project::AudioGainInterpolation::Hold,
            },
        ],
    });
    clip.effects.operations = vec![AudioEffectOperation::PlaybackSpeed {
        rate: 2.0,
        link_pitch: false,
    }];
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    };

    let graph = compile(&mix, 5.0).expect("processed-time graph");
    let fade_start = "(t-4.00000000000000000)/1.00000000000000000";
    assert!(graph.filter_complex.contains(fade_start));
    assert!(graph.filter_complex.contains("lt(t,2.00000000000000000)"));
    assert!(!graph.filter_complex.contains("t-9.00000000000000000"));
    assert!(!graph.filter_complex.contains("t-1.00000000000000000"));
}

#[test]
fn reused_source_speed_is_branch_local_and_keeps_one_input() {
    let mut plain = automation_test_clip();
    plain.id = "plain".to_owned();
    plain.path = PathBuf::from("shared.wav");
    let mut sped = plain.clone();
    sped.id = "sped".to_owned();
    sped.processed_duration = 0.5;
    sped.effects.operations = vec![AudioEffectOperation::PlaybackSpeed {
        rate: 2.0,
        link_pitch: false,
    }];
    let mix = AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![plain, sped],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    };

    let graph = compile(&mix, 1.0).expect("reused source graph");
    let split = graph.filter_complex.find("asplit=2").expect("source split");
    let tempo = graph
        .filter_complex
        .find("atempo=2.00000000000000000")
        .expect("sped branch");
    assert_eq!(graph.input_paths, vec![PathBuf::from("shared.wav")]);
    assert!(split < tempo);
    assert_eq!(graph.filter_complex.matches("atempo=").count(), 1);
    assert!(graph.filter_complex.contains("apad,atrim=end_sample=24000"));
}

#[test]
fn parametric_eq_changes_signal_without_changing_project_frame_count() {
    let directory = tempfile::tempdir().expect("temporary fixtures");
    let source = directory.path().join("eq-tone.wav");
    write_sine_wav(&source, 120.0, 0.5);
    let mut plain_clip = automation_test_clip();
    plain_clip.path = source.clone();
    let mut effected_clip = plain_clip.clone();
    effected_clip
        .effects
        .operations
        .push(AudioEffectOperation::ParametricEq {
            frequency_hz: 120.0,
            gain_db: -24.0,
            q: 1.0,
        });
    let mix = |clip| AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".to_owned(),
            mute: false,
            gain: 1.0,
            clips: vec![clip],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    };
    let plain = render_pcm(&compile(&mix(plain_clip), 1.0).expect("plain graph"), 1.0);
    let effected = render_pcm(&compile(&mix(effected_clip), 1.0).expect("eq graph"), 1.0);
    let rms = |samples: &[f32]| {
        (samples
            .iter()
            .map(|sample| f64::from(*sample).powi(2))
            .sum::<f64>()
            / samples.len() as f64)
            .sqrt()
    };
    assert_eq!(plain.len(), 48_000 * 2);
    assert_eq!(effected.len(), plain.len());
    assert!(rms(&effected[..24_000 * 2]) < rms(&plain[..24_000 * 2]) * 0.5);
}

fn tone_window_correlation(samples: &[f32], frequency: f64, start: usize, frames: usize) -> f64 {
    (start..start + frames)
        .map(|index| {
            samples[index * 2] as f64
                * (std::f64::consts::TAU * frequency * index as f64 / 48_000.0).sin()
        })
        .sum::<f64>()
        .abs()
        / frames as f64
}

fn single_clip_mix(clip: AudioClipPlan) -> AudioMixPlan {
    AudioMixPlan {
        tracks: vec![AudioTrackPlan {
            id: "track".into(),
            mute: false,
            gain: 1.0,
            clips: vec![clip],
            effects: AudioEffectPassPlan::default(),
        }],
        effects: AudioEffectPassPlan::default(),
    }
}

fn stereo_window_peak(samples: &[f32], start: usize, end: usize) -> f32 {
    samples[start * 2..end * 2]
        .iter()
        .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
}

#[test]
fn linked_and_preserving_speed_change_tempo_with_different_pitch() {
    use vestra_core::project::{AudioGainAutomation, AudioGainInterpolation, AudioGainKeyframe};

    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("tone.wav");
    write_sine_wav(&path, 440.0, 0.5);

    for (link_pitch, expected_hz) in [(true, 484.0), (false, 440.0)] {
        let clip = AudioClipPlan {
            path: path.clone(),
            trim_start: 0.05,
            selected_duration: 0.44,
            processed_duration: 0.4,
            fade_out: 0.075,
            gain_automation: Some(AudioGainAutomation {
                keyframes: vec![
                    AudioGainKeyframe {
                        time: 0.0,
                        gain: 1.0,
                        interpolation: AudioGainInterpolation::Hold,
                    },
                    AudioGainKeyframe {
                        time: 0.15,
                        gain: 0.5,
                        interpolation: AudioGainInterpolation::Hold,
                    },
                ],
            }),
            effects: AudioEffectPassPlan {
                operations: vec![AudioEffectOperation::PlaybackSpeed {
                    rate: 1.10,
                    link_pitch,
                }],
            },
            ..automation_test_clip()
        };
        let graph = compile(&single_clip_mix(clip), 0.55).expect("speed graph");
        let samples = render_pcm(&graph, 1.0);
        let frames = samples[2 * 4_800..2 * 14_400].chunks_exact(2);
        let rising_crossings = frames
            .clone()
            .zip(frames.skip(1))
            .filter(|(first, next)| first[0] <= 0.0 && next[0] > 0.0)
            .count();
        let measured_hz = rising_crossings as f64 / 0.2;
        assert!(
            (measured_hz - expected_hz).abs() <= 5.0,
            "expected {expected_hz} Hz, got {measured_hz} Hz"
        );

        assert_eq!(samples.len(), 26_400 * 2);
        assert!(stereo_window_peak(&samples, 19_200, 26_400) < 1e-6);
        assert!(stereo_window_peak(&samples, 18_800, 19_200) > 0.001);
        let before_hold = stereo_window_peak(&samples, 5_760, 6_720);
        let after_hold = stereo_window_peak(&samples, 7_680, 8_640);
        let fade_peak = stereo_window_peak(&samples, 17_280, 18_240);
        assert!((after_hold / before_hold - 0.5).abs() < 0.02);
        assert!(fade_peak < after_hold * 0.6);
    }
}

#[test]
fn fractional_linked_speed_keeps_cues_at_the_authored_rate() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("cue.wav");
    let cue = 47_000;
    let mut source = vec![0.0; 48_000];
    source[cue] = 0.75;
    write_mono_wav_samples(&path, 48_000, &source);

    let rate = 0.250_010_5;
    let operation = AudioEffectOperation::PlaybackSpeed {
        rate,
        link_pitch: true,
    };
    let frames = operation.transform_duration_samples(48_000).unwrap() as usize;
    let clip = AudioClipPlan {
        path,
        selected_duration: 1.0,
        processed_duration: frames as f64 / 48_000.0,
        effects: AudioEffectPassPlan {
            operations: vec![operation],
        },
        ..automation_test_clip()
    };
    let graph = compile(&single_clip_mix(clip), frames as f64 / 48_000.0).expect("speed graph");
    let samples = render_pcm(&graph, 1.0);
    assert_eq!(samples.len(), frames * 2);
    let (peak_frame, _) = samples
        .chunks_exact(2)
        .enumerate()
        .max_by(|(_, a), (_, b)| a[0].abs().total_cmp(&b[0].abs()))
        .unwrap();
    let expected = (cue as f64 / rate).round() as usize;
    assert!(
        peak_frame.abs_diff(expected) <= 1,
        "cue at {peak_frame}, expected {expected}"
    );
}

#[test]
fn room_reverb_decays_in_stereo_and_respects_tail_boundaries() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("impulse.wav");
    let impulse_frame = 2_400;
    write_stereo_wav(
        &path,
        48_000,
        (0..4800).map(|frame| {
            if frame == impulse_frame {
                [0.75, -0.25]
            } else {
                [0.0; 2]
            }
        }),
    );
    let plain = single_clip_mix(AudioClipPlan {
        selected_duration: 0.1,
        processed_duration: 0.1,
        path,
        ..automation_test_clip()
    });
    let effect = AudioEffectOperation::RoomReverb {
        decay_seconds: 0.5,
        mix: 1.0,
    };
    let mut on_clip = plain.clone();
    on_clip.tracks[0].clips[0]
        .effects
        .operations
        .push(effect.clone());
    let mut on_track = plain.clone();
    on_track.tracks[0].effects.operations.push(effect.clone());
    let mut on_master = plain;
    on_master.effects.operations.push(effect);

    let render = |mix: &AudioMixPlan, duration| {
        render_pcm(&compile(mix, duration).expect("room graph"), 1.0)
    };
    let track = render(&on_track, 0.65);
    assert_eq!(track.len(), 31_200 * 2);
    assert!(stereo_window_peak(&track, 0, impulse_frame + 336) < 1e-6);
    let early = stereo_window_peak(&track, impulse_frame + 480, impulse_frame + 1_440);
    let late = stereo_window_peak(&track, impulse_frame + 12_000, impulse_frame + 12_960);
    assert!(early > 0.005);
    assert!(
        late > 1e-5 && late < early * 0.1,
        "early {early}, late {late}"
    );
    assert!(stereo_window_peak(&track, impulse_frame + 24_000, 31_200) < 1e-6);
    for frame in track.chunks_exact(2) {
        assert!((frame[0] + 3.0 * frame[1]).abs() < 1e-5);
    }
    let clip = render(&on_clip, 0.65);
    assert_eq!(clip.len(), track.len());
    assert!(stereo_window_peak(&clip, 3_000, 4_800) > 0.005);
    assert!(stereo_window_peak(&clip, 4_800, 31_200) < 1e-6);

    let master = render(&on_master, 0.15);
    assert_eq!(master.len(), 7_200 * 2);
    assert!(stereo_window_peak(&master, 4_800, 7_200) > 0.001);

    let selected_graph = compile_with_options(
        &on_master,
        0.15,
        AudioGraphCompileOptions {
            first_audio_input_index: 0,
            output_range: Some((0.1, 0.2)),
        },
    )
    .expect("selected room graph");
    let selected = render_pcm(&selected_graph, 1.0);
    assert_eq!(selected.len(), 4_800 * 2);
    assert_eq!(&selected[..4_800], &master[9_600..]);
    assert!(selected[4_800..].iter().all(|sample| *sample == 0.0));
}

#[test]
fn filtered_delay_repeats_on_exact_samples_and_low_passes_only_the_wet_path() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("delay-fixture.wav");
    let tone_frames = 1_920;
    let impulse_frame = 2_304;
    let spacing = seconds_to_samples(0.1113).unwrap() as usize;
    write_stereo_wav(
        &path,
        48_000,
        (0..impulse_frame + 1).map(|frame| {
            if frame == impulse_frame {
                [0.75; 2]
            } else if frame < tone_frames {
                [200.0, 8_000.0].map(|hz| {
                    (0.3 * (std::f64::consts::TAU * hz * frame as f64 / 48_000.0).sin()) as f32
                })
            } else {
                [0.0; 2]
            }
        }),
    );
    let clip_duration = (impulse_frame + 1) as f64 / 48_000.0;
    let mut mix = single_clip_mix(AudioClipPlan {
        path,
        selected_duration: clip_duration,
        processed_duration: clip_duration,
        ..automation_test_clip()
    });
    let effect = AudioEffectOperation::FilteredDelay {
        delay_ms: 111.3,
        repeats: 2,
        mix: 0.5,
        cutoff_hz: 900.0,
    };
    mix.tracks[0].effects.operations.push(effect.clone());

    let samples = render_pcm(&compile(&mix, 0.45).expect("delay graph"), 1.0);
    assert_eq!(samples.len(), 21_600 * 2);
    let channel_peak = |start, end, channel| {
        samples[start * 2..end * 2]
            .chunks_exact(2)
            .fold(0.0_f32, |peak, frame| {
                peak.max(frame[channel as usize].abs())
            })
    };
    assert!(channel_peak(480, 1_440, 1) > 0.125, "dry highs intact");
    assert!(channel_peak(spacing + 480, spacing + 1_440, 0) > 0.08);
    assert!(channel_peak(spacing + 480, spacing + 1_440, 1) < 0.005);
    let impulse_at = impulse_frame + spacing;
    assert!(stereo_window_peak(&samples, impulse_at - 10, impulse_at) < 1e-6);
    assert!(stereo_window_peak(&samples, impulse_at, impulse_at + 1) > 0.0001);
    let first = stereo_window_peak(&samples, impulse_at, impulse_at + 100);
    let second = stereo_window_peak(&samples, impulse_at + spacing, impulse_at + spacing + 100);
    assert!(first > 0.01);
    assert!((second / first - 0.5).abs() < 1e-4);
    assert!(stereo_window_peak(&samples, 15_000, 21_600) < 1e-6);

    mix.tracks[0].effects.operations.clear();
    mix.tracks[0].clips[0]
        .effects
        .operations
        .push(effect.clone());
    let clip = render_pcm(&compile(&mix, 0.45).expect("clip delay graph"), 1.0);
    assert!(stereo_window_peak(&clip, impulse_frame + 1, 21_600) < 1e-6);
    mix.tracks[0].clips[0].effects.operations.clear();
    mix.effects.operations.push(effect);
    let master = render_pcm(&compile(&mix, 0.45).expect("master delay graph"), 1.0);
    assert_eq!(master, samples);
}

#[test]
fn spatial_effects_leave_silence_and_zero_mix_unchanged() {
    use vestra_core::{plan_audio::compile_effects, project::AudioEffect};

    let directory = tempfile::tempdir().expect("temporary audio directory");
    let signal = directory.path().join("signal.wav");
    let silence = directory.path().join("silence.wav");
    write_mono_wav(&signal, 48_000, 4_800, 0.25);
    write_mono_wav(&silence, 48_000, 4_800, 0.0);
    let mut mix = single_clip_mix(AudioClipPlan {
        selected_duration: 0.1,
        processed_duration: 0.1,
        path: signal,
        ..automation_test_clip()
    });
    let dry = render_pcm(&compile(&mix, 0.4).expect("dry graph"), 1.0);
    let spatial_effects = |mix| {
        compile_effects(&[
            AudioEffect::RoomReverb {
                id: "room".into(),
                decay_seconds: 0.5,
                mix,
            },
            AudioEffect::FilteredDelay {
                id: "echo".into(),
                delay_ms: 100.0,
                repeats: 2,
                mix,
                cutoff_hz: 2_000.0,
            },
        ])
    };
    mix.effects = spatial_effects(0.0);
    assert_eq!(render_pcm(&compile(&mix, 0.4).expect("zero mix"), 1.0), dry);
    mix.tracks[0].clips[0].path = silence;
    mix.effects = spatial_effects(0.5);
    let silent = render_pcm(&compile(&mix, 0.4).expect("silence graph"), 1.0);
    assert_eq!(silent.len(), 19_200 * 2);
    assert!(silent.iter().all(|sample| *sample == 0.0));
}

#[test]
fn delay_before_linked_speed_is_scaled_and_clipped_in_effect_order() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("impulse.wav");
    write_stereo_wav(
        &path,
        48_000,
        (0..4_800).map(|frame| if frame == 1_920 { [0.8; 2] } else { [0.0; 2] }),
    );
    let mut mix = single_clip_mix(AudioClipPlan {
        path,
        selected_duration: 0.1,
        processed_duration: 0.05,
        effects: AudioEffectPassPlan {
            operations: vec![
                AudioEffectOperation::FilteredDelay {
                    delay_ms: 40.0,
                    repeats: 2,
                    mix: 1.0,
                    cutoff_hz: 2_000.0,
                },
                AudioEffectOperation::PlaybackSpeed {
                    rate: 2.0,
                    link_pitch: true,
                },
            ],
        },
        ..automation_test_clip()
    });
    let render = |mix: &AudioMixPlan| render_pcm(&compile(mix, 0.1).expect("stack graph"), 1.0);
    let first = render(&mix);
    assert!(stereo_window_peak(&first, 1_900, 2_000) > 0.01);
    assert!(stereo_window_peak(&first, 2_400, 4_800) < 1e-6);

    mix.tracks[0].clips[0].effects.operations.reverse();
    let reversed = render(&mix);
    assert!(reversed.iter().all(|sample| sample.abs() < 1e-6));
}

#[test]
fn linked_speed_keeps_nonempty_sub_filter_length_audio() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("one-frame.wav");
    write_stereo_wav(&path, 48_000, std::iter::once([0.7; 2]));
    let duration = 1.0 / 48_000.0;
    let mix = single_clip_mix(AudioClipPlan {
        path,
        selected_duration: duration,
        processed_duration: duration,
        effects: AudioEffectPassPlan {
            operations: vec![AudioEffectOperation::PlaybackSpeed {
                rate: 4.0,
                link_pitch: true,
            }],
        },
        ..automation_test_clip()
    });
    let graph = compile(&mix, duration).expect("linked speed graph");
    let samples = render_pcm(&graph, 1.0);
    assert_eq!(samples.len(), 2);
    assert!(samples.iter().all(|sample| *sample > 0.1));
}

#[test]
fn preserve_pitch_followed_by_wet_effects_keeps_processed_clip_timing() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("tone.wav");
    write_sine_wav(&path, 440.0, 0.5);
    for (effect, onset) in [
        (
            AudioEffectOperation::RoomReverb {
                decay_seconds: 0.5,
                mix: 1.0,
            },
            2_784,
        ),
        (
            AudioEffectOperation::FilteredDelay {
                delay_ms: 50.0,
                repeats: 1,
                mix: 1.0,
                cutoff_hz: 2_000.0,
            },
            4_800,
        ),
    ] {
        let mix = single_clip_mix(AudioClipPlan {
            path: path.clone(),
            start: 0.05,
            selected_duration: 0.5,
            processed_duration: 0.25,
            fade_out: 0.075,
            effects: AudioEffectPassPlan {
                operations: vec![
                    AudioEffectOperation::PlaybackSpeed {
                        rate: 2.0,
                        link_pitch: false,
                    },
                    effect,
                ],
            },
            ..automation_test_clip()
        });
        let graph = compile(&mix, 0.5).expect("wet speed graph");
        let samples = render_pcm(&graph, 1.0);
        assert_eq!(samples.len(), 48_000);
        assert!(stereo_window_peak(&samples, 0, onset) < 1e-6);
        assert!(stereo_window_peak(&samples, onset, onset + 960) > 0.005);
        let steady = stereo_window_peak(&samples, 9_900, 10_860);
        let faded = stereo_window_peak(&samples, 13_680, 14_160);
        assert!(faded > 0.001 && faded < steady * 0.3);
        assert!(stereo_window_peak(&samples, 14_400, 24_000) < 1e-6);
    }
}

#[test]
fn raw_spatial_plan_rejects_invalid_effect_parameters() {
    let mut mix = single_clip_mix(automation_test_clip());
    for (effect, parameter) in [
        (
            AudioEffectOperation::RoomReverb {
                decay_seconds: 0.001,
                mix: 0.5,
            },
            "decay_seconds",
        ),
        (
            AudioEffectOperation::FilteredDelay {
                delay_ms: 100.0,
                repeats: 255,
                mix: 0.5,
                cutoff_hz: 900.0,
            },
            "repeats",
        ),
    ] {
        mix.effects.operations = vec![effect];
        let error = compile(&mix, 0.5).unwrap_err();
        assert!(error.to_string().contains(parameter));
    }
}

#[test]
fn room_reverb_keeps_wet_energy_across_decay_limits_for_short_clips() {
    let directory = tempfile::tempdir().expect("temporary audio directory");
    let path = directory.path().join("short-impulse.wav");
    write_stereo_wav(&path, 48_000, std::iter::once([0.75, 0.0]));
    let mut mix = single_clip_mix(AudioClipPlan {
        path,
        selected_duration: 1.0 / 48_000.0,
        processed_duration: 1.0 / 48_000.0,
        ..automation_test_clip()
    });
    let project_duration = 2.1;
    for decay_seconds in [0.1, 2.0] {
        mix.effects.operations = vec![AudioEffectOperation::RoomReverb {
            decay_seconds,
            mix: 1.0,
        }];
        let graph = compile(&mix, project_duration).expect("short impulse graph");
        let samples = render_pcm(&graph, 1.0);
        assert_eq!(samples.len(), 100_800 * 2);
        let decay_frames = seconds_to_samples(decay_seconds).unwrap() as usize;
        let energy = samples[..decay_frames * 2]
            .chunks_exact(2)
            .map(|frame| f64::from(frame[0] / 0.75).powi(2))
            .sum::<f64>();
        assert!((0.8..1.2).contains(&energy), "wet impulse energy: {energy}");
        assert!(stereo_window_peak(&samples, decay_frames, 100_800) < 1e-6);
    }
}

#[path = "audio_graph/remix_tone_tests.rs"]
mod remix_tone;
