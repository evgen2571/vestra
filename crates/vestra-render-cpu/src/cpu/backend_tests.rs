use super::*;
use vestra_core::plan::{
    ColourTransform, CompileOptions, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer,
    EvaluatedSource, ScheduledItem, TemporalDependency, compile, evaluate,
};

use crate::test_support::{ValidationOptions, load_and_validate};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct VideoFixtureFactory {
    frame: crate::VideoFrame,
}

struct VideoFixtureSession {
    frame: crate::VideoFrame,
}

impl crate::VideoDecoderSession for VideoFixtureSession {
    fn frame_at(&mut self, _seconds: f64) -> Result<crate::VideoFrame, String> {
        Ok(self.frame.clone())
    }
}

impl crate::VideoDecoderFactory for VideoFixtureFactory {
    fn open(
        &self,
        _asset: &vestra_core::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        Ok(Box::new(VideoFixtureSession {
            frame: self.frame.clone(),
        }))
    }
}

struct TimelineVideoFactory {
    requested_times: Arc<Mutex<Vec<f64>>>,
}

struct TimelineVideoSession {
    requested_times: Arc<Mutex<Vec<f64>>>,
    previous_time: Option<f64>,
    seeks: u64,
    requests: u64,
}

impl crate::VideoDecoderSession for TimelineVideoSession {
    fn frame_at(&mut self, seconds: f64) -> Result<crate::VideoFrame, String> {
        self.requests += 1;
        if self
            .previous_time
            .is_some_and(|previous| seconds < previous)
        {
            self.seeks += 1;
        }
        self.previous_time = Some(seconds);
        self.requested_times.lock().unwrap().push(seconds);
        let value = (seconds * 100.0).round().clamp(0.0, 255.0) as u8;
        let colour = if seconds < 1.0 {
            [value, 0, 0, 255]
        } else {
            [0, 0, value, 255]
        };
        Ok(crate::VideoFrame {
            pts: value.into(),
            pixels: Arc::new(image::RgbaImage::from_pixel(1, 1, image::Rgba(colour))),
        })
    }

    fn metrics(&self) -> crate::VideoDecoderMetrics {
        crate::VideoDecoderMetrics {
            frame_requests: self.requests,
            seeks: self.seeks,
            cache_bytes_retained: 8,
            cache_bytes_peak: 12,
            current_frame_bytes_retained: 4,
            working_buffer_bytes_retained: 16,
            ..crate::VideoDecoderMetrics::default()
        }
    }
}

impl crate::VideoDecoderFactory for TimelineVideoFactory {
    fn open(
        &self,
        _asset: &vestra_core::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        Ok(Box::new(TimelineVideoSession {
            requested_times: Arc::clone(&self.requested_times),
            previous_time: None,
            seeks: 0,
            requests: 0,
        }))
    }
}

struct ColourVideoFactory {
    requests: Arc<Mutex<Vec<(String, f64)>>>,
}

struct ColourVideoSession {
    id: String,
    colour: [u8; 4],
    requests: Arc<Mutex<Vec<(String, f64)>>>,
}

struct WorkerBenchmarkVideoSession {
    cache: std::collections::BTreeMap<i64, Arc<image::RgbaImage>>,
    metrics: crate::VideoDecoderMetrics,
}

struct WorkerBenchmarkVideoFactory;

impl crate::VideoDecoderSession for WorkerBenchmarkVideoSession {
    fn frame_at(&mut self, seconds: f64) -> Result<crate::VideoFrame, String> {
        self.metrics.frame_requests += 1;
        let key = (seconds * 15.0).round() as i64;
        let pixels = if let Some(pixels) = self.cache.get(&key) {
            self.metrics.cache_hits += 1;
            Arc::clone(pixels)
        } else {
            self.metrics.cache_misses += 1;
            let started = std::time::Instant::now();
            let pixels = Arc::new(image::RgbaImage::from_fn(320, 180, |x, y| {
                let value = ((x as i64 * 17 + y as i64 * 31 + key * 13) & 255) as u8;
                image::Rgba([value, value.wrapping_add(23), value.wrapping_add(47), 255])
            }));
            self.metrics.actual_decodes += 1;
            self.metrics.decode_time_us += started.elapsed().as_micros() as u64;
            self.cache.insert(key, Arc::clone(&pixels));
            pixels
        };
        Ok(crate::VideoFrame { pts: key, pixels })
    }

    fn metrics(&self) -> crate::VideoDecoderMetrics {
        self.metrics
    }
}

impl crate::VideoDecoderFactory for WorkerBenchmarkVideoFactory {
    fn open(
        &self,
        _asset: &vestra_core::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        Ok(Box::new(WorkerBenchmarkVideoSession {
            cache: std::collections::BTreeMap::new(),
            metrics: crate::VideoDecoderMetrics::default(),
        }))
    }
}

impl crate::VideoDecoderSession for ColourVideoSession {
    fn frame_at(&mut self, seconds: f64) -> Result<crate::VideoFrame, String> {
        self.requests
            .lock()
            .unwrap()
            .push((self.id.clone(), seconds));
        Ok(crate::VideoFrame {
            pts: (seconds * 1_000.0).round() as i64,
            pixels: Arc::new(image::RgbaImage::from_pixel(1, 1, image::Rgba(self.colour))),
        })
    }
}

impl crate::VideoDecoderFactory for ColourVideoFactory {
    fn open(
        &self,
        asset: &vestra_core::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        let colour = match asset.id.as_str() {
            "red" => [255, 0, 0, 255],
            "blue" => [0, 0, 255, 255],
            _ => return Err(format!("unexpected fixture asset '{}'", asset.id)),
        };
        Ok(Box::new(ColourVideoSession {
            id: asset.id.clone(),
            colour,
            requests: Arc::clone(&self.requests),
        }))
    }
}

fn same_video_asset_plan() -> RenderPlan {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
            "output": {
                "path": "same-video.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "video.mp4"}],
            "visual": {"clips": [
                {"id": "first", "source": {"type": "video", "asset": "video"},
                 "start": 0, "duration": 2, "layer": 0,
                 "source_start": 0, "playback_rate": 1,
                 "opacity": {"base_value": 0.5}},
                {"id": "second", "source": {"type": "video", "asset": "video"},
                 "start": 0, "duration": 2, "layer": 1,
                 "source_start": 1, "playback_rate": 1,
                 "opacity": {"base_value": 0.5}}
            ]}
        }"##,
    )
    .expect("same-asset Video fixture parses");
    let paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("video.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 4.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            2,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("same-asset Video fixture compiles")
}

fn static_frame() -> EvaluatedFrame {
    EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 255],
        width: 4,
        height: 4,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 7,
            visible: true,
            content_dependency: TemporalDependency::Static,
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
            source: EvaluatedSource::SolidColor {
                colour: [30, 60, 90, 255],
            },
            opacity: 0.5,
            effects: Vec::new(),
            masks: Vec::new(),
            matte: None,
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    }
}

#[path = "../../../../benchmarks/rust/cpu/native_worker_benchmarks.rs"]
mod native_worker_benchmarks;

#[path = "backend_tests/benchmarks.rs"]
mod benchmarks;

#[path = "backend_tests/memory.rs"]
mod memory;

#[path = "backend_tests/rendering.rs"]
mod rendering;

#[path = "backend_tests/video.rs"]
mod video;

#[path = "backend_tests/workers.rs"]
mod workers;
