//! Real native decoder controls. The bridge mirrors SDK preparation.rs with CPU prefetch off.

use super::*;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::{Weak, mpsc},
    time::Instant,
};
use vestra_media::{FrameSink, VideoDecoder, VideoDecoderOptions, VideoMediaInfo};

struct NativeFactory {
    metadata: Arc<BTreeMap<String, VideoMediaInfo>>,
    records: Arc<Mutex<Vec<Value>>>,
    trace: bool,
}

type Command = (f64, mpsc::Sender<Result<crate::VideoFrame, String>>);
struct NativeSession {
    tx: Option<mpsc::Sender<Command>>,
    metrics: Arc<Mutex<crate::VideoDecoderMetrics>>,
    join: Option<std::thread::JoinHandle<Value>>,
    records: Arc<Mutex<Vec<Value>>>,
    ready_us: u128,
    reply_wait_us: u128,
    waits: Option<Vec<u128>>,
}

impl Drop for NativeSession {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(join) = self.join.take() {
            let mut record = join.join().expect("native owner joins without panic");
            record["ready_wait_us"] = if self.waits.is_some() {
                json!(self.ready_us)
            } else {
                Value::Null
            };
            record["caller_reply_wait_us"] = if self.waits.is_some() {
                json!(self.reply_wait_us)
            } else {
                Value::Null
            };
            if let Some(waits) = &self.waits {
                let trace = record["trace"].as_array_mut().expect("request trace");
                assert_eq!(trace.len(), waits.len());
                for (row, wait) in trace.iter_mut().zip(waits) {
                    row["caller_reply_wait_us"] = json!(wait);
                }
            }
            self.records.lock().expect("session records").push(record);
        }
    }
}

impl crate::VideoDecoderSession for NativeSession {
    fn frame_at(&mut self, seconds: f64) -> Result<crate::VideoFrame, String> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .as_ref()
            .ok_or("native session closed")?
            .send((seconds, tx))
            .map_err(|e| e.to_string())?;
        let started = self.waits.as_ref().map(|_| Instant::now());
        let result = rx.recv().map_err(|e| e.to_string())?;
        if let Some(started) = started {
            let wait = started.elapsed().as_micros();
            self.reply_wait_us += wait;
            self.waits.as_mut().expect("diagnostic waits").push(wait);
        }
        result
    }
    fn metrics(&self) -> crate::VideoDecoderMetrics {
        *self.metrics.lock().expect("native metrics")
    }
}

