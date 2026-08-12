//! FFmpeg-specific translation of the backend-neutral audio mixer plan.

use std::{collections::HashMap, path::PathBuf};

use vestra_core::{
    plan_audio::{
        AudioClipPlan, AudioEffectOperation, AudioEffectPassPlan, AudioMixPlan,
        MASTER_AUDIO_SAMPLE_RATE,
    },
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
        let track_effects = graph.apply_effect_chain(track_mix, &track.effects)?;
        let track_label = graph.label("track");
        graph.filters.push(format!(
            "[{track_effects}]volume=volume={}[{track_label}]",
            number(track.gain)
        ));
        tracks.push(track_label);
    }
    let master = graph.mix(&tracks, "master").ok_or_else(|| {
        MediaError::InvalidAudioTiming("audio graph has no audible contributors".to_owned())
    })?;
    let master_effects = graph.apply_effect_chain(master, &mix.effects)?;
    graph.filters.push(format!(
        "[{master_effects}]apad=whole_len={project_samples},atrim=end_sample={project_samples}[audio]"
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
        let processed = seconds_to_samples(clip.processed_duration)?;
        // Semantic validation works in seconds. Independent edge rounding can
        // make two exactly-fitting fades exceed the selected interval by one
        // sample, so preserve fade-in first and shorten fade-out only enough
        // to fit the selected sample interval.
        let fade_in = seconds_to_samples(clip.fade_in)?.min(processed);
        let requested_fade_out = seconds_to_samples(clip.fade_out)?;
        let fade_out = requested_fade_out.min(processed.saturating_sub(fade_in));
        let normalized = self.label("clip");
        self.filters.push(format!(
            "[{source_branch}]atrim=start_sample={trim_start}:end_sample={trim_end},asetpts=PTS-STARTPTS[{normalized}]"
        ));
        let effected = self.apply_effect_chain(normalized, &clip.effects)?;
        let effected = if clip.effects.has_duration_transform() {
            let output = self.label("duration");
            self.filters.push(format!(
                "[{effected}]apad,atrim=end_sample={processed},asetpts=PTS-STARTPTS[{output}]"
            ));
            output
        } else {
            effected
        };
        let label = self.label("clip");
        let mut filters = vec![format!("[{effected}]volume=volume={}", number(clip.gain))];
        let envelope = envelope_expression(
            clip.gain_automation.as_ref(),
            fade_in,
            fade_out,
            processed,
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

    fn apply_effect_chain(
        &mut self,
        mut input: String,
        effects: &AudioEffectPassPlan,
    ) -> Result<String, MediaError> {
        for operation in &effects.operations {
            let output = self.label("effect");
            let filter = match operation {
                AudioEffectOperation::ParametricEq {
                    frequency_hz,
                    gain_db,
                    q,
                } => format!(
                    "[{input}]equalizer=f={}:width_type=q:width={}:g={}[{output}]",
                    number(*frequency_hz),
                    number(*q),
                    number(*gain_db)
                ),
                AudioEffectOperation::PlaybackSpeed { rate } => {
                    let stages = decompose_atempo(*rate)?;
                    let mut current = input.clone();
                    for stage in stages {
                        let staged = self.label("tempo");
                        self.filters
                            .push(format!("[{current}]atempo={}[{staged}]", number(stage)));
                        current = staged;
                    }
                    input = current;
                    continue;
                }
            };
            self.filters.push(filter);
            input = output;
        }
        Ok(input)
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

fn decompose_atempo(rate: f64) -> Result<Vec<f64>, MediaError> {
    if !rate.is_finite() || rate <= 0.0 {
        return Err(MediaError::InvalidAudioTiming(
            "playback speed rate must be finite and positive".to_owned(),
        ));
    }
    let mut remaining = rate;
    let mut stages = Vec::new();
    while remaining > 2.0 {
        stages.push(2.0);
        remaining /= 2.0;
    }
    while remaining < 0.5 {
        stages.push(0.5);
        remaining /= 0.5;
    }
    if (remaining - 1.0).abs() > f64::EPSILON {
        stages.push(remaining);
    }
    Ok(stages)
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
    keyframes: &[vestra_core::project::AudioGainKeyframe],
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
    keyframes: &[vestra_core::project::AudioGainKeyframe],
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
#[path = "audio_graph_tests.rs"]
mod tests;
