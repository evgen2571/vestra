use std::{
    io::Read,
    process::{ChildStderr, Command, Stdio},
    thread,
};

use vestra_core::plan_audio::{AudioMixPlan, MASTER_AUDIO_SAMPLE_RATE};

use crate::{
    MediaError,
    audio_graph::{self, AudioGraphCompileOptions},
    ffmpeg::{
        FILTERGRAPH_SCRIPT_THRESHOLD_BYTES, TemporaryFiltergraph, enforce_audio_source_limit,
    },
};

use super::CHANNELS;

#[cfg(test)]
use super::record_master_decode_invocation;

const PCM_READ_BYTES: usize = 32 * 1024;
const STDERR_LIMIT_BYTES: usize = 64 * 1024;

/// Fixed format of the timeline Master stream supplied to analysis processors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MasterPcmSpec {
    pub sample_rate: u32,
    pub channels: u16,
}

impl MasterPcmSpec {
    #[must_use]
    pub const fn timeline_master() -> Self {
        Self {
            sample_rate: MASTER_AUDIO_SAMPLE_RATE,
            channels: CHANNELS,
        }
    }
}

/// Stream the production Master waveform in interleaved stereo `f32le` chunks.
///
/// The callback can retain only its own incremental state, so future feature
/// processors can share one FFmpeg execution without retaining project PCM.
pub fn consume_master_pcm(
    mix: &AudioMixPlan,
    project_duration: f64,
    maximum_audio_sources: usize,
    mut consume: impl FnMut(&[f32]) -> Result<(), MediaError>,
) -> Result<MasterPcmSpec, MediaError> {
    let spec = MasterPcmSpec::timeline_master();
    let (expected_frames, graph) =
        master_pcm_execution(mix, project_duration, maximum_audio_sources)?;
    let Some(graph) = graph else {
        consume_silence(expected_frames, &mut consume)?;
        return Ok(spec);
    };

    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error"]);
    for path in &graph.input_paths {
        command.arg("-i").arg(path);
    }
    let filtergraph_file = (graph.filter_complex.len() > FILTERGRAPH_SCRIPT_THRESHOLD_BYTES)
        .then(|| TemporaryFiltergraph::create_in(&std::env::temp_dir(), &graph.filter_complex))
        .transpose()?;
    if let Some(file) = &filtergraph_file {
        command.arg("-/filter_complex").arg(&file.path);
    } else {
        command.arg("-filter_complex").arg(&graph.filter_complex);
    }
    command
        .args(["-map", "[audio]"])
        .args(["-f", "f32le", "-acodec", "pcm_f32le", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|source| MediaError::ProcessStart {
        program: "ffmpeg",
        source,
    })?;
    #[cfg(test)]
    record_master_decode_invocation();
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => return reap_failed_child(child, MediaError::MissingErrorOutput),
    };
    let stderr_reader = thread::spawn(move || collect_bounded_stderr(stderr));
    let mut stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stderr_reader.join();
            return Err(MediaError::MissingMasterPcmOutput);
        }
    };

    let result = consume_pcm_reader(&mut stdout, &mut consume, expected_frames);
    let consumer_failed = matches!(result, Err(MediaError::MasterPcmConsumer { .. }));
    if consumer_failed {
        let _ = child.kill();
    }
    drop(stdout);
    // Always reap and drain stderr before returning the primary stream error.
    // In particular, `wait` failures must not skip the stderr thread join.
    let status = child.wait();
    if status.is_err() {
        // A failed wait is unusual, but still make a best-effort second cleanup
        // attempt before joining the stderr drainer and reporting it.
        let _ = child.kill();
        let _ = child.wait();
    }
    let stderr = stderr_reader
        .join()
        .map_err(|_| MediaError::StderrCollection {
            operation: "reading Master PCM",
        })?;
    if consumer_failed {
        return result.map(|()| spec);
    }
    let status = status.map_err(MediaError::ProcessWait)?;
    if !status.success() {
        return Err(MediaError::ProcessFailed {
            program: "ffmpeg",
            status,
            stderr,
        });
    }
    result?;
    Ok(spec)
}

fn master_pcm_execution(
    mix: &AudioMixPlan,
    project_duration: f64,
    maximum_audio_sources: usize,
) -> Result<(u64, Option<audio_graph::FfmpegAudioGraph>), MediaError> {
    let expected_frames = audio_graph::seconds_to_samples(project_duration)?;
    if !mix.has_authored_material() {
        return Err(MediaError::MasterAudioUnavailable);
    }
    if mix.audible_clip_count() == 0 {
        return Ok((expected_frames, None));
    }
    let graph = audio_graph::compile_with_options(
        mix,
        project_duration,
        AudioGraphCompileOptions {
            first_audio_input_index: 0,
        },
    )?;
    enforce_audio_source_limit(&graph, maximum_audio_sources)?;
    Ok((expected_frames, Some(graph)))
}