impl crate::VideoDecoderFactory for NativeFactory {
    fn open(
        &self,
        asset: &vestra_core::plan::VideoAsset,
        cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        let started = self.trace.then(Instant::now);
        let worker = std::thread::current()
            .name()
            .expect("named CPU worker")
            .rsplit('-')
            .next()
            .expect("worker suffix")
            .parse::<usize>()
            .expect("worker id");
        let asset_id = asset.id.clone();
        let path = asset.path.clone();
        let info = self.metadata[&asset.id].clone();
        let trace_enabled = self.trace;
        let (tx, rx) = mpsc::channel::<Command>();
        let (ready_tx, ready_rx) = mpsc::channel();
        let shared_metrics = Arc::new(Mutex::new(crate::VideoDecoderMetrics::default()));
        let owner_metrics = Arc::clone(&shared_metrics);
        let join = std::thread::Builder::new().name(format!("native-diag-{worker}-{asset_id}")).spawn(move || {
            let open_started = trace_enabled.then(Instant::now);
            let opened = VideoDecoder::open_with_info(&path, VideoDecoderOptions { cache_budget_bytes, ..VideoDecoderOptions::default() }, info);
            let mut decoder = match opened {
                Ok(decoder) => { ready_tx.send(Ok(())).expect("readiness receiver"); decoder },
                Err(error) => { let _=ready_tx.send(Err(error.to_string())); return json!({"error":error.to_string()}); }
            };
            let open_us = open_started.map(|started| started.elapsed().as_micros());
            let mut previous: Weak<vestra_media::DecodedVideoFrame> = Weak::new();
            let mut conversions = 0_u64;
            let mut native_frame_at_us = 0_u128;
            let mut trace = Vec::new();
            while let Ok((seconds, reply)) = rx.recv() {
                let before = trace_enabled.then(|| decoder.metrics());
                let request_started = trace_enabled.then(Instant::now);
                let result = decoder.frame_at(seconds);
                let elapsed = request_started.map_or(0, |started| started.elapsed().as_micros());
                native_frame_at_us += elapsed;
                let after = decoder.metrics();
                let result = result.map(|frame| {
                    if let Some(before) = before {
                        let mut converted = false;
                        if after.cache_hits == before.cache_hits {
                            // Only conversion replaces current's Arc; LRU hits leave it alone.
                            converted = previous.as_ptr() != Arc::as_ptr(&frame);
                            conversions += u64::from(converted);
                            previous = Arc::downgrade(&frame);
                        }
                        trace.push(json!({"source_seconds":seconds,"selected_pts":frame.pts.0,"received_frames":after.actual_decodes-before.actual_decodes,"seeks":after.seeks-before.seeks,"cache_hits":after.cache_hits-before.cache_hits,"conversion":converted,"native_frame_at_us":elapsed}));
                    }
                    crate::VideoFrame { pts: frame.pts.0, pixels: Arc::clone(&frame.pixels) }
                }).map_err(|e|e.to_string());
                // Match SDK lock/reply ordering so metrics cannot lag a completed frame.
                let mut current = owner_metrics.lock().expect("native metrics");
                let ended = reply.send(result).is_err();
                *current = crate::VideoDecoderMetrics {
                    frame_requests: after.frame_requests, actual_decodes: after.actual_decodes, seeks: after.seeks,
                    cache_hits: after.cache_hits, cache_misses: after.cache_misses, decode_time_us: after.decode_time_us,
                    cache_bytes_retained: after.cache_bytes_retained, cache_bytes_peak: after.cache_bytes_peak,
                    current_frame_bytes_retained: after.current_frame_bytes_retained,
                    working_buffer_bytes_retained: after.working_buffer_bytes_retained,
                };
                if ended { break; }
            }
            let final_metrics = *owner_metrics.lock().expect("final native metrics");
            drop(decoder); // FFmpeg destruction stays on its owner thread.
            json!({"worker":worker,"asset":asset_id,"cache_budget_bytes":cache_budget_bytes,"opens":1,"closes":1,"open_us":open_us,"native_frame_at_us":if trace_enabled {Some(native_frame_at_us)}else{None},"conversions":if trace_enabled {Some(conversions)}else{None},"metrics":final_metrics,"trace":trace})
        }).map_err(|e|e.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Box::new(NativeSession {
                tx: Some(tx),
                metrics: shared_metrics,
                join: Some(join),
                records: Arc::clone(&self.records),
                ready_us: started.map_or(0, |started| started.elapsed().as_micros()),
                reply_wait_us: 0,
                waits: trace_enabled.then(Vec::new),
            })),
            result => {
                drop(tx);
                let _ = join.join();
                Err(format!("native open failed: {result:?}"))
            }
        }
    }
}

