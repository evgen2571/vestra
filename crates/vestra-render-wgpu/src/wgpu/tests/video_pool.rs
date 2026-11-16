//! Decoder policy tests; none of these initialize WGPU.
use image::RgbaImage;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use vestra_core::plan::VideoAsset;
use vestra_render_common::{
    VideoDecoderFactory, VideoDecoderMetrics, VideoDecoderSession, VideoFrame,
};

use super::video_pool::VideoPool;

#[derive(Default)]
pub(super) struct CursorTrace {
    pub(super) requests: Vec<Vec<f64>>,
    pub(super) budgets: Vec<u64>,
    assets: Vec<String>,
    prefetches: usize,
    pub(super) live: usize,
}

pub(super) struct CursorFactory(pub(super) Arc<std::sync::Mutex<CursorTrace>>);
struct CursorSession {
    trace: Arc<std::sync::Mutex<CursorTrace>>,
    id: usize,
    metrics: VideoDecoderMetrics,
    previous: f64,
}

impl Drop for CursorSession {
    fn drop(&mut self) {
        self.trace.lock().unwrap().live -= 1;
    }
}

impl VideoDecoderFactory for CursorFactory {
    fn open(
        &self,
        asset: &vestra_core::plan::VideoAsset,
        budget: u64,
    ) -> Result<Box<dyn VideoDecoderSession>, String> {
        let mut trace = self.0.lock().unwrap();
        let id = trace.requests.len();
        trace.requests.push(Vec::new());
        trace.budgets.push(budget);
        trace.assets.push(asset.id.clone());
        trace.live += 1;
        Ok(Box::new(CursorSession {
            trace: Arc::clone(&self.0),
            id,
            metrics: VideoDecoderMetrics {
                cache_bytes_retained: budget.min(4),
                cache_bytes_peak: budget.min(4),
                current_frame_bytes_retained: 4,
                working_buffer_bytes_retained: 8,
                ..Default::default()
            },
            previous: 0.0,
        }))
    }
}

impl VideoDecoderSession for CursorSession {
    fn frame_at(&mut self, seconds: f64) -> Result<VideoFrame, String> {
        if seconds < 0.0 {
            return Err("fake decode failure".to_owned());
        }
        self.trace.lock().unwrap().requests[self.id].push(seconds);
        let held = self.metrics.frame_requests > 0 && seconds == self.previous;
        self.metrics.cache_hits += u64::from(held);
        self.metrics.cache_misses += u64::from(!held);
        self.metrics.actual_decodes += u64::from(!held);
        self.metrics.decode_time_us += u64::from(!held) * 10;
        self.metrics.frame_requests += 1;
        self.metrics.seeks += u64::from(seconds < self.previous);
        self.previous = seconds;
        Ok(VideoFrame {
            pts: (seconds * 1000.0).round() as i64,
            pixels: Arc::new(RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([(seconds * 40.0).round() as u8, 0, 0, 255]),
            )),
        })
    }

    fn enable_prefetch(&mut self) {
        self.trace.lock().unwrap().prefetches += 1;
    }

    fn metrics(&self) -> VideoDecoderMetrics {
        self.metrics
    }
}

fn asset(id: &str) -> VideoAsset {
    VideoAsset {
        id: id.to_owned(),
        path: format!("{id}.mp4").into(),
        duration_seconds: 8.0,
        width: 1,
        height: 1,
    }
}

fn prepare(slot_assets: &[usize]) -> (VideoPool, Arc<Mutex<CursorTrace>>) {
    let trace = Arc::new(Mutex::new(CursorTrace::default()));
    let (pool, _) = VideoPool::new(
        &[asset("first"), asset("unused"), asset("third")],
        slot_assets,
        96,
        Some(Arc::new(CursorFactory(Arc::clone(&trace)))),
    )
    .expect("video pool prepares without an adapter");
    (pool, trace)
}

#[test]
fn video_pool_opens_only_used_assets_and_splits_primary_cache_budgets() {
    let (pool, trace) = prepare(&[2, 2, 0]);
    let trace = trace.lock().unwrap();
    assert_eq!(trace.assets, ["first", "third"]);
    assert_eq!(trace.budgets, [48, 48]);
    assert_eq!(trace.requests, [vec![0.0], vec![0.0]]);
    assert_eq!(trace.prefetches, 2);
    assert_eq!(pool.session_count(), 2);
    assert_eq!(pool.open_count(), 2);
    assert_eq!(pool.metrics().frame_requests, 2);
}

#[test]
fn video_pool_keeps_independent_cursors_and_shares_only_equal_results() {
    let (mut pool, trace) = prepare(&[0, 0, 0]);
    for time in [0.0, 0.25, 0.5] {
        let frames = pool
            .frames(BTreeMap::from([
                (10, (0, time)),
                (11, (0, 2.0 + time * 0.5)),
                (12, (0, time)),
            ]))
            .unwrap();
        assert_eq!(
            frames
                .iter()
                .map(|(slot, frame)| (*slot, frame.pts))
                .collect::<Vec<_>>(),
            [
                (10, (time * 1000.0).round() as i64),
                (11, ((2.0 + time * 0.5) * 1000.0).round() as i64),
                (12, (time * 1000.0).round() as i64)
            ]
        );
        assert!(Arc::ptr_eq(&frames[0].1.pixels, &frames[2].1.pixels));
    }
    assert_eq!(pool.metrics().frame_requests, 7);
    assert_eq!(pool.metrics().seeks, 0);
    assert_eq!(pool.open_count(), 2);
    assert_eq!(
        trace.lock().unwrap().requests,
        [vec![0.0, 0.0, 0.25, 0.5], vec![2.0, 2.125, 2.25]]
    );
    let frames = pool
        .frames(BTreeMap::from([
            (10, (0, 0.75)),
            (11, (0, 2.375)),
            (12, (0, 1.5)),
        ]))
        .unwrap();
    assert_eq!(frames[2].1.pts, 1500);
    assert!(!Arc::ptr_eq(&frames[0].1.pixels, &frames[2].1.pixels));
    assert_eq!(pool.open_count(), 3);
    assert_eq!(pool.session_count(), 3);
    assert_eq!(pool.metrics().seeks, 0);
    assert_eq!(trace.lock().unwrap().budgets, [96, 0, 0]);
}

