//! Sample-aligned clip edits built from portable FFmpeg audio filters.

use crate::MediaError;

use super::{GraphBuilder, MIX_SAMPLE_RATE};

const EDIT_FADE_SAMPLES: u64 = MIX_SAMPLE_RATE / 400;
const SAMPLE_CLOCK: &str = "asettb=1/sr,asetpts=N";

impl GraphBuilder {
    pub(super) fn beat_repeat(
        &mut self,
        input: String,
        slice_start: u64,
        slice_samples: u64,
        output_start: u64,
        repeats: u8,
    ) -> Result<String, MediaError> {
        let slice_end = slice_start + slice_samples;
        let output_end = output_start + slice_samples * u64::from(repeats);
        let edge = EDIT_FADE_SAMPLES.min(slice_samples / 4);
        let out_fade = slice_samples - edge - 1;
        let last = output_end - 1;
        let branches = self.split(input, 2);

        // Crossfade into and out of the replacement. Internal cuts keep their
        // exact period by fading each side of the join without overlapping.
        let dry_gain = format!("1-clip(min(n-{output_start},{last}-n)/{edge},0,1)");
        let dry = self.filter(
            &branches[0],
            &format!(
                "aeval=exprs='val(0)*({dry_gain})|val(1)*({dry_gain})':c=stereo,{SAMPLE_CLOCK}"
            ),
        );
        let extra_plays = repeats - 1;
        let repeated = self.filter(
            &branches[1],
            &format!(
                "atrim=start_sample={slice_start}:end_sample={slice_end},{SAMPLE_CLOCK},afade=t=in:ns={edge},afade=t=out:ss={out_fade}:ns={edge},aloop=loop={extra_plays}:size={slice_samples},adelay={output_start}S:all=1,{SAMPLE_CLOCK}"
            ),
        );
        self.mix(&[dry, repeated], "repeat")
            .ok_or_else(|| MediaError::InvalidAudioTiming("missing beat repeat inputs".to_owned()))
    }

    pub(super) fn tape_stop(
        &mut self,
        input: String,
        start: u64,
        duration: u64,
        clip_samples: u64,
    ) -> Result<String, MediaError> {
        let (prefix, input) = if start == 0 {
            (None, input)
        } else {
            let branches = self.split(input, 2);
            (
                Some(self.filter(&branches[0], &format!("atrim=end_sample={start}"))),
                branches[1].clone(),
            )
        };
        let stop_end = start + duration;
        let source = self.filter(
            &input,
            &format!("atrim=start_sample={start}:end_sample={stop_end},{SAMPLE_CLOCK}"),
        );
        // shortcut: approximate continuous speed with short resampling segments,
        // use a variable-rate resampler if the mixer gains native DSP.
        let segments = (duration / (MIX_SAMPLE_RATE / 100)).clamp(16, 64);
        let first = duration / segments / 4;
        let half_overlap = 24.min(first / 4);
        let branches = self.split(source, segments as usize);
        let overlap = 2 * half_overlap;
        let dry_head = self.filter(
            &branches[0],
            &format!(
                "atrim=end_sample={},afade=t=out:ss={}:ns={overlap}",
                first + half_overlap,
                first - half_overlap,
            ),
        );
        // A short unity-rate prefix anchors the slowdown to the original phase.
        let boundary = |index: u64| first + (duration - first) * index / (segments - 1);
        let mut output_segments = vec![dry_head];
        for (index, branch) in branches[1..].iter().enumerate() {
            let index = index as u64;
            let begin = boundary(index);
            let end = boundary(index + 1);
            let head = begin - half_overlap;
            let tail = (end + half_overlap).min(duration);
            let segment = self.tape_segment(branch, duration, begin, end, head, tail);
            let fade_out = if tail == duration {
                String::new()
            } else {
                format!("afade=t=out:ss={}:ns={overlap},", tail - head - overlap)
            };
            output_segments.push(self.filter(
                &segment,
                &format!("afade=t=in:ns={overlap},{fade_out}adelay={head}S:all=1,{SAMPLE_CLOCK}"),
            ));
        }
        let stop = self.mix(&output_segments, "tape_ramp").ok_or_else(|| {
            MediaError::InvalidAudioTiming("missing tape stop segments".to_owned())
        })?;
        let fade = duration / 5;
        let fade_start = duration - fade - 1;
        let stop = self.filter(
            &stop,
            &format!("{SAMPLE_CLOCK},afade=t=out:ss={fade_start}:ns={fade}:curve=hsin"),
        );
        let output = if let Some(prefix) = prefix {
            let output = self.label("tape_stop");
            self.filters
                .push(format!("[{prefix}][{stop}]concat=n=2:v=0:a=1[{output}]"));
            output
        } else {
            stop
        };
        Ok(self.filter(
            &output,
            &format!("apad=whole_len={clip_samples},{SAMPLE_CLOCK}"),
        ))
    }

    fn tape_segment(
        &mut self,
        branch: &str,
        duration: u64,
        begin: u64,
        end: u64,
        head: u64,
        tail: u64,
    ) -> String {
        // Integrating the speed ramp gives the source phase at each join.
        let position = |sample: u64| {
            let sample = sample as f64;
            sample * (1.0 - sample / (2.0 * duration as f64))
        };
        let speed = (position(end) - position(begin)) / (end - begin) as f64;
        let resample_rate = (MIX_SAMPLE_RATE as f64 * speed).round() as u64;
        let speed = resample_rate as f64 / MIX_SAMPLE_RATE as f64;
        let phase = position(begin) - (begin - head) as f64 * speed;
        // Give the sinc filter real source context before and after the cut.
        let source_start = (phase.floor() as u64).saturating_sub(32);
        let skip = ((phase - source_start as f64) / speed).round() as u64;
        let take = skip + tail - head;
        let source_end = source_start + (take as f64 * speed).ceil() as u64 + 32;
        self.filter(
            branch,
            &format!(
                "atrim=start_sample={source_start}:end_sample={source_end},asetrate={resample_rate},aresample={MIX_SAMPLE_RATE},atrim=start_sample={skip}:end_sample={take},{SAMPLE_CLOCK}"
            ),
        )
    }
}

#[cfg(test)]
#[path = "remix_edits_tests.rs"]
mod tests;
