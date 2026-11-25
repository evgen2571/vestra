use std::{
    fmt::Display,
    fs,
    ops::Range,
    path::{Path, PathBuf},
};

use ffmpeg_next as ffmpeg;
use uuid::Uuid;
use vestra_core::timeline::seconds_to_nanos;

use crate::{MediaError, effective_parent, init_ffmpeg, output::OutputFormat};

/// Plain-text MP4 subtitle cue on the original project timeline, in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct SubtitleCue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug)]
pub struct SubtitleTrack<'a> {
    samples: Vec<SubtitleSample<'a>>,
}

#[derive(Debug)]
struct SubtitleSample<'a> {
    start_ms: i64,
    end_ms: i64,
    text: &'a str,
}

impl<'a> SubtitleTrack<'a> {
    /// Validate before starting an encoder, then clip and rebase onto the output clock.
    pub fn prepare(
        cues: &'a [SubtitleCue],
        output: &Path,
        window: Range<u128>,
    ) -> Result<Option<Self>, MediaError> {
        if cues.is_empty() {
            return Ok(None);
        }
        if OutputFormat::from_path(output) != OutputFormat::Mp4 {
            return Err(MediaError::SubtitleEncoding(
                "selectable subtitles require MP4 output".to_owned(),
            ));
        }

        let mut timed = Vec::with_capacity(cues.len());
        for (index, cue) in cues.iter().enumerate() {
            let start = seconds_to_nanos(cue.start);
            let end = seconds_to_nanos(cue.end);
            let (Some(start), Some(end)) = (start, end) else {
                return Err(invalid_cue(
                    index,
                    "times must be finite, non-negative and within the supported timeline",
                ));
            };
            if start >= end {
                return Err(invalid_cue(index, "end must be greater than start"));
            }
            if cue.text.trim().is_empty() || cue.text.contains('\0') {
                return Err(invalid_cue(
                    index,
                    "text must be nonblank and contain no NUL characters",
                ));
            }
            if u16::try_from(cue.text.len()).is_err() {
                return Err(invalid_cue(index, "text exceeds 65535 UTF-8 bytes"));
            }
            timed.push((start, end, index, cue.text.as_str()));
        }
        timed.sort_unstable_by_key(|&(start, _, index, _)| (start, index));
        let mut samples = Vec::with_capacity(timed.len());
        let mut previous_end = 0;
        for (start, end, index, text) in timed {
            if start < previous_end {
                return Err(invalid_cue(
                    index,
                    "cues on a selectable track must not overlap",
                ));
            }
            previous_end = end;
            let start = start.max(window.start);
            let end = end.min(window.end);
            if start >= end {
                continue;
            }
            let start_ms = output_milliseconds(start - window.start)
                .ok_or_else(|| invalid_cue(index, "start exceeds the track clock"))?;
            let end_ms = output_milliseconds(end - window.start)
                .ok_or_else(|| invalid_cue(index, "end exceeds the track clock"))?;
            if start_ms < end_ms {
                samples.push(SubtitleSample {
                    start_ms,
                    end_ms,
                    text,
                });
            }
        }
        Ok((!samples.is_empty()).then_some(Self { samples }))
    }

    pub(crate) fn write_temporary(
        &self,
        output: &Path,
    ) -> Result<TemporarySubtitleTrack, MediaError> {
        let track = TemporarySubtitleTrack {
            path: effective_parent(output)
                .join(format!(".vestra-{}.subtitles.mp4", Uuid::new_v4())),
        };
        track.write(&self.samples)?;
        Ok(track)
    }
}

fn output_milliseconds(nanos: u128) -> Option<i64> {
    i64::try_from((nanos / 1_000_000) + u128::from(nanos % 1_000_000 >= 500_000)).ok()
}

fn invalid_cue(index: usize, message: &str) -> MediaError {
    MediaError::SubtitleEncoding(format!("cue {}: {message}", index + 1))
}

pub(crate) struct TemporarySubtitleTrack {
    pub(crate) path: PathBuf,
}