fn oracle_frame(
    frame: &EvaluatedFrame,
    plan: &vestra_core::plan::RenderPlan,
    manifest: &Value,
) -> Vec<u8> {
    let bytes = frame.width as usize * frame.height as usize * 4;
    let mut expected = frame.background.repeat(bytes / 4);
    for layer in &frame.layers {
        let EvaluatedSource::Video {
            asset_index,
            source_time,
            ..
        } = layer.source
        else {
            panic!("video-only oracle");
        };
        assert!(layer.effects.is_empty() && layer.masks.is_empty() && layer.matte.is_none());
        let asset = &manifest["assets"][&plan.videos[asset_index].id];
        let numerator = asset["time_base"][0].as_i64().expect("time base");
        let denominator = asset["time_base"][1].as_i64().expect("time base");
        let target = asset["origin"].as_i64().expect("origin")
            + (source_time * denominator as f64 / numerator as f64).floor() as i64;
        let pts = asset["pts"].as_array().expect("PTS");
        let index = pts
            .iter()
            .rposition(|p| p.as_i64().expect("PTS") <= target)
            .expect("covering reference frame");
        let mut file =
            fs::File::open(asset["rgba"].as_str().expect("oracle path")).expect("external RGBA");
        file.seek(SeekFrom::Start((index * bytes) as u64))
            .expect("oracle seek");
        let mut source = vec![0; bytes];
        file.read_exact(&mut source).expect("oracle frame");
        for (out, input) in expected.chunks_exact_mut(4).zip(source.chunks_exact(4)) {
            assert_eq!(input[3], 255);
            for c in 0..3 {
                out[c] = (f64::from(input[c]) * layer.opacity
                    + f64::from(out[c]) * (1.0 - layer.opacity)
                    + 0.5)
                    .floor()
                    .clamp(0.0, 255.0) as u8;
            }
            out[3] = 255;
        }
    }
    expected
}