#[test]
fn video_pool_does_not_share_equal_times_across_assets() {
    let (mut pool, _) = prepare(&[0, 2]);
    let frames = pool
        .frames(BTreeMap::from([(10, (0, 0.5)), (11, (2, 0.5))]))
        .unwrap();
    assert_eq!(frames[0].1.pts, frames[1].1.pts);
    assert!(!Arc::ptr_eq(&frames[0].1.pixels, &frames[1].1.pixels));
    assert_eq!(pool.metrics().frame_requests, 4);
}

#[test]
fn video_pool_retirement_and_reactivation_preserve_live_cursors_and_cumulative_metrics() {
    let (mut pool, trace) = prepare(&[0, 0]);
    pool.frames(BTreeMap::from([(10, (0, 0.25)), (11, (0, 2.0))]))
        .unwrap();
    pool.frames(BTreeMap::from([(11, (0, 2.25))])).unwrap();
    assert_eq!(trace.lock().unwrap().requests[1], [2.0, 2.25]);
    assert_eq!(
        pool.metrics().seeks,
        0,
        "surviving cursor was not reassigned"
    );
    // Retire a cursor with both cache hits and explicit seeks in its history.
    for time in [2.25, 2.0, 2.25] {
        pool.frames(BTreeMap::from([(11, (0, time))])).unwrap();
    }
    let before_idle = pool.metrics();
    assert!(before_idle.cache_hits > 0);
    assert!(before_idle.seeks > 0);
    pool.frames(BTreeMap::new()).unwrap();
    let idle = pool.metrics();
    assert_eq!(idle.frame_requests, before_idle.frame_requests);
    assert_eq!(idle.actual_decodes, before_idle.actual_decodes);
    assert_eq!(idle.seeks, before_idle.seeks);
    assert_eq!(idle.cache_hits, before_idle.cache_hits);
    assert_eq!(idle.cache_misses, before_idle.cache_misses);
    assert_eq!(idle.decode_time_us, before_idle.decode_time_us);
    assert_eq!(idle.cache_bytes_peak, before_idle.cache_bytes_peak);
    assert_eq!(idle.cache_bytes_retained, 4);
    assert_eq!(idle.current_frame_bytes_retained, 4);
    assert_eq!(idle.working_buffer_bytes_retained, 8);
    assert_eq!(pool.session_count(), 1);
    assert_eq!(trace.lock().unwrap().live, 1);
    pool.frames(BTreeMap::from([(10, (0, 0.5)), (11, (0, 2.5))]))
        .unwrap();
    assert_eq!(pool.session_count(), 2);
    assert_eq!(pool.open_count(), 3);
    assert_eq!(pool.metrics().frame_requests, idle.frame_requests + 2);
    assert_eq!(pool.metrics().actual_decodes, idle.actual_decodes + 2);
    assert_eq!(pool.metrics().cache_misses, idle.cache_misses + 2);
    assert_eq!(pool.metrics().decode_time_us, idle.decode_time_us + 20);
    assert_eq!(pool.metrics().seeks, idle.seeks);
    pool.frames(BTreeMap::from([(10, (0, 0.125)), (11, (0, 2.125))]))
        .unwrap();
    assert_eq!(pool.metrics().seeks, idle.seeks + 2);
    drop(pool);
    assert_eq!(trace.lock().unwrap().live, 0);
}

#[test]
fn video_pool_validates_factory_and_assets_and_preserves_decode_errors() {
    assert_eq!(
        VideoPool::new(&[asset("video")], &[0], 0, None)
            .err()
            .unwrap()
            .code,
        "WGPU-VIDEO-FACTORY"
    );
    let (empty, frames) = VideoPool::new(&[], &[], 0, None).unwrap();
    assert_eq!(empty.session_count(), 0);
    assert!(frames.is_empty());
    let factory: Arc<dyn VideoDecoderFactory> = Arc::new(CursorFactory(Arc::default()));
    assert_eq!(
        VideoPool::new(&[], &[0], 0, Some(factory))
            .err()
            .unwrap()
            .code,
        "WGPU-VIDEO-ASSET"
    );
    let (mut pool, _) = prepare(&[0]);
    assert_eq!(
        pool.frames(BTreeMap::from([(10, (2, 0.5))]))
            .unwrap_err()
            .code,
        "WGPU-VIDEO-DECODER"
    );
    let error = pool.frames(BTreeMap::from([(10, (0, -1.0))])).unwrap_err();
    assert_eq!(error.code, "WGPU-VIDEO-DECODE");
    assert_eq!(error.message, "fake decode failure");
}