impl TemporarySubtitleTrack {
    fn write(&self, samples: &[SubtitleSample<'_>]) -> Result<(), MediaError> {
        let path = self
            .path
            .to_str()
            .filter(|path| !path.contains('\0'))
            .ok_or_else(|| {
                track_error("subtitle staging path must be valid UTF-8 without NUL characters")
            })?;
        init_ffmpeg()?;
        // The safe FFmpeg bindings obtain codec extradata through demuxing.
        // Read a tx3g sample description before replacing the staging file with packets.
        fs::write(path, tx3g_description()).map_err(track_error)?;
        let parameters = {
            let input = ffmpeg::format::input(path).map_err(track_error)?;
            input
                .stream(0)
                .ok_or_else(|| track_error("timed-text sample description has no stream"))?
                .parameters()
                .clone()
        };
        let mut output = ffmpeg::format::output_as(path, "mp4").map_err(track_error)?;
        let mut stream = output.add_stream(None).map_err(track_error)?;
        stream.set_parameters(parameters);
        stream.set_time_base((1, 1000));
        output.write_header().map_err(track_error)?;
        for sample in samples {
            // A tx3g sample is a big-endian byte count followed by literal UTF-8.
            let byte_count = u16::try_from(sample.text.len()).map_err(track_error)?;
            let mut bytes = Vec::with_capacity(2 + usize::from(byte_count));
            bytes.extend_from_slice(&byte_count.to_be_bytes());
            bytes.extend_from_slice(sample.text.as_bytes());
            let mut packet = ffmpeg::Packet::copy(&bytes);
            packet.set_stream(0);
            packet.set_pts(Some(sample.start_ms));
            packet.set_dts(Some(sample.start_ms));
            packet.set_duration(sample.end_ms - sample.start_ms);
            packet.write_interleaved(&mut output).map_err(track_error)?;
        }
        output.write_trailer().map_err(track_error)
    }
}

impl Drop for TemporarySubtitleTrack {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn tx3g_description() -> Vec<u8> {
    fn atom(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let size = u32::try_from(8 + body.len()).expect("fixed sample description size");
        [size.to_be_bytes().as_slice(), kind, body].concat()
    }

    let fonts = atom(b"ftab", b"\0\x01\0\x01\x05Serif");
    let entry = [
        b"\0\0\0\0\0\0\0\x01".as_slice(), // reserved and data reference
        b"\0\0\0\0\x01\xff",              // display flags and center/bottom justification
        &[0; 12],                         // transparent background and default text box
        b"\0\0\0\0\0\x01\0\x12\xff\xff\xff\xff", // white text, font 1 at size 18
        &fonts,
    ]
    .concat();
    let description = atom(
        b"stsd",
        &[b"\0\0\0\0\0\0\0\x01", atom(b"tx3g", &entry).as_slice()].concat(),
    );
    let table = atom(b"minf", &atom(b"stbl", &description));
    let handler = atom(
        b"hdlr",
        &[b"\0\0\0\0\0\0\0\0sbtl".as_slice(), &[0; 13]].concat(),
    );
    let media_header = atom(
        b"mdhd",
        &[
            [0; 12].as_slice(), // version, flags and creation/modification times
            &1000_u32.to_be_bytes(),
            b"\0\0\0\0\x55\xc4\0\0", // zero duration and undetermined language
        ]
        .concat(),
    );
    atom(
        b"moov",
        &atom(
            b"trak",
            &atom(b"mdia", &[media_header, handler, table].concat()),
        ),
    )
}

fn track_error(error: impl Display) -> MediaError {
    MediaError::SubtitleEncoding(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_track_writes_literal_text_and_timing() {
        let cues = [
            SubtitleCue {
                start: 0.1,
                end: 0.6,
                text: "  \n<i>literal</i> \\N 🥪".to_owned(),
            },
            SubtitleCue {
                start: 0.8,
                end: 1.1,
                text: "é".to_owned(),
            },
        ];
        let directory = tempfile::tempdir().expect("temp dir");
        let output = directory.path().join("output.mp4");
        let track = SubtitleTrack::prepare(&cues, &output, 300_000_000..900_000_000)
            .expect("valid cues")
            .expect("clipped cues");
        let temporary = track.write_temporary(&output).expect("subtitle MP4");
        let mut input = ffmpeg::format::input(&temporary.path).expect("open subtitle MP4");
        let times_and_text = input
            .packets()
            .filter_map(|(_, packet)| {
                let text = packet.data()?.get(2..)?;
                if text.is_empty() {
                    return None;
                }
                Some((
                    packet.pts().expect("pts"),
                    packet.duration(),
                    String::from_utf8(text.to_vec()).expect("text"),
                ))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            times_and_text,
            [
                (0, 300, cues[0].text.clone()),
                (500, 100, cues[1].text.clone())
            ]
        );
        let staged = temporary.path.clone();
        drop(input);
        drop(temporary);
        assert!(!staged.exists());
    }
}
