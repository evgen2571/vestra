//! Stream independently encoded lossless frames into an animated RIFF container.
//! libwebp_anim retains animation state; libwebp plus this mux keeps memory bounded.

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::{EncoderSettings, MediaError};

pub(crate) fn frame_delay(
    frame: u64,
    rate: (u64, u64),
    ticks: u32,
    maximum: u32,
) -> Result<u32, MediaError> {
    if rate.0 == 0 || rate.1 == 0 || u128::from(rate.0) > u128::from(rate.1) * u128::from(ticks) {
        return Err(MediaError::AnimationEncoding(format!(
            "frame rate must be positive and at most {ticks} FPS"
        )));
    }
    let numerator = u128::from(rate.0);
    let period = u128::from(rate.1) * u128::from(ticks);
    if period.div_ceil(numerator) > u128::from(maximum) {
        return Err(MediaError::AnimationEncoding(
            "frame delay exceeds format limits".to_owned(),
        ));
    }
    // Split whole ticks before multiplication so even u64 frame/rate bounds fit u128.
    let boundary = |index: u128| (index * (period % numerator) + numerator / 2) / numerator;
    let start = boundary(u128::from(frame));
    let end = boundary(u128::from(frame) + 1);
    u32::try_from(period / numerator + end - start)
        .ok()
        .filter(|delay| *delay > 0 && *delay <= maximum)
        .ok_or_else(|| {
            MediaError::AnimationEncoding("frame delay exceeds format limits".to_owned())
        })
}

pub(crate) fn mux(
    mut input: impl Read,
    output: &Path,
    settings: &EncoderSettings,
) -> Result<(), MediaError> {
    mux_inner(&mut input, output, settings).map_err(MediaError::AnimationWrite)
}

fn mux_inner(
    input: &mut impl Read,
    output: &Path,
    settings: &EncoderSettings,
) -> std::io::Result<()> {
    let invalid =
        |message: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, message.to_owned());
    let mut file = File::create(output)?;
    file.write_all(b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\x12\0\0\0")?;
    write_u24(&mut file, settings.width - 1)?;
    write_u24(&mut file, settings.height - 1)?;
    file.write_all(b"ANIM\x06\0\0\0\0\0\0\0\0\0")?;
    let mut written = 44_u64;
    let mut frames = 0_u64;
    loop {
        let mut header = [0_u8; 20];
        if input.read(&mut header[..1])? == 0 {
            break;
        }
        input.read_exact(&mut header[1..])?;
        if &header[..4] != b"RIFF" || &header[8..12] != b"WEBP" || &header[12..16] != b"VP8L" {
            return Err(invalid(
                "FFmpeg libwebp did not produce a lossless VP8L frame",
            ));
        }
        let riff_bytes = u32::from_le_bytes(header[4..8].try_into().unwrap());
        let payload = u32::from_le_bytes(header[16..20].try_into().unwrap());
        let padded = u64::from(payload) + u64::from(payload & 1);
        if u64::from(riff_bytes) != 12 + padded || payload < 5 || frames >= settings.frame_count {
            return Err(invalid("invalid or unexpected lossless WebP frame"));
        }
        let next_size = written + 32 + padded;
        if next_size > u64::from(u32::MAX) - 1 {
            return Err(invalid("animated WebP exceeds the RIFF 4 GiB size limit"));
        }
        // Full-canvas, no-blend frames replace previous pixels, including alpha.
        file.write_all(b"ANMF")?;
        file.write_all(&(24_u32 + payload + (payload & 1)).to_le_bytes())?;
        file.write_all(&[0; 6])?;
        write_u24(&mut file, settings.width - 1)?;
        write_u24(&mut file, settings.height - 1)?;
        let delay = frame_delay(frames, settings.frame_rate, 1000, 0xff_ffff)
            .map_err(|error| invalid(&error.to_string()))?;
        write_u24(&mut file, delay)?;
        file.write_all(&[2])?;
        file.write_all(&header[12..])?;
        let copied = std::io::copy(&mut (&mut *input).take(padded), &mut file)?;
        if copied != padded {
            return Err(invalid("truncated lossless WebP frame"));
        }
        written = next_size;
        frames += 1;
    }
    if frames != settings.frame_count {
        return Err(invalid(
            "FFmpeg returned fewer animation frames than requested",
        ));
    }
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&((written - 8) as u32).to_le_bytes())?;
    file.flush()
}

fn write_u24(output: &mut impl Write, value: u32) -> std::io::Result<()> {
    output.write_all(&value.to_le_bytes()[..3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantized_delays_round_boundaries_without_accumulated_drift() {
        for ticks in [100, 1000] {
            let delays: Vec<_> = (0..30_000)
                .map(|frame| frame_delay(frame, (30_000, 1001), ticks, 0xff_ffff).unwrap())
                .collect();
            assert_eq!(delays.iter().sum::<u32>(), 1001 * ticks);
            assert!(delays.iter().all(|delay| *delay > 0));
        }
        assert!(frame_delay(0, (101, 1), 100, 65535).is_err());
        assert!(frame_delay(0, (0, 1), 100, 65535).is_err());
        assert!(frame_delay(0, (1, 1000), 100, 65535).is_err());
        assert_eq!(
            frame_delay(u64::MAX, (u64::MAX, u64::MAX - 1), 1000, 65535).unwrap(),
            1000
        );
    }

    #[test]
    fn rejects_truncated_frames_and_riff_size_overflow_before_copying_payload() {
        let directory = tempfile::tempdir().unwrap();
        let settings = EncoderSettings {
            width: 1,
            height: 1,
            frame_rate: (1, 1),
            frame_count: 1,
            duration: 1.0,
            audio_range: None,
            quality_crf: 18,
            maximum_audio_sources: 128,
            audio_mix: None,
        };
        for payload in [6, u32::MAX - 13] {
            let mut header = b"RIFF".to_vec();
            header.extend_from_slice(&(payload + 12).to_le_bytes());
            header.extend_from_slice(b"WEBPVP8L");
            header.extend_from_slice(&payload.to_le_bytes());
            let error = mux(
                header.as_slice(),
                &directory.path().join("invalid.webp"),
                &settings,
            )
            .unwrap_err();
            let expected = if payload == 6 { "truncated" } else { "4 GiB" };
            assert!(error.to_string().contains(expected), "{error}");
        }
    }
}
