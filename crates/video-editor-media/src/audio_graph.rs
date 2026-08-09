//! FFmpeg-specific translation of the backend-neutral audio mixer plan.

use std::{collections::HashMap, path::PathBuf};

use video_editor_core::{
    plan_audio::{AudioClipPlan, AudioMixPlan, MASTER_AUDIO_SAMPLE_RATE},
    project::{AudioFadeCurve, AudioGainAutomation, AudioGainInterpolation},
};

use crate::MediaError;

const MIX_SAMPLE_RATE: u64 = MASTER_AUDIO_SAMPLE_RATE as u64;

/// The graph owns mixer semantics, while its caller owns FFmpeg input layout.
/// Encoders place audio after video at index 1; standalone analysis starts at 0.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AudioGraphCompileOptions {
    pub(crate) first_audio_input_index: usize,
}

#[derive(Debug)]
pub(crate) struct FfmpegAudioGraph {
    pub(crate) input_paths: Vec<PathBuf>,
    pub(crate) filter_complex: String,
    pub(crate) clip_branch_count: usize,
}

/// Convert non-negative timeline seconds to the nearest 48 kHz sample. Ties
/// round away from zero, which for the schema's non-negative times is upward.
pub fn seconds_to_samples(seconds: f64) -> Result<u64, MediaError> {
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(MediaError::InvalidAudioTiming(format!(
            "seconds must be finite and non-negative, got {seconds}"
        )));
    }
    let rounded = (seconds * MIX_SAMPLE_RATE as f64).round();
    if !rounded.is_finite() || rounded > u64::MAX as f64 {
        return Err(MediaError::InvalidAudioTiming(format!(
            "{seconds} seconds cannot be represented at {MIX_SAMPLE_RATE} Hz"
        )));
    }
    Ok(rounded as u64)
}

#[cfg(test)]
pub(crate) fn compile(
    mix: &AudioMixPlan,
    project_duration: f64,
) -> Result<FfmpegAudioGraph, MediaError> {
    compile_with_options(
        mix,
        project_duration,
        AudioGraphCompileOptions {
            first_audio_input_index: 0,
        },
    )
}

pub(crate) fn compile_with_options(
    mix: &AudioMixPlan,
    project_duration: f64,
    options: AudioGraphCompileOptions,
) -> Result<FfmpegAudioGraph, MediaError> {
    let project_samples = seconds_to_samples(project_duration)?;
    if project_samples == 0 {
        return Err(MediaError::InvalidAudioTiming(
            "project duration rounds to zero samples".to_owned(),
        ));
    }
    let mut graph = GraphBuilder::new(options.first_audio_input_index);
    // Register sources before emitting filters. This preserves first-use order
    // while allowing each input to fan out to its clip-local branches.
    for track in &mix.tracks {
        if track.mute || track.gain == 0.0 {
            continue;
        }
        for clip in &track.clips {
            if !clip.mute && clip.gain != 0.0 {
                graph.register_source(&clip.path);
            }
        }
    }
    graph.emit_source_filters();
    let mut tracks = Vec::new();
    for track in &mix.tracks {
        if track.mute || track.gain == 0.0 {
            continue;
        }
        let mut clips = Vec::new();
        for clip in &track.clips {
            if clip.mute || clip.gain == 0.0 {
                continue;
            }
            clips.push(graph.clip(clip)?);
        }
        let Some(track_mix) = graph.mix(&clips, "track") else {
            continue;
        };
        let track_label = graph.label("track");
        graph.filters.push(format!(
            "[{track_mix}]volume=volume={}[{track_label}]",
            number(track.gain)
        ));
        tracks.push(track_label);
    }
    let master = graph.mix(&tracks, "master").ok_or_else(|| {
        MediaError::InvalidAudioTiming("audio graph has no audible contributors".to_owned())
    })?;
    graph.filters.push(format!(
        "[{master}]apad=whole_len={project_samples},atrim=end_sample={project_samples}[audio]"
    ));
    Ok(FfmpegAudioGraph {
        input_paths: graph
            .sources
            .iter()
            .map(|source| source.path.clone())
            .collect(),
        filter_complex: graph.filters.join(";"),
        clip_branch_count: graph.clip_branch_count,
    })
}

struct GraphBuilder {
    first_audio_input_index: usize,
    sources: Vec<Source>,
    source_indexes: HashMap<PathBuf, usize>,
    filters: Vec<String>,
    label_number: usize,
    clip_branch_count: usize,
}

