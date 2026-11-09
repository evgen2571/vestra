use super::*;
use crate::test_support::write_mono_wav;

fn ffmpeg() -> std::path::PathBuf {
    std::env::var_os("VESTRA_TEST_FFMPEG")
        .map(Into::into)
        .unwrap_or_else(|| "ffmpeg".into())
}

fn reference_duration(path: &Path, executable: &Path) -> f64 {
    let output = Command::new(executable)
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-c:a",
            "pcm_f32le",
            "-f",
            "f32le",
            "-",
        ])
        .output()
        .expect("independent reader");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout.len() as f64 / 8.0 / 48_000.0
}

#[test]
fn exact_duration_uses_selected_reader_padding_and_mixer_sample_rate() {
    let directory = tempfile::tempdir().expect("media directory");
    let pcm = directory.path().join("tone.wav");
    write_mono_wav(&pcm, 32_000, 12_000, 0.1);
    let aac = directory.path().join("tone.m4a");
    let encoded = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=48000:duration=1.25",
            "-c:a",
            "aac",
        ])
        .arg(&aac)
        .output()
        .expect("encode AAC");
    assert!(
        encoded.status.success(),
        "{}",
        String::from_utf8_lossy(&encoded.stderr)
    );
    let executable = ffmpeg();
    for path in [pcm, aac] {
        let duration =
            probe_audio_reader_duration_cancellable(&path, &executable, &AtomicBool::new(false))
                .expect("reader duration");
        assert_eq!(duration, reference_duration(&path, &executable));
    }
}

#[test]
fn failed_reader_never_returns_a_duration() {
    let directory = tempfile::tempdir().expect("media directory");
    let path = directory.path().join("broken.m4a");
    std::fs::write(&path, b"not media").expect("invalid fixture");
    assert!(matches!(
        probe_audio_reader_duration_cancellable(&path, &ffmpeg(), &AtomicBool::new(false)),
        Err(MediaError::ProcessFailed { .. })
    ));
}

#[test]
fn cancellation_precedes_starting_the_reader() {
    let error = probe_audio_reader_duration_cancellable(
        Path::new("missing.wav"),
        Path::new("missing-ffmpeg"),
        &AtomicBool::new(true),
    )
    .expect_err("already cancelled");
    assert!(error.to_string().contains("cancelled"), "{error}");
}

#[test]
#[cfg(unix)]
fn cancels_and_reaps_an_active_reader() {
    use std::io::Write;
    let directory = tempfile::tempdir().expect("media directory");
    let path = directory.path().join("large.rf64.wav");
    let data_bytes = 5_u64 * 1024 * 1024 * 1024;
    let mut header = Vec::from(&b"RF64\xff\xff\xff\xffWAVEds64\x1c\0\0\0"[..]);
    header.extend_from_slice(&(data_bytes + 72).to_le_bytes());
    header.extend_from_slice(&data_bytes.to_le_bytes());
    header.extend_from_slice(&(data_bytes / 2).to_le_bytes());
    header.extend_from_slice(&0_u32.to_le_bytes());
    header.extend_from_slice(b"fmt \x10\0\0\0\x01\0\x01\0");
    header.extend_from_slice(&48_000_u32.to_le_bytes());
    header.extend_from_slice(&96_000_u32.to_le_bytes());
    header.extend_from_slice(b"\x02\0\x10\0data\xff\xff\xff\xff");
    let mut file = std::fs::File::create(&path).expect("sparse fixture");
    file.write_all(&header).expect("RF64 header");
    file.set_len(data_bytes + 80).expect("sparse payload");
    drop(file);
    let cancelled = AtomicBool::new(false);
    let executable = ffmpeg();
    let started = Instant::now();
    thread::scope(|scope| {
        let worker =
            scope.spawn(|| probe_audio_reader_duration_cancellable(&path, &executable, &cancelled));
        thread::sleep(Duration::from_millis(25));
        assert!(!worker.is_finished(), "reader still has work to cancel");
        cancelled.store(true, Ordering::Relaxed);
        let error = worker
            .join()
            .expect("probe worker")
            .expect_err("cancelled reader");
        assert!(error.to_string().contains("cancelled"), "{error}");
    });
    assert!(started.elapsed() < Duration::from_secs(5));
}
