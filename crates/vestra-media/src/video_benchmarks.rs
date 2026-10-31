//! Manual native-decoder measurements with independent presentation/pixel oracles.

use super::{VideoDecoder, VideoDecoderOptions};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

#[derive(serde::Serialize)]
struct Measurement {
    fixture: &'static str,
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
    decode_time_us: u64,
    cache_bytes_peak: u64,
    owned_and_visible_buffer_bytes_peak: u64,
}

/// Release mode, one discarded oracle/warmup pass and three timing passes.
/// References stay on disk; process RSS does not include a whole decoded movie.
/// Baseline/candidate reports in the same directory reuse identical media.
/// `PATTERN` selects a focused repeat; `MODE=timing` skips oracle I/O after an
/// oracle run has validated the same inputs, without skipping decoder warmup.
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
    let directory = output.parent().expect("output directory").join("media");
    fs::create_dir_all(&directory).expect("create fixture directory");
    assert!(!output.exists(), "benchmark output must be new");
    let filter = std::env::var("VESTRA_VIDEO_ACCESS_BENCH_FIXTURE").ok();
    let pattern_filter = std::env::var("VESTRA_VIDEO_ACCESS_BENCH_PATTERN").ok();
    let mode = std::env::var("VESTRA_VIDEO_ACCESS_BENCH_MODE").unwrap_or_else(|_| "oracle".into());
    assert!(matches!(mode.as_str(), "oracle" | "timing"));
    let mut measurements = Vec::new();
    let mut fixtures = Vec::new();
    for (name, width, height, duration, gop, vfr) in [
        ("typical-1080p", 1920, 1080, 30, 60, false),
        ("long-gop", 640, 360, 60, 300, false),
        ("vfr", 640, 360, 30, 60, true),
    ] {
        if filter.as_deref().is_some_and(|filter| filter != name) {
            continue;
        }
        let path = directory.join(format!("{name}.mkv"));
        if !path.exists() {
            let mut command = Command::new("ffmpeg");
            command.args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-n",
                "-f",
                "lavfi",
                "-i",
            ]);
            command.arg(format!(
                "testsrc2=size={width}x{height}:rate=30:duration={duration}"
            ));
            if vfr {
                command.args(["-vf", "select=not(eq(mod(n\\,3)\\,1))", "-fps_mode", "vfr"]);
            }
            let status = command
                .args([
                    "-c:v", "libx264", "-preset", "veryfast", "-threads", "2", "-g",
                ])
                .arg(gop.to_string())
                .args(["-keyint_min"])
                .arg(gop.to_string())
                .args([
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
                .arg(&path)
                .status()
                .expect("generate benchmark fixture");
            assert!(status.success());
        }
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_frames",
                "-show_streams",
                "-show_entries",
                "frame=best_effort_timestamp,pict_type,key_frame:stream=time_base",
                "-of",
                "json",
            ])
            .arg(&path)
            .output()
            .expect("independent presentation timestamp oracle");
        assert!(probe.status.success());
        let probe: serde_json::Value = serde_json::from_slice(&probe.stdout).expect("probe JSON");
        let pts: Vec<i64> = probe["frames"]
            .as_array()
            .expect("frames")
            .iter()
            .map(|frame| frame["best_effort_timestamp"].as_i64().expect("frame PTS"))
            .collect();
        assert!(pts.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(
            probe["frames"]
                .as_array()
                .expect("frames")
                .iter()
                .any(|frame| frame["pict_type"] == "B")
        );
        if vfr {
            assert!(
                pts.windows(3)
                    .any(|triple| triple[1] - triple[0] != triple[2] - triple[1])
            );
        }
        let reference = directory.join(format!("{name}.rgba"));
        if !reference.exists() {
            let status = Command::new("ffmpeg")
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-n",
                    "-threads",
                    "2",
                    "-i",
                ])
                .arg(&path)
                .args([
                    "-sws_flags",
                    "bilinear",
                    "-fps_mode",
                    "passthrough",
                    "-pix_fmt",
                    "rgba",
                    "-f",
                    "rawvideo",
                ])
                .arg(&reference)
                .status()
                .expect("external full sequential RGBA oracle");
            assert!(status.success());
        }
        let frame_bytes = width as usize * height as usize * 4;
        assert_eq!(
            fs::metadata(&reference).expect("oracle metadata").len(),
            (pts.len() * frame_bytes) as u64
        );
        let mut oracle_file = fs::File::open(&reference).expect("oracle file");
        let mut oracle_pixels = vec![0; frame_bytes];
        let info = VideoDecoder::open(&path)
            .expect("probe native decoder")
            .info()
            .clone();
        assert_eq!(
            probe["streams"][0]["time_base"],
            format!(
                "{}/{}",
                info.time_base.numerator, info.time_base.denominator
            )
        );
        let times: Vec<f64> = pts
            .iter()
            .map(|&pts| info.time_base.ticks_to_seconds(pts - info.source_origin.0) + 0.001)
            .collect();
        let last = duration as f64 - 0.01;
        let patterns = [
            ("sequential", times.clone()),
            (
                "short_forward",
                (0..8).map(|index| index as f64 * 3.9 + 0.001).collect(),
            ),
            (
                "four_second_jumps",
                (0..8)
                    .map(|index| index as f64 * 4.1 + 0.001)
                    .filter(|&seconds| seconds < last)
                    .collect(),
            ),
            ("large_forward", vec![0.001, 10.001, 20.001, last]),
            ("backward", vec![last, last - 10.0, last - 20.0, 0.001]),
            (
                "alternating",
                vec![0.001, last, 1.001, last - 1.0, 2.001, last - 2.0],
            ),
            (
                "scrub",
                [
                    0.001, 0.73, 0.21, 0.91, 0.39, 0.62, 0.11, 0.88, 0.51, 0.03, 0.97, 0.27,
                ]
                .iter()
                .map(|fraction| fraction * last)
                .collect(),
            ),
            ("holds", vec![10.001; 32]),
            ("stream_edges", vec![0.0, 0.001, last, last, 0.0, last]),
            ("initial_below_24_frames", vec![0.79]),
            ("initial_above_24_frames", vec![0.81]),
            ("initial_below_one_second", vec![0.99]),
            ("initial_above_one_second", vec![1.01]),
            ("forward_below_four_seconds", vec![0.001, 3.99]),
            ("forward_above_four_seconds", vec![0.001, 4.01]),
            (
                "eight_second_jumps",
                (0..8)
                    .map(|index| index as f64 * 8.1 + 0.001)
                    .filter(|&seconds| seconds < last)
                    .collect(),
            ),
            ("forward_below_eight_seconds", vec![0.001, 7.99]),
            ("forward_above_eight_seconds", vec![0.001, 8.01]),
        ];
        for budget in [0, 64 * 1024 * 1024] {
            for (pattern, requests) in &patterns {
                if pattern_filter
                    .as_deref()
                    .is_some_and(|filter| !filter.split(',').any(|selected| selected == *pattern))
                {
                    continue;
                }
                for sample in 0..4 {
                    let started = Instant::now();
                    let mut decoder = VideoDecoder::open_with_options(
                        &path,
                        VideoDecoderOptions {
                            cache_budget_bytes: budget,
                            ..VideoDecoderOptions::default()
                        },
                    )
                    .expect("open benchmark decoder");
                    let open_us = started.elapsed().as_micros();
                    let mut elapsed = Duration::ZERO;
                    let mut bytes_peak = 0;
                    for &seconds in requests {
                        let started = Instant::now();
                        let frame = decoder.frame_at(seconds).expect("request benchmark frame");
                        elapsed += started.elapsed();
                        if sample == 0 && mode == "oracle" {
                            let target = info
                                .seconds_to_timestamp(seconds)
                                .expect("request timestamp")
                                .0;
                            let index = pts
                                .partition_point(|&pts| pts <= target)
                                .checked_sub(1)
                                .expect("covering frame");
                            assert_eq!(
                                frame.pts.0, pts[index],
                                "fixture={name} pattern={pattern} seconds={seconds} budget={budget}"
                            );
                            oracle_file
                                .seek(SeekFrom::Start((index * frame_bytes) as u64))
                                .expect("oracle seek");
                            oracle_file
                                .read_exact(&mut oracle_pixels)
                                .expect("oracle RGBA frame");
                            assert_eq!(
                                frame.pixels.as_raw(),
                                &oracle_pixels,
                                "fixture={name} pattern={pattern} seconds={seconds} budget={budget}"
                            );
                        }
                        let metrics = decoder.metrics();
                        bytes_peak = bytes_peak.max(
                            metrics.cache_bytes_retained
                                + metrics.current_frame_bytes_retained
                                + metrics.working_buffer_bytes_retained,
                        );
                        std::hint::black_box(frame);
                    }
                    if sample == 0 && mode == "oracle" {
                        assert!(
                            decoder.frame_at(duration as f64 + 1.0).is_err(),
                            "EOF request"
                        );
                        assert_eq!(
                            decoder
                                .frame_at(0.001)
                                .expect("backward after EOF request")
                                .pts
                                .0,
                            pts[0]
                        );
                    }
                    let metrics = decoder.metrics();
                    measurements.push(Measurement {
                        fixture: name,
                        pattern,
                        cache_budget_bytes: budget,
                        sample,
                        warmup: sample == 0,
                        open_us,
                        frame_requests_us: elapsed.as_micros(),
                        frame_requests: metrics.frame_requests,
                        actual_decodes: metrics.actual_decodes,
                        seeks: metrics.seeks,
                        cache_hits: metrics.cache_hits,
                        cache_misses: metrics.cache_misses,
                        decode_time_us: metrics.decode_time_us,
                        cache_bytes_peak: metrics.cache_bytes_peak,
                        owned_and_visible_buffer_bytes_peak: bytes_peak,
                    });
                }
            }
        }
        fixtures.push(serde_json::json!({"name":name,"width":width,"height":height,"duration_seconds":duration,"frame_rate":"30/1 nominal","frames":pts.len(),"codec":"h264","pixel_format":"yuv420p","gop":gop,"b_frames":3,"vfr":vfr,"oracle_bytes":fs::metadata(&reference).expect("oracle metadata").len()}));
    }
    assert!(!fixtures.is_empty(), "unknown fixture filter");
    assert!(!measurements.is_empty(), "unknown pattern filter");
    let report = serde_json::json!({"fixtures": fixtures, "mode":mode,"pattern_filter":pattern_filter, "measurements": measurements});
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .expect("create new benchmark report");
    serde_json::to_writer_pretty(file, &report).expect("write benchmark report");
    Ok(())
}