struct Source {
    path: PathBuf,
    branch_count: usize,
    branches: Vec<String>,
    next_branch: usize,
}

impl GraphBuilder {
    fn new(first_audio_input_index: usize) -> Self {
        Self {
            first_audio_input_index,
            sources: Vec::new(),
            source_indexes: HashMap::new(),
            filters: Vec::new(),
            label_number: 0,
            clip_branch_count: 0,
        }
    }

    fn label(&mut self, kind: &str) -> String {
        let label = format!("a_{kind}_{:06}", self.label_number);
        self.label_number += 1;
        label
    }

    fn register_source(&mut self, path: &PathBuf) {
        if let Some(index) = self.source_indexes.get(path) {
            self.sources[*index].branch_count += 1;
            return;
        }
        let index = self.sources.len();
        self.sources.push(Source {
            path: path.clone(),
            branch_count: 1,
            branches: Vec::new(),
            next_branch: 0,
        });
        self.source_indexes.insert(path.clone(), index);
    }

    fn emit_source_filters(&mut self) {
        for source_index in 0..self.sources.len() {
            let input_index = self.first_audio_input_index + source_index;
            let normalized = self.label("source");
            self.filters.push(format!(
                "[{input_index}:a]aformat=sample_rates={MIX_SAMPLE_RATE}:sample_fmts=fltp:channel_layouts=stereo[{normalized}]"
            ));
            let branch_count = self.sources[source_index].branch_count;
            let branches = if branch_count == 1 {
                vec![normalized]
            } else {
                let branches = (0..branch_count)
                    .map(|_| self.label("branch"))
                    .collect::<Vec<_>>();
                self.filters.push(format!(
                    "[{normalized}]asplit={branch_count}{}",
                    branches
                        .iter()
                        .map(|branch| format!("[{branch}]"))
                        .collect::<String>()
                ));
                branches
            };
            self.sources[source_index].branches = branches;
        }
    }

    fn next_branch(&mut self, path: &PathBuf) -> Result<String, MediaError> {
        let source_index = *self.source_indexes.get(path).ok_or_else(|| {
            MediaError::InvalidAudioTiming("audio source was not registered".to_owned())
        })?;
        let source = &mut self.sources[source_index];
        let branch = source
            .branches
            .get(source.next_branch)
            .cloned()
            .ok_or_else(|| {
                MediaError::InvalidAudioTiming(
                    "audio source branch allocation overflowed".to_owned(),
                )
            })?;
        source.next_branch += 1;
        self.clip_branch_count += 1;
        Ok(branch)
    }

    fn clip(&mut self, clip: &AudioClipPlan) -> Result<String, MediaError> {
        let source_branch = self.next_branch(&clip.path)?;
        let trim_start = seconds_to_samples(clip.trim_start)?;
        let trim_end = seconds_to_samples(clip.trim_start + clip.selected_duration)?;
        let selected = trim_end.checked_sub(trim_start).ok_or_else(|| {
            MediaError::InvalidAudioTiming(
                "clip trim end precedes trim start after sample rounding".to_owned(),
            )
        })?;
        if selected == 0 {
            return Err(MediaError::InvalidAudioTiming(
                "clip selection rounds to zero samples".to_owned(),
            ));
        }
        let timeline_start = seconds_to_samples(clip.start)?;
        // Semantic validation works in seconds. Independent edge rounding can
        // make two exactly-fitting fades exceed the selected interval by one
        // sample, so preserve fade-in first and shorten fade-out only enough
        // to fit the selected sample interval.
        let fade_in = seconds_to_samples(clip.fade_in)?.min(selected);
        let requested_fade_out = seconds_to_samples(clip.fade_out)?;
        let fade_out = requested_fade_out.min(selected.saturating_sub(fade_in));
        let label = self.label("clip");
        let mut filters = vec![format!(
            "[{source_branch}]atrim=start_sample={trim_start}:end_sample={trim_end},asetpts=PTS-STARTPTS,volume=volume={}",
            number(clip.gain)
        )];
        let envelope = envelope_expression(
            clip.gain_automation.as_ref(),
            fade_in,
            fade_out,
            selected,
            clip,
        )?;
        if envelope != "1" {
            // `aeval` evaluates its expression for every input sample. `volume`
            // cannot do this: it only supports once or audio-frame evaluation.
            filters.push(format!(
                "aeval=exprs='val(0)*({envelope})|val(1)*({envelope})':c=stereo,aformat=sample_rates={MIX_SAMPLE_RATE}:sample_fmts=fltp:channel_layouts=stereo"
            ));
        }
        filters.push(format!("adelay={timeline_start}S:all=1[{label}]"));
        self.filters.push(filters.join(","));
        Ok(label)
    }