fn reap_failed_child(
    mut child: std::process::Child,
    error: MediaError,
) -> Result<MasterPcmSpec, MediaError> {
    let _ = child.kill();
    let _ = child.wait();
    Err(error)
}

fn consume_silence(
    mut remaining_frames: u64,
    consume: &mut impl FnMut(&[f32]) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    let zeros = vec![0.0_f32; 4_096 * CHANNELS as usize];
    while remaining_frames > 0 {
        let frames = remaining_frames.min(4_096) as usize;
        consume(&zeros[..frames * CHANNELS as usize]).map_err(MediaError::master_pcm_consumer)?;
        remaining_frames -= frames as u64;
    }
    Ok(())
}

fn consume_pcm_reader(
    reader: &mut impl Read,
    consume: &mut impl FnMut(&[f32]) -> Result<(), MediaError>,
    expected_frames: u64,
) -> Result<(), MediaError> {
    let mut decoder = PcmDecoder::default();
    let mut bytes = [0_u8; PCM_READ_BYTES];
    let mut emitted_frames = 0_u64;
    loop {
        let read = reader.read(&mut bytes).map_err(|error| {
            MediaError::MalformedMasterPcm(format!("cannot read FFmpeg stdout: {error}"))
        })?;
        if read == 0 {
            break;
        }
        decoder.push(&bytes[..read], |samples| {
            emitted_frames += (samples.len() / CHANNELS as usize) as u64;
            consume(samples).map_err(MediaError::master_pcm_consumer)
        })?;
    }
    decoder.finish()?;
    if emitted_frames != expected_frames {
        return Err(MediaError::MalformedMasterPcm(format!(
            "FFmpeg emitted {emitted_frames} stereo frames; expected {expected_frames}"
        )));
    }
    Ok(())
}

#[derive(Default)]
struct PcmDecoder {
    carry: Vec<u8>,
    samples: Vec<f32>,
}

impl PcmDecoder {
    fn push(
        &mut self,
        bytes: &[u8],
        consume: impl FnOnce(&[f32]) -> Result<(), MediaError>,
    ) -> Result<(), MediaError> {
        self.carry.extend_from_slice(bytes);
        let complete_bytes = self.carry.len() / (4 * CHANNELS as usize) * (4 * CHANNELS as usize);
        if complete_bytes == 0 {
            return Ok(());
        }
        self.samples.clear();
        self.samples.reserve(complete_bytes / 4);
        for bytes in self.carry[..complete_bytes].chunks_exact(4) {
            let sample = f32::from_le_bytes(bytes.try_into().expect("four-byte PCM sample"));
            if !sample.is_finite() {
                return Err(MediaError::MalformedMasterPcm(
                    "FFmpeg emitted a non-finite sample".to_owned(),
                ));
            }
            self.samples.push(sample);
        }
        self.carry.drain(..complete_bytes);
        consume(&self.samples)
    }

    fn finish(self) -> Result<(), MediaError> {
        if self.carry.is_empty() {
            Ok(())
        } else if self.carry.len() < 4 {
            Err(MediaError::MalformedMasterPcm(
                "FFmpeg ended with an incomplete float sample".to_owned(),
            ))
        } else {
            Err(MediaError::MalformedMasterPcm(
                "FFmpeg ended with an incomplete stereo frame".to_owned(),
            ))
        }
    }
}

fn collect_bounded_stderr(stderr: ChildStderr) -> String {
    let mut reader = std::io::BufReader::new(stderr);
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4_096];
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if bytes.len() > STDERR_LIMIT_BYTES {
            let excess = bytes.len() - STDERR_LIMIT_BYTES;
            bytes.drain(..excess);
        }
    }
    String::from_utf8_lossy(&bytes).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoder_reconstructs_unaligned_pcm_and_preserves_amplitude() {
        let values = [1.5_f32, -2.0, 0.25, 0.75];
        let bytes = values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>();
        let mut decoder = PcmDecoder::default();
        let mut output = Vec::new();
        let mut offset = 0;
        for chunk in [3, 5, 7, 1] {
            let before = offset;
            offset += chunk;
            decoder
                .push(&bytes[before..offset], |samples| {
                    output.extend_from_slice(samples);
                    Ok(())
                })
                .expect("decode");
        }
        decoder.finish().expect("complete PCM");
        assert_eq!(output, values);
    }

    #[test]
    fn decoder_rejects_partial_and_non_finite_pcm() {
        let mut decoder = PcmDecoder::default();
        decoder.push(&[1, 2, 3], |_| Ok(())).expect("carry bytes");
        assert!(matches!(
            decoder.finish(),
            Err(MediaError::MalformedMasterPcm(_))
        ));
        let mut decoder = PcmDecoder::default();
        let non_finite = [f32::NAN.to_le_bytes(), 0.0_f32.to_le_bytes()].concat();
        assert!(matches!(
            decoder.push(&non_finite, |_| Ok(())),
            Err(MediaError::MalformedMasterPcm(_))
        ));
    }
}
