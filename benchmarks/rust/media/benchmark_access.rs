//! Collector-only harness for the existing native decoder and isolated RGBA cache.
//! No production API or cache semantics are changed.

use std::{error::Error, fs, path::PathBuf, time::Instant};

use serde::Deserialize;
use serde_json::json;
use vestra_media::{VideoDecoder, VideoDecoderOptions};

#[derive(Deserialize)]
struct Request {
    session: usize,
    seconds: f64,
}

#[derive(Deserialize)]
struct Workload {
    source: PathBuf,
    cache_budget_bytes: u64,
    sessions: usize,
    requests: Vec<Request>,
    verify: bool,
}

fn run() -> Result<(), Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Err("benchmark requires release mode".into());
    }
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: benchmark_access REQUEST.json RESULT.json".into());
    }
    let workload: Workload = serde_json::from_slice(&fs::read(&args[1])?)?;
    if !(1..=4).contains(&workload.sessions) || workload.requests.len() > 100_000 {
        return Err("invalid bounded workload".into());
    }
    let started = Instant::now();
    let mut decoders = (0..workload.sessions)
        .map(|_| {
            VideoDecoder::open_with_options(
                &workload.source,
                VideoDecoderOptions {
                    cache_budget_bytes: workload.cache_budget_bytes,
                    ..VideoDecoderOptions::default()
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let open_us = started.elapsed().as_micros();
    // Exact byte comparisons are a SEPARATE correctness invocation. The
    // reference decoder and pixel scans never run in performance samples.
    let mut references = if workload.verify {
        (0..workload.sessions)
            .map(|_| {
                VideoDecoder::open_with_options(
                    &workload.source,
                    VideoDecoderOptions {
                        cache_budget_bytes: 0,
                        ..VideoDecoderOptions::default()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let mut latencies_us = Vec::with_capacity(workload.requests.len());
    let mut verified_frames = 0;
    for request in &workload.requests {
        let decoder = decoders.get_mut(request.session).ok_or("invalid session")?;
        let started = Instant::now();
        let frame = decoder.frame_at(request.seconds)?;
        latencies_us.push(started.elapsed().as_micros());
        if workload.verify {
            let expected = references[request.session].frame_at(request.seconds)?;
            if frame.pts != expected.pts || frame.pixels.as_raw() != expected.pixels.as_raw() {
                return Err(format!("cache changed pixels at {} seconds", request.seconds).into());
            }
            verified_frames += 1;
        }
        std::hint::black_box(frame);
    }
    let metrics: Vec<_> = decoders
        .iter()
        .map(|decoder| {
            let m = decoder.metrics();
            json!({
                "frame_requests": m.frame_requests, "actual_decodes": m.actual_decodes,
                "seeks": m.seeks, "cache_hits": m.cache_hits, "cache_misses": m.cache_misses,
                "decode_time_us": m.decode_time_us, "cache_bytes_retained": m.cache_bytes_retained,
                "cache_bytes_peak": m.cache_bytes_peak,
                "current_frame_bytes_retained": m.current_frame_bytes_retained,
                "working_buffer_bytes_retained": m.working_buffer_bytes_retained,
                "cache_evictions": null,
            })
        })
        .collect();
    fs::write(
        &args[2],
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1, "status": "passed", "open_us": open_us,
            "frame_requests_us": latencies_us.iter().sum::<u128>(),
            "latencies_us": latencies_us, "metrics": metrics,
            "decoder_opens": workload.sessions, "workers": 1,
            "cache_budget_bytes_per_session": workload.cache_budget_bytes,
            "exact_verified_frames": verified_frames,
            "timing_scope": "native frame_at calls only; opens and correctness separate",
            "static_cache_hits": null, "bitmap_cache_hits": null,
            "unavailable": "eviction count is not exposed; static/bitmap caches are not exercised",
        }))?,
    )?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("benchmark access: {error}");
        std::process::exit(1);
    }
}