    fn mix(&mut self, inputs: &[String], kind: &str) -> Option<String> {
        match inputs {
            [] => None,
            [input] => Some(input.clone()),
            _ => {
                let label = self.label(kind);
                self.filters.push(format!(
                    "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[{label}]",
                    inputs
                        .iter()
                        .map(|input| format!("[{input}]"))
                        .collect::<String>(),
                    inputs.len()
                ));
                Some(label)
            }
        }
    }
}

fn envelope_expression(
    automation: Option<&AudioGainAutomation>,
    fade_in: u64,
    fade_out: u64,
    selected: u64,
    clip: &AudioClipPlan,
) -> Result<String, MediaError> {
    let mut factors = Vec::new();
    if let Some(automation) = automation {
        factors.push(automation_expression(automation, clip)?);
    }
    if fade_in > 0 {
        let duration = number(fade_in as f64 / MIX_SAMPLE_RATE as f64);
        factors.push(fade_expression(true, &clip.fade_in_curve, "t", &duration));
    }
    if fade_out > 0 {
        let start = number((selected - fade_out) as f64 / MIX_SAMPLE_RATE as f64);
        let duration = number(fade_out as f64 / MIX_SAMPLE_RATE as f64);
        let progress = format!("(t-{start})/{duration}");
        factors.push(format!(
            "if(lt(t,{start}),1,{})",
            fade_expression(false, &clip.fade_out_curve, &progress, "1")
        ));
    }
    if factors.is_empty() {
        Ok("1".to_owned())
    } else {
        Ok(factors.join("*"))
    }
}

fn automation_expression(
    automation: &AudioGainAutomation,
    clip: &AudioClipPlan,
) -> Result<String, MediaError> {
    if automation.keyframes.is_empty() {
        return Ok("1".to_owned());
    }

    let sample_times = automation
        .keyframes
        .iter()
        .map(|keyframe| seconds_to_samples(keyframe.time))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, pair) in sample_times.windows(2).enumerate() {
        if pair[1] <= pair[0] {
            return Err(MediaError::InvalidAudioTiming(format!(
                "gain automation for audio clip '{}' has keyframes {} and {} at the same 48 kHz mixer sample",
                clip.id,
                index,
                index + 1
            )));
        }
    }

    // There is one leaf for every linear or hold segment plus the final hold.
    // A midpoint dispatch keeps parser nesting logarithmic while this single
    // String is appended in-place, avoiding repeated copies of completed trees.
    let mut expression = String::new();
    append_automation_tree(
        &mut expression,
        &automation.keyframes,
        &sample_times,
        0,
        automation.keyframes.len(),
    );
    Ok(expression)
}

fn append_automation_tree(
    output: &mut String,
    keyframes: &[video_editor_core::project::AudioGainKeyframe],
    sample_times: &[u64],
    start: usize,
    end: usize,
) {
    if end - start == 1 {
        append_automation_leaf(output, keyframes, sample_times, start);
        return;
    }
    let middle = start + (end - start) / 2;
    output.push_str("if(lt(t,");
    output.push_str(&number(
        sample_times[middle] as f64 / MIX_SAMPLE_RATE as f64,
    ));
    output.push_str("),");
    append_automation_tree(output, keyframes, sample_times, start, middle);
    output.push(',');
    append_automation_tree(output, keyframes, sample_times, middle, end);
    output.push(')');
}

fn append_automation_leaf(
    output: &mut String,
    keyframes: &[video_editor_core::project::AudioGainKeyframe],
    sample_times: &[u64],
    index: usize,
) {
    let first = &keyframes[index];
    if index + 1 == keyframes.len() {
        output.push_str(&number(first.gain));
        return;
    }
    match first.interpolation {
        AudioGainInterpolation::Hold => output.push_str(&number(first.gain)),
        AudioGainInterpolation::Linear => {
            let second = &keyframes[index + 1];
            let first_time = number(sample_times[index] as f64 / MIX_SAMPLE_RATE as f64);
            let second_time = number(sample_times[index + 1] as f64 / MIX_SAMPLE_RATE as f64);
            output.push_str(&number(first.gain));
            output.push_str("+(");
            output.push_str(&number(second.gain));
            output.push('-');
            output.push_str(&number(first.gain));
            output.push_str(")*(t-");
            output.push_str(&first_time);
            output.push_str(")/(");
            output.push_str(&second_time);
            output.push('-');
            output.push_str(&first_time);
            output.push(')');
        }
    }
}

