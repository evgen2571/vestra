//! FFmpeg-specific translation of the backend-neutral audio mixer plan.

use std::{collections::HashMap, path::PathBuf};

use vestra_core::{
    plan_audio::{
        AudioClipPlan, AudioEffectOperation, AudioEffectPassPlan, AudioMixPlan,
        MASTER_AUDIO_SAMPLE_RATE,
    },
    project::{AudioEffect, AudioFadeCurve, AudioGainAutomation, AudioGainInterpolation},
};

use crate::MediaError;

mod remix_edits;
mod remix_tone;

const MIX_SAMPLE_RATE: u64 = MASTER_AUDIO_SAMPLE_RATE as u64;
// FFmpeg takes integer sample rates. Scale both virtual clocks to preserve the ratio.
const LINKED_SPEED_CLOCK: u64 = 100_000_000;

/// The graph owns mixer semantics, while its caller owns FFmpeg input layout.
/// Encoders place audio after video at index 1; standalone analysis starts at 0.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AudioGraphCompileOptions {
    pub(crate) first_audio_input_index: usize,
    pub(crate) output_range: Option<(f64, f64)>,
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
            output_range: None,
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
        let track_effects = graph.apply_effect_chain(track_mix, &track.effects, None)?;
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
    let master_effects = graph.apply_effect_chain(master, &mix.effects, None)?;
    let mixed_output = if options.output_range.is_some() {
        "timeline_audio"
    } else {
        "audio"
    };
    graph.filters.push(format!(
        "[{master_effects}]apad=whole_len={project_samples},atrim=end_sample={project_samples}[{mixed_output}]"
    ));
    if let Some((start, end)) = options.output_range {
        let start = seconds_to_samples(start)?;
        let end = seconds_to_samples(end)?;
        if start >= end {
            return Err(MediaError::InvalidAudioTiming(
                "output range contains no audio samples".to_owned(),
            ));
        }
        graph.filters.push(format!(
            "[timeline_audio]apad=whole_len={end},atrim=start_sample={start}:end_sample={end},asetpts=PTS-STARTPTS[audio]"
        ));
    }
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
            let branches = self.split(normalized, branch_count);
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
            "[{source_branch}]atrim=start_sample={trim_start}:end_sample={trim_end},asetpts=N/SR/TB[{normalized}]"
        ));
        let effected = self.apply_effect_chain(
            normalized,
            &clip.effects,
            Some(seconds_to_samples(clip.selected_duration)?),
        )?;
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
        // FFmpeg's synthetic delay frames can have no usable timestamps.
        filters.push(format!(
            "adelay={timeline_start}S:all=1,asetpts=N/SR/TB[{label}]"
        ));
        self.filters.push(filters.join(","));
        Ok(label)
    }

    fn filter(&mut self, input: &str, filter: &str) -> String {
        let output = self.label("effect");
        self.filters.push(format!("[{input}]{filter}[{output}]"));
        output
    }

    fn split(&mut self, input: String, count: usize) -> Vec<String> {
        if count == 1 {
            return vec![input];
        }
        let branches = (0..count).map(|_| self.label("branch")).collect::<Vec<_>>();
        self.filters.push(format!(
            "[{input}]asplit={count}{}",
            branches
                .iter()
                .map(|branch| format!("[{branch}]"))
                .collect::<String>(),
        ));
        branches
    }

    fn apply_effect_chain(
        &mut self,
        mut input: String,
        effects: &AudioEffectPassPlan,
        mut clip_samples: Option<u64>,
    ) -> Result<String, MediaError> {
        for operation in &effects.operations {
            input = match *operation {
                AudioEffectOperation::ParametricEq {
                    frequency_hz,
                    gain_db,
                    q,
                } => self.filter(
                    &input,
                    &format!(
                        "equalizer=f={}:width_type=q:width={}:g={}",
                        number(frequency_hz),
                        number(q),
                        number(gain_db),
                    ),
                ),
                AudioEffectOperation::PlaybackSpeed { rate, link_pitch } => {
                    let samples = clip_samples.ok_or_else(|| {
                        MediaError::InvalidAudioTiming("playback speed requires a clip".to_owned())
                    })?;
                    let processed =
                        operation
                            .transform_duration_samples(samples)
                            .map_err(|error| {
                                MediaError::InvalidAudioTiming(format!(
                                    "invalid playback speed duration: {error:?}"
                                ))
                            })?;
                    clip_samples = Some(processed);
                    if link_pitch {
                        validate_effect_parameters(&AudioEffect::PlaybackSpeed {
                            id: String::new(),
                            rate,
                            link_pitch,
                        })?;
                        let resample_rate = (LINKED_SPEED_CLOCK as f64 * rate).round();
                        // Flush the resampler even when the selected clip is shorter than its filter.
                        self.filter(
                            &input,
                            &format!(
                                "apad=pad_len=128,asetrate={resample_rate:.0},aresample={LINKED_SPEED_CLOCK},asetrate={MIX_SAMPLE_RATE},apad=whole_len={processed},atrim=end_sample={processed},asetpts=N/SR/TB"
                            ),
                        )
                    } else {
                        for stage in decompose_atempo(rate)? {
                            input = self.filter(&input, &format!("atempo={}", number(stage)));
                        }
                        input
                    }
                }
                AudioEffectOperation::RoomReverb { decay_seconds, mix } => {
                    validate_effect_parameters(&AudioEffect::RoomReverb {
                        id: String::new(),
                        decay_seconds,
                        mix,
                    })?;
                    self.apply_wet_effect(input, mix, clip_samples, |graph, wet| {
                        graph.room_reverb(wet, decay_seconds)
                    })?
                }
                AudioEffectOperation::FilteredDelay {
                    delay_ms,
                    repeats,
                    mix,
                    cutoff_hz,
                } => {
                    validate_effect_parameters(&AudioEffect::FilteredDelay {
                        id: String::new(),
                        delay_ms,
                        repeats,
                        mix,
                        cutoff_hz,
                    })?;
                    self.apply_wet_effect(input, mix, clip_samples, |graph, wet| {
                        graph.filtered_delay(wet, delay_ms, repeats, cutoff_hz)
                    })?
                }
                AudioEffectOperation::BeatRepeat {
                    slice_start,
                    slice_duration,
                    output_start,
                    repeats,
                } => {
                    validate_effect_parameters(&AudioEffect::BeatRepeat {
                        id: String::new(),
                        slice_start,
                        slice_duration,
                        output_start,
                        repeats,
                    })?;
                    let (input, _) = self.remix_input(&input, operation, clip_samples)?;
                    self.beat_repeat(
                        input,
                        seconds_to_samples(slice_start)?,
                        seconds_to_samples(slice_duration)?,
                        seconds_to_samples(output_start)?,
                        repeats,
                    )?
                }
                AudioEffectOperation::FilterSweep {
                    mode,
                    ref keyframes,
                } => {
                    let (input, _) = self.remix_input(&input, operation, clip_samples)?;
                    self.filter_sweep(&input, mode, keyframes)?
                }
                AudioEffectOperation::PitchShift { semitones: 0.0 } => input,
                AudioEffectOperation::PitchShift { semitones } => {
                    let (input, samples) = self.remix_input(&input, operation, clip_samples)?;
                    self.pitch_shift(&input, semitones, samples)?
                }
                AudioEffectOperation::TapeStop { start, duration } => {
                    validate_effect_parameters(&AudioEffect::TapeStop {
                        id: String::new(),
                        start,
                        duration,
                    })?;
                    let (input, samples) = self.remix_input(&input, operation, clip_samples)?;
                    self.tape_stop(
                        input,
                        seconds_to_samples(start)?,
                        seconds_to_samples(duration)?,
                        samples,
                    )?
                }
            };
        }
        Ok(input)
    }

    fn remix_input(
        &mut self,
        input: &str,
        operation: &AudioEffectOperation,
        clip_samples: Option<u64>,
    ) -> Result<(String, u64), MediaError> {
        let samples = clip_samples.ok_or_else(|| {
            MediaError::InvalidAudioTiming("remix effects require a clip".to_owned())
        })?;
        if let Some(field) = operation.invalid_clip_timing(samples) {
            return Err(MediaError::InvalidAudioTiming(format!(
                "remix effect {field} exceeds the clip span"
            )));
        }
        // Independent trim-edge rounding and atempo can differ from the planned sample count.
        let stage = self.filter(
            input,
            &format!("apad=whole_len={samples},atrim=end_sample={samples},asetpts=N/SR/TB"),
        );
        Ok((stage, samples))
    }

    fn apply_wet_effect(
        &mut self,
        input: String,
        mix: f64,
        clip_samples: Option<u64>,
        wet_filter: impl FnOnce(&mut Self, &str) -> Result<String, MediaError>,
    ) -> Result<String, MediaError> {
        if mix == 0.0 {
            return Ok(input);
        }
        let (dry, wet_input) = if mix == 1.0 {
            (None, input)
        } else {
            let dry = self.label("dry");
            let wet = self.label("wet");
            self.filters
                .push(format!("[{input}]asplit=2[{dry}][{wet}]"));
            (Some(dry), wet)
        };
        let wet = wet_filter(self, &wet_input)?;
        let output = if let Some(dry) = dry {
            let output = self.label("effect");
            self.filters.push(format!(
                "[{dry}][{wet}]amix=inputs=2:duration=longest:normalize=0:weights='{} {}'[{output}]",
                number(1.0 - mix),
                number(mix),
            ));
            output
        } else {
            wet
        };
        // Later clip effects receive the current clip span, even before a speed change.
        Ok(if let Some(samples) = clip_samples {
            self.filter(
                &output,
                &format!("atrim=end_sample={samples},asetpts=N/SR/TB"),
            )
        } else {
            output
        })
    }

    fn room_reverb(&mut self, input: &str, decay_seconds: f64) -> Result<String, MediaError> {
        const PREDELAY_SAMPLES: u64 = MIX_SAMPLE_RATE * 8 / 1000;
        let decay_samples = seconds_to_samples(decay_seconds)?;
        let response_samples = decay_samples - PREDELAY_SAMPLES;
        let wet = self.filter(input, &format!("apad=pad_len={}", decay_samples - 1));
        let impulse = self.label("room_impulse");
        // A seeded, darkened noise response diffuses reflections and falls 60 dB across the tail.
        self.filters.push(format!(
            "anoisesrc=r={MIX_SAMPLE_RATE}:seed=1234,atrim=end_sample={response_samples},lowpass=f=6500,aeval=exprs='val(0)*pow(0.001,n/{response_samples})',adelay={PREDELAY_SAMPLES}S:all=1[{impulse}]"
        ));
        // The portable FIR default normalizes by the sum of absolute coefficients.
        // Restore roughly unit energy using the exponential envelope's L1/L2 ratio.
        let gain =
            (4.0 * response_samples as f64 / (std::f64::consts::PI * 1000.0_f64.ln())).sqrt();
        let output = self.label("effect");
        self.filters.push(format!(
            "[{wet}][{impulse}]afir=irfmt=mono:minp=64:maxp=4096,volume=volume={}[{output}]",
            number(gain),
        ));
        Ok(output)
    }

    fn filtered_delay(
        &mut self,
        input: &str,
        delay_ms: f64,
        repeats: u8,
        cutoff_hz: f64,
    ) -> Result<String, MediaError> {
        let spacing = seconds_to_samples(delay_ms / 1000.0)?;
        // Feed a few zeroed cycles through the low-pass so a hit at EOF can still ring.
        let padding = seconds_to_samples(4.0 / cutoff_hz)?;
        let wet = self.filter(
            input,
            &format!("apad=pad_len={padding},lowpass=f={}", number(cutoff_hz)),
        );
        let sum = 2.0 * (1.0 - 0.5_f64.powi(i32::from(repeats)));
        let branches = self.split(wet, usize::from(repeats));
        let echoes = branches
            .iter()
            .enumerate()
            .map(|(index, branch)| {
                let delay = spacing * (index as u64 + 1);
                let gain = 0.5_f64.powi(index as i32) / sum;
                self.filter(
                    branch,
                    &format!("adelay={delay}S:all=1,volume=volume={}", number(gain)),
                )
            })
            .collect::<Vec<_>>();
        self.mix(&echoes, "echo").ok_or_else(|| {
            MediaError::InvalidAudioTiming("filtered delay requires at least one repeat".to_owned())
        })
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

fn validate_effect_parameters(effect: &AudioEffect) -> Result<(), MediaError> {
    if let Some(parameter) = effect.invalid_numeric_parameters().next() {
        return Err(MediaError::InvalidAudioTiming(format!(
            "{} parameter {parameter} is outside its declared range",
            effect.definition().id,
        )));
    }
    Ok(())
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