#[expect(
    clippy::assertions_on_constants,
    reason = "ignored timing diagnostics deliberately reject unoptimized builds"
)]
fn native_workers(workers: usize) {
    assert!(!cfg!(debug_assertions), "release-only diagnostic");
    let inputs =
        PathBuf::from(std::env::var("VESTRA_NATIVE_BENCH_INPUTS").expect("prepared inputs"));
    let output =
        PathBuf::from(std::env::var("VESTRA_NATIVE_BENCH_OUTPUT").expect("fresh output JSON"));
    assert!(!output.exists());
    let case = std::env::var("VESTRA_NATIVE_BENCH_CASE").expect("case");
    assert!(
        [
            "single-video",
            "video-heavy",
            "holds",
            "trimmed",
            "divergent-clocks"
        ]
        .contains(&case.as_str())
    );
    let sink = std::env::var("VESTRA_NATIVE_BENCH_SINK").expect("null or encoded");
    assert!(["null", "encoded"].contains(&sink.as_str()));
    let mode = std::env::var("VESTRA_NATIVE_BENCH_MODE").expect("oracle or timing");
    assert!(["oracle", "timing"].contains(&mode.as_str()));
    let oracle = mode == "oracle";
    let manifest: Value =
        serde_json::from_slice(&fs::read(inputs.join("manifest.json")).expect("manifest"))
            .expect("manifest JSON");
    let project = crate::project::Project::from_json(
        &fs::read_to_string(inputs.join(format!("{case}.json"))).expect("scene"),
    )
    .expect("scene parses");
    let mut paths = BTreeMap::new();
    let mut durations = BTreeMap::new();
    let mut dimensions = BTreeMap::new();
    let mut metadata = BTreeMap::new();
    for asset in manifest["assets"].as_object().expect("assets") {
        let path = PathBuf::from(asset.1["path"].as_str().expect("source path"));
        let info = vestra_media::probe_video(&path).expect("real native probe");
        paths.insert(asset.0.clone(), path);
        durations.insert(asset.0.clone(), info.duration_seconds.expect("duration"));
        dimensions.insert(asset.0.clone(), (info.coded_width, info.coded_height));
        metadata.insert(asset.0.clone(), info);
    }
    let plan = compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            &inputs,
            &paths,
            &BTreeMap::new(),
            1.0,
            (30, 1),
            30,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        CompileOptions::default(),
    )
    .expect("diagnostic compiles");
    assert_eq!(plan.frame_count, 30);
    assert_eq!((plan.canvas.width, plan.canvas.height), (1280, 720));
    let active = vestra_core::plan::ActiveSchedule::compile(&plan);
    let frames = (0..plan.frame_count)
        .map(|n| {
            evaluate(
                &plan,
                &active.active_at(&plan, n),
                vestra_core::timeline::frame_time_nanos(n, 30, 1).expect("frame time"),
            )
            .expect("evaluate")
        })
        .collect::<Vec<_>>();
    let metadata = Arc::new(metadata);
    let mut rows = Vec::new();
    for sample in 0..if oracle { 1 } else { 6 } {
        let records = Arc::new(Mutex::new(Vec::new()));
        let factory = Arc::new(NativeFactory {
            metadata: Arc::clone(&metadata),
            records: Arc::clone(&records),
            trace: oracle,
        });
        let decoded =
            DecodedAssets::build_with_video_factory(&plan, Some(factory)).expect("native assets");
        let setup = Instant::now();
        let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, workers);
        let setup_ms = setup.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(backend.capacity(), workers);
        let partitions=backend.worker_cache_budgets.iter().enumerate().map(|(worker,b)|json!({"worker":worker,"static_bytes":b.static_cache_budget_bytes,"crop_bytes":b.crop_cache_budget_bytes,"video_bytes":b.video_cache_budget_bytes})).collect::<Vec<_>>();
        let started = Instant::now();
        let encoded_path = output.with_file_name(format!("sample-{sample}.mp4"));
        assert!(!encoded_path.exists(), "fresh encoded output");
        let mut encoder = (sink == "encoded").then(|| {
            vestra_media::FfmpegSink::start(&plan.encoder, &encoded_path).expect("encoder starts")
        });
        let mut ready = BTreeMap::new();
        let mut submit = 0_usize;
        let mut written = 0_usize;
        let mut poll_wait_us = 0_u128;
        let mut writer_us = 0_u128;
        let mut owned_peak = 0;
        while written < frames.len() {
            while submit < frames.len() && backend.in_flight() < workers && ready.len() < workers {
                backend
                    .submit_frame(submit as u64, &frames[submit])
                    .expect("submit");
                submit += 1;
            }
            owned_peak = owned_peak.max(backend.in_flight() + ready.len());
            assert!(owned_peak <= workers * 2);
            let wait = oracle.then(Instant::now);
            let completed = backend
                .poll_completed(PollMode::WaitForOne)
                .expect("native poll")
                .expect("completion");
            poll_wait_us += wait.map_or(0, |wait| wait.elapsed().as_micros());
            ready.insert(completed.frame_number as usize, completed);
            while let Some(frame) = ready.remove(&written) {
                if oracle {
                    let expected = oracle_frame(&frames[written], &plan, &manifest);
                    if frame.rgba != expected {
                        let i = frame
                            .rgba
                            .iter()
                            .zip(&expected)
                            .position(|(a, b)| a != b)
                            .expect("mismatch");
                        panic!(
                            "oracle mismatch {case} workers={workers} frame={written} byte={i}: actual={} expected={}",
                            frame.rgba[i], expected[i]
                        );
                    }
                    if [0, 15, 29].contains(&written) {
                        image::save_buffer(
                            output.with_file_name(format!("frame-{written}.png")),
                            &frame.rgba,
                            1280,
                            720,
                            image::ColorType::Rgba8,
                        )
                        .expect("save verified frame");
                    }
                }
                let write = oracle.then(Instant::now);
                if let Some(encoder) = &mut encoder {
                    encoder
                        .write_frame(written as u64, &frame.rgba)
                        .expect("encode frame");
                } else {
                    std::hint::black_box(&frame.rgba);
                };
                writer_us += write.map_or(0, |write| write.elapsed().as_micros());
                written += 1;
            }
        }
        if let Some(encoder) = &mut encoder {
            assert_eq!(encoder.finish().expect("encoder finish").frames_written, 30);
        }
        let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
        let stats = backend.stats();
        let staged = backend.staged_metrics();
        let teardown = Instant::now();
        drop(backend);
        let teardown_ms = teardown.elapsed().as_secs_f64() * 1000.0;
        let sessions = std::mem::take(&mut *records.lock().expect("sessions"));
        assert_eq!(sessions.len() as u64, stats.video_decoder_open_count);
        for (key, total) in [
            ("frame_requests", stats.video_frame_requests),
            ("actual_decodes", stats.video_actual_decodes),
            ("seeks", stats.video_seek_count),
            ("cache_hits", stats.video_cache_hits),
            ("cache_misses", stats.video_cache_misses),
        ] {
            assert_eq!(
                sessions
                    .iter()
                    .map(|s| s["metrics"][key].as_u64().expect("counter"))
                    .sum::<u64>(),
                total,
                "counter {key}"
            );
        }
        assert!(sessions.iter().all(|s| s["closes"] == 1));
        let conversions = oracle.then(|| {
            sessions
                .iter()
                .map(|s| s["conversions"].as_u64().expect("conversions"))
                .sum::<u64>()
        });
        if let Some(conversions) = conversions {
            assert!(conversions <= stats.video_frame_requests);
        }
        if oracle {
            for s in &sessions {
                let trace = s["trace"].as_array().expect("trace");
                assert_eq!(
                    trace.len() as u64,
                    s["metrics"]["frame_requests"].as_u64().expect("requests")
                );
                for (event_key, metric_key) in [
                    ("received_frames", "actual_decodes"),
                    ("seeks", "seeks"),
                    ("cache_hits", "cache_hits"),
                ] {
                    assert_eq!(
                        trace
                            .iter()
                            .map(|q| q[event_key].as_u64().expect("event counter"))
                            .sum::<u64>(),
                        s["metrics"][metric_key].as_u64().expect("session counter")
                    );
                }
                assert_eq!(
                    trace.iter().filter(|q| q["conversion"] == true).count() as u64,
                    s["conversions"].as_u64().expect("conversions")
                );
                let asset = &manifest["assets"][s["asset"].as_str().expect("asset")];
                for q in s["trace"].as_array().expect("trace") {
                    let target = asset["origin"].as_i64().expect("origin")
                        + (q["source_seconds"].as_f64().expect("seconds")
                            * asset["time_base"][1].as_i64().expect("den") as f64
                            / asset["time_base"][0].as_i64().expect("num") as f64)
                            .floor() as i64;
                    let expected = asset["pts"]
                        .as_array()
                        .expect("PTS")
                        .iter()
                        .rfind(|p| p.as_i64().expect("PTS") <= target)
                        .expect("selected PTS");
                    assert_eq!(
                        &q["selected_pts"], expected,
                        "independent source PTS oracle"
                    );
                }
            }
        }
        rows.push(json!({"sample":sample,"warmup":!oracle && sample==0,"wall_ms":wall_ms,"backend_start_ms":setup_ms,"drop_join_ms":teardown_ms,"poll_wait_us":if oracle {Some(poll_wait_us)}else{None},"writer_us":if oracle {Some(writer_us)}else{None},"worker_frame_work_ms":staged.frame_render_work_duration.as_secs_f64()*1000.0,"outstanding_frame_bound_peak":owned_peak,"conversions":conversions,"stats":stats,"partitions":partitions,"sessions":sessions,"verified_full_rgba_frames":if oracle {30}else{0}}));
    }
    fs::write(output,serde_json::to_vec_pretty(&json!({"mode":mode,"case":case,"sink":sink,"workers":workers,"backend":"cpu","width":1280,"height":720,"frames":30,"fps":[30,1],"quality":"preview ultrafast/CRF30 yuv420p","prefetch":false,"scope":"internal CPU coordinator, native SDK-equivalent owner-thread bridge; timing excludes oracle/trace; pipeline wall includes encoder start/finish when encoded","rows":rows})).expect("native report")).expect("write report");
}

#[test]
#[ignore = "real native worker diagnostic; release and prepared inputs required"]
fn native_workers_1() {
    native_workers(1)
}
#[test]
#[ignore = "real native worker diagnostic; release and prepared inputs required"]
fn native_workers_2() {
    native_workers(2)
}
#[test]
#[ignore = "real native worker diagnostic; release and prepared inputs required"]
fn native_workers_4() {
    native_workers(4)
}
#[test]
#[ignore = "real native worker diagnostic; release and prepared inputs required"]
fn native_workers_8() {
    native_workers(8)
}