fn fade_expression(
    incoming: bool,
    curve: &AudioFadeCurve,
    progress: &str,
    linear_duration: &str,
) -> String {
    let progress = if linear_duration == "1" {
        progress.to_owned()
    } else {
        format!("({progress})/{linear_duration}")
    };
    match (incoming, curve) {
        (true, AudioFadeCurve::Linear) => format!("min(1,{progress})"),
        (false, AudioFadeCurve::Linear) => format!("max(0,1-({progress}))"),
        (true, AudioFadeCurve::EqualPower) => format!("sin(PI*min(1,{progress})/2)"),
        (false, AudioFadeCurve::EqualPower) => format!("cos(PI*min(1,{progress})/2)"),
    }
}

fn number(value: f64) -> String {
    format!("{value:.17}")
}

#[cfg(test)]
mod tests {
    use super::{
        AudioGraphCompileOptions, automation_expression, compile, compile_with_options,
        seconds_to_samples,
    };
    use std::{
        fs,
        path::PathBuf,
        process::{Command, Stdio},
    };
    use video_editor_core::plan_audio::{AudioClipPlan, AudioMixPlan, AudioTrackPlan};

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
        let automation = video_editor_core::project::AudioGainAutomation {
            keyframes: (0..128)
                .map(|index| video_editor_core::project::AudioGainKeyframe {
                    time: index as f64 * 0.01,
                    gain: if index % 2 == 0 { 0.25 } else { 0.75 },
                    interpolation: video_editor_core::project::AudioGainInterpolation::Hold,
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
                    gain_automation: Some(automation),
                    ..automation_test_clip()
                }],
            }],
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
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![
                AudioTrackPlan {
                    id: "first".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("a", "a.wav"), clip("b", "b.wav")],
                },
                AudioTrackPlan {
                    id: "second".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("c", "c.wav")],
                },
            ],
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
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![
                AudioTrackPlan {
                    id: "first".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("clip-b", "shared.wav"), clip("clip-a", "other.wav")],
                },
                AudioTrackPlan {
                    id: "second".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("clip-c", "shared.wav"), clip("clip-d", "shared.wav")],
                },
            ],
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
            mute: false,
            gain: 0.5,
            gain_automation: None,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 0.75,
                clips: vec![clip("first", "first.wav"), clip("second", "second.wav")],
            }],
        };
        let encoder = compile_with_options(
            &mix,
            2.0,
            AudioGraphCompileOptions {
                first_audio_input_index: 1,
            },
        )
        .expect("encoder graph");
        let analyzer = compile_with_options(
            &mix,
            2.0,
            AudioGraphCompileOptions {
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
                        mute: false,
                        gain: 1.0,
                        fade_in: 0.0,
                        fade_out: 0.0,
                        gain_automation: None,
                        fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
                        fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
                    })
                    .collect(),
            })
            .collect();
        let mix = AudioMixPlan { tracks };
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
                    mute: false,
                    gain: 1.0,
                    fade_in: 0.5 / 48_000.0,
                    fade_out: 0.5 / 48_000.0,
                    gain_automation: None,
                    fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
                    fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
                }],
            }],
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
                    mute: false,
                    gain: 0.5,
                    gain_automation: Some(video_editor_core::project::AudioGainAutomation {
                        keyframes: vec![
                            video_editor_core::project::AudioGainKeyframe {
                                time: 0.0,
                                gain: 0.0,
                                interpolation:
                                    video_editor_core::project::AudioGainInterpolation::Linear,
                            },
                            video_editor_core::project::AudioGainKeyframe {
                                time: 0.12345,
                                gain: 1.0,
                                interpolation:
                                    video_editor_core::project::AudioGainInterpolation::Hold,
                            },
                            video_editor_core::project::AudioGainKeyframe {
                                time: 0.5,
                                gain: 0.25,
                                interpolation:
                                    video_editor_core::project::AudioGainInterpolation::Linear,
                            },
                        ],
                    }),
                    fade_in: 0.5,
                    fade_out: 0.5,
                    fade_in_curve: video_editor_core::project::AudioFadeCurve::EqualPower,
                    fade_out_curve: video_editor_core::project::AudioFadeCurve::EqualPower,
                }],
            }],
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
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![
                AudioTrackPlan {
                    id: "one".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("a", 0.12345)],
                },
                AudioTrackPlan {
                    id: "two".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("b", 0.12345)],
                },
            ],
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
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 0.5,
                clips: vec![clip("a"), clip("b")],
            }],
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
                    mute: false,
                    gain: 0.5,
                    fade_in: 0.1,
                    fade_out: 0.1,
                    gain_automation: None,
                    fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
                    fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
                }],
            }],
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
            mute: false,
            gain: 1.0,
            gain_automation: automation,
            fade_in,
            fade_out,
            fade_in_curve,
            fade_out_curve,
        };
        let automation = video_editor_core::project::AudioGainAutomation {
            keyframes: vec![
                video_editor_core::project::AudioGainKeyframe {
                    time: 0.0,
                    gain: 0.0,
                    interpolation: video_editor_core::project::AudioGainInterpolation::Linear,
                },
                video_editor_core::project::AudioGainKeyframe {
                    time: 1.0,
                    gain: 1.0,
                    interpolation: video_editor_core::project::AudioGainInterpolation::Hold,
                },
                video_editor_core::project::AudioGainKeyframe {
                    time: 1.12345,
                    gain: 0.25,
                    interpolation: video_editor_core::project::AudioGainInterpolation::Linear,
                },
            ],
        };
        let mix = |clip| AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip],
            }],
        };
        let automated = render_pcm(
            &compile(
                &mix(clip(
                    Some(automation),
                    0.0,
                    0.0,
                    video_editor_core::project::AudioFadeCurve::Linear,
                    video_editor_core::project::AudioFadeCurve::Linear,
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

        let descending = video_editor_core::project::AudioGainAutomation {
            keyframes: vec![
                video_editor_core::project::AudioGainKeyframe {
                    time: 0.0,
                    gain: 1.0,
                    interpolation: video_editor_core::project::AudioGainInterpolation::Linear,
                },
                video_editor_core::project::AudioGainKeyframe {
                    time: 1.0,
                    gain: 0.0,
                    interpolation: video_editor_core::project::AudioGainInterpolation::Linear,
                },
            ],
        };
        let descending_pcm = render_pcm(
            &compile(
                &mix(clip(
                    Some(descending),
                    0.0,
                    0.0,
                    video_editor_core::project::AudioFadeCurve::Linear,
                    video_editor_core::project::AudioFadeCurve::Linear,
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
                    video_editor_core::project::AudioFadeCurve::EqualPower,
                    video_editor_core::project::AudioFadeCurve::Linear,
                )),
                2.0,
            )
            .expect("fade graph"),
            2.0,
        );
        let reference = channel(&equal, 72_000);
        assert!(
            (channel(&equal, 24_000) / reference - std::f32::consts::FRAC_1_SQRT_2).abs() < 0.02
        );
        let linear = render_pcm(
            &compile(
                &mix(clip(
                    None,
                    1.0,
                    0.0,
                    video_editor_core::project::AudioFadeCurve::Linear,
                    video_editor_core::project::AudioFadeCurve::Linear,
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
                    mute: false,
                    gain: 0.5,
                    gain_automation: Some(video_editor_core::project::AudioGainAutomation {
                        keyframes: vec![video_editor_core::project::AudioGainKeyframe {
                            time: 0.0,
                            gain: 0.5,
                            interpolation:
                                video_editor_core::project::AudioGainInterpolation::Linear,
                        }],
                    }),
                    fade_in: 0.0,
                    fade_out: 0.0,
                    fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
                    fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
                }],
            }],
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
            mute: false,
            gain: 1.0,
            gain_automation: None,
            fade_in,
            fade_out,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::EqualPower,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::EqualPower,
        };
        let mix = |clips| AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips,
            }],
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
            mute: false,
            gain,
            fade_in,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
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
            }],
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
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("early", 0.0, 0.1), clip("late", 0.5, 1.0)],
            }],
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
    fn pcm_graph_normalizes_44100_mono_to_48000_stereo() {
        let directory = tempfile::tempdir().expect("temporary fixtures");
        let mono = directory.path().join("mono-44100.wav");
        let stereo_source = directory.path().join("stereo-48000.wav");
        write_mono_wav(&mono, 44_100, 44_100, 0.08);
        write_stereo_wav(&stereo_source, 48_000, 48_000, 0.06);
        let clip = |id: &str, path: PathBuf| AudioClipPlan {
            id: id.to_owned(),
            asset: id.to_owned(),
            path,
            start: 0.0,
            trim_start: 0.0,
            selected_duration: 0.5,
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![
                AudioTrackPlan {
                    id: "mono".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("mono", mono)],
                },
                AudioTrackPlan {
                    id: "other".to_owned(),
                    mute: false,
                    gain: 1.0,
                    clips: vec![clip("other", stereo_source)],
                },
            ],
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
            mute: false,
            gain: 1.0,
            fade_in: 0.0,
            fade_out: 0.0,
            gain_automation: None,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
        };
        let mix = AudioMixPlan {
            tracks: vec![AudioTrackPlan {
                id: "same-track".to_owned(),
                mute: false,
                gain: 1.0,
                clips: vec![clip("low", low), clip("high", high)],
            }],
        };
        let samples = render_pcm(&compile(&mix, 0.5).expect("production graph"), 0.5);
        assert!(tone_correlation(&samples, 440.0) > 0.01);
        assert!(tone_correlation(&samples, 880.0) > 0.01);
    }

    fn automation_with_keyframes(count: usize) -> video_editor_core::project::AudioGainAutomation {
        video_editor_core::project::AudioGainAutomation {
            keyframes: (0..count)
                .map(|index| video_editor_core::project::AudioGainKeyframe {
                    time: index as f64 / 48_000.0,
                    gain: if index % 2 == 0 { 0.25 } else { 0.75 },
                    interpolation: if index % 3 == 0 {
                        video_editor_core::project::AudioGainInterpolation::Hold
                    } else {
                        video_editor_core::project::AudioGainInterpolation::Linear
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
            mute: false,
            gain: 1.0,
            gain_automation: None,
            fade_in: 0.0,
            fade_out: 0.0,
            fade_in_curve: video_editor_core::project::AudioFadeCurve::Linear,
            fade_out_curve: video_editor_core::project::AudioFadeCurve::Linear,
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

    fn write_mono_wav(path: &std::path::Path, sample_rate: u32, samples: usize, amplitude: f32) {
        write_mono_wav_samples(path, sample_rate, &vec![amplitude; samples]);
    }

    fn write_mono_wav_samples(path: &std::path::Path, sample_rate: u32, samples: &[f32]) {
        let data_length = (samples.len() * 2) as u32;
        let mut bytes = Vec::with_capacity(44 + data_length as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for sample in samples {
            let pcm = (sample * i16::MAX as f32).round() as i16;
            bytes.extend_from_slice(&pcm.to_le_bytes());
        }
        fs::write(path, bytes).expect("fixture WAV");
    }

    fn write_stereo_wav(path: &std::path::Path, sample_rate: u32, samples: usize, amplitude: f32) {
        let pcm = (amplitude * i16::MAX as f32).round() as i16;
        let data_length = (samples * 4) as u32;
        let mut bytes = Vec::with_capacity(44 + data_length as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_length).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&(sample_rate * 4).to_le_bytes());
        bytes.extend_from_slice(&4_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_length.to_le_bytes());
        for _ in 0..samples {
            bytes.extend_from_slice(&pcm.to_le_bytes());
            bytes.extend_from_slice(&pcm.to_le_bytes());
        }
        fs::write(path, bytes).expect("fixture WAV");
    }

    fn write_sine_wav(path: &std::path::Path, frequency: f64, amplitude: f32) {
        let samples = (0..24_000)
            .map(|index| {
                (amplitude as f64
                    * (std::f64::consts::TAU * frequency * index as f64 / 48_000.0).sin())
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

    fn tone_window_correlation(
        samples: &[f32],
        frequency: f64,
        start: usize,
        frames: usize,
    ) -> f64 {
        (start..start + frames)
            .map(|index| {
                samples[index * 2] as f64
                    * (std::f64::consts::TAU * frequency * index as f64 / 48_000.0).sin()
            })
            .sum::<f64>()
            .abs()
            / frames as f64
    }
}
