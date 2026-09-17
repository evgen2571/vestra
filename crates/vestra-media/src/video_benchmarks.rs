//! Manual native-decoder measurements with an independent pixel oracle.

use super::{VideoDecoder, VideoDecoderOptions};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

#[derive(serde::Serialize)]
struct Measurement {
    pattern: &'static str,
    cache_budget_bytes: u64,
    sample: usize,
    warmup: bool,
    open_us: u128,
    frame_requests_us: u128,
    frame_requests: u64,
    actual_decodes: u64,
    seeks: u64,
    cache_hits: u64,
    cache_misses: u64,
}

/// Run an optimized test binary in isolation. Fixture generation precedes all
/// passes; pixel comparisons run only in each pair's discarded warmup. Process
/// RSS includes the 180-frame reference buffer, not just decoder memory.
#[test]
#[ignore = "manual native video access benchmark; requires release mode and FFmpeg"]
fn video_access_benchmark_matrix() -> Result<(), &'static str> {
    if cfg!(debug_assertions) {
        return Err("run this benchmark in release mode");
    }
    let output = PathBuf::from(
        std::env::var_os("VESTRA_VIDEO_ACCESS_BENCH_OUTPUT")
            .expect("set a fresh VESTRA_VIDEO_ACCESS_BENCH_OUTPUT JSON path"),
    );
    let directory = output.parent().expect("output directory");
    fs::create_dir_all(directory).expect("create output directory");
    assert!(!output.exists(), "benchmark output must be new");
    let fixture = directory.join("long-gop.mkv");
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-n",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=30:duration=6",
            "-frames:v",
            "180",
            "-c:v",
            "libx264",
            "-g",
            "60",
            "-bf",
            "3",
            "-sc_threshold",
            "0",
            "-pix_fmt",
            "yuv420p",
            "-fflags",
            "+bitexact",
            "-flags",
            "+bitexact",
        ])
        .arg(&fixture)
        .status()
        .expect("generate long-GOP fixture");
    assert!(status.success());
    let reference = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(&fixture)
        .args([
            "-sws_flags",
            "bilinear",
            "-pix_fmt",
            "rgba",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .expect("decode reference with FFmpeg");
    assert!(reference.status.success());
    let frame_bytes = 1280 * 720 * 4;
    assert_eq!(reference.stdout.len(), 180 * frame_bytes);
    let patterns = [
        ("sequential", (0..180).collect::<Vec<usize>>()),
        ("holds", (0..180).map(|index| index / 3).collect()),
        (
            "sparse_forward",
            vec![0, 30, 59, 60, 90, 119, 120, 150, 179],
        ),
        (
            "scrub",
            [0, 90, 45, 120, 59, 60, 61, 150, 30, 179, 119, 121].repeat(4),
        ),
    ];
    let mut measurements = Vec::new();
    for budget in [
        0,
        frame_bytes as u64,
        frame_bytes as u64 * 4,
        64 * 1024 * 1024,
    ] {
        for (pattern, indices) in &patterns {
            for sample in 0..4 {
                let started = Instant::now();
                let mut decoder = VideoDecoder::open_with_options(
                    &fixture,
                    VideoDecoderOptions {
                        cache_budget_bytes: budget,
                        ..VideoDecoderOptions::default()
                    },
                )
                .expect("open benchmark decoder");
                let open_us = started.elapsed().as_micros();
                let mut frame_requests_time = Duration::ZERO;
                for (request, &index) in indices.iter().enumerate() {
                    let phase = if *pattern == "holds" {
                        (request % 3) as f64 / 3.0 + 1.0 / 12.0
                    } else {
                        0.25
                    };
                    let started = Instant::now();
                    let frame = decoder
                        .frame_at((index as f64 + phase) / 30.0)
                        .expect("request benchmark frame");
                    frame_requests_time += started.elapsed();
                    if sample == 0 {
                        assert_eq!(
                            frame.pixels.as_raw(),
                            &reference.stdout[index * frame_bytes..(index + 1) * frame_bytes],
                            "pattern={pattern}, budget={budget}, frame={index}"
                        );
                    }
                    std::hint::black_box(frame);
                }
                let metrics = decoder.metrics();
                measurements.push(Measurement {
                    pattern,
                    cache_budget_bytes: budget,
                    sample,
                    warmup: sample == 0,
                    open_us,
                    frame_requests_us: frame_requests_time.as_micros(),
                    frame_requests: metrics.frame_requests,
                    actual_decodes: metrics.actual_decodes,
                    seeks: metrics.seeks,
                    cache_hits: metrics.cache_hits,
                    cache_misses: metrics.cache_misses,
                });
            }
        }
    }
    let report = serde_json::json!({
        "width": 1280, "height": 720, "frame_rate": "30/1", "frames": 180,
        "codec": "h264", "pixel_format": "yuv420p", "gop": 60, "b_frames": 3,
        "oracle_bytes": reference.stdout.len(), "measurements": measurements,
    });
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .expect("create new benchmark report");
    serde_json::to_writer_pretty(file, &report).expect("write benchmark report");
    Ok(())
}
