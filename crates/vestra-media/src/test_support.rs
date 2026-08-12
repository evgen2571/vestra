use std::path::Path;

pub(crate) fn write_mono_wav(path: &Path, sample_rate: u32, samples: usize, amplitude: f32) {
    write_mono_wav_samples(path, sample_rate, &vec![amplitude; samples]);
}

pub(crate) fn write_mono_wav_samples(path: &Path, sample_rate: u32, samples: &[f32]) {
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
        bytes.extend_from_slice(&((sample * i16::MAX as f32).round() as i16).to_le_bytes());
    }
    std::fs::write(path, bytes).expect("fixture WAV");
}
