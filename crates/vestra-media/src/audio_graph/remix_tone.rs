use std::f64::consts::FRAC_1_SQRT_2;

use vestra_core::project::{AudioEffect, AudioFilterKeyframe, AudioFilterMode};

use super::{
    GraphBuilder, LINKED_SPEED_CLOCK, MIX_SAMPLE_RATE, MediaError, number, seconds_to_samples,
    validate_effect_parameters,
};

const FILTER_BLOCK_SAMPLES: u64 = 120;

impl GraphBuilder {
    pub(super) fn filter_sweep(
        &mut self,
        input: &str,
        mode: AudioFilterMode,
        keyframes: &[AudioFilterKeyframe],
    ) -> Result<String, MediaError> {
        let kind = match mode {
            AudioFilterMode::LowPass => "lowpass",
            AudioFilterMode::HighPass => "highpass",
            AudioFilterMode::BandPass => "bandpass",
        };
        if !(1..=AudioFilterKeyframe::MAX_COUNT).contains(&keyframes.len()) {
            return Err(MediaError::InvalidAudioTiming(
                "filter sweep keyframe count is outside its declared range".to_owned(),
            ));
        }
        let first = &keyframes[0];
        let frequencies =
            AudioFilterKeyframe::MIN_FREQUENCY_HZ..=AudioFilterKeyframe::MAX_FREQUENCY_HZ;
        if first.time != 0.0
            || keyframes
                .iter()
                .any(|key| !frequencies.contains(&key.frequency_hz))
        {
            return Err(MediaError::InvalidAudioTiming(
                "filter sweep keyframes must start at zero and use frequencies within the declared range"
                    .to_owned(),
            ));
        }
        let target = format!("{kind}@{}", self.label("sweep"));
        // State variable filters keep their history as the cutoff coefficients change.
        let filter = format!(
            "{target}=f={}:width_type=q:w={}:transform=svf",
            number(first.frequency_hz),
            number(FRAC_1_SQRT_2),
        );
        if keyframes.len() == 1 {
            return Ok(self.filter(input, &filter));
        }

        let mut commands = String::new();
        for pair in keyframes.windows(2) {
            let start = seconds_to_samples(pair[0].time)?;
            let end = seconds_to_samples(pair[1].time)?;
            if start >= end {
                return Err(MediaError::InvalidAudioTiming(
                    "filter sweep times must strictly increase at 48 kHz".to_owned(),
                ));
            }
            let start_hz = pair[0].frequency_hz;
            let delta_hz = pair[1].frequency_hz - start_hz;
            let start = start as f64 / MIX_SAMPLE_RATE as f64;
            let end = end as f64 / MIX_SAMPLE_RATE as f64;
            commands.push_str(&format!(
                "{start:.6}-{end:.6} [expr] {target} f {}+TI*({});",
                number(start_hz),
                number(delta_hz),
            ));
        }
        let last = &keyframes[keyframes.len() - 1];
        let end = seconds_to_samples(last.time)? as f64 / MIX_SAMPLE_RATE as f64;
        commands.push_str(&format!(
            "{end:.6} [enter] {target} f {}",
            number(last.frequency_hz),
        ));

        // FFmpeg updates cutoff per audio frame. Reblock onto the current clip clock.
        Ok(self.filter(
            input,
            &format!(
                "asetnsamples=n={FILTER_BLOCK_SAMPLES}:p=0,asetpts=N/SR/TB,asendcmd=c='{commands}',{filter}"
            ),
        ))
    }

    pub(super) fn pitch_shift(
        &mut self,
        input: &str,
        semitones: f64,
        clip_samples: u64,
    ) -> Result<String, MediaError> {
        validate_effect_parameters(&AudioEffect::PitchShift {
            id: String::new(),
            semitones,
        })?;
        if semitones == 0.0 {
            return Ok(input.to_owned());
        }

        let pitch_ratio = 2.0_f64.powf(semitones / 12.0);
        let resample_rate = (LINKED_SPEED_CLOCK as f64 * pitch_ratio).round();
        let tempo = number(pitch_ratio.recip());
        // shortcut: atempo can shift individual attacks locally; use transient-aware stretching
        // if the effect needs sample-accurate attack timing.
        // Pad both filters to flush their windows, including on sub-window clips.
        Ok(self.filter(
            input,
            &format!(
                "apad=pad_len=128,asetrate={resample_rate:.0},aresample={LINKED_SPEED_CLOCK},asetrate={MIX_SAMPLE_RATE},asetpts=N/SR/TB,apad=pad_len=4096,atempo={tempo},apad=whole_len={clip_samples},atrim=end_sample={clip_samples},asetpts=N/SR/TB"
            ),
        ))
    }
}
