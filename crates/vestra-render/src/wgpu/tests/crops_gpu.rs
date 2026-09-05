//! Adapter-dependent WGPU crop parity tests.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use super::{compare_rgba, gpu::wgpu_backend_or_skip};
use vestra_core::plan::{CompileOptions, compile};

use crate::{
    animation::{Interpolation, Keyframe, Track},
    domain::Crop,
    render::CpuBackend,
    test_support::{ValidationOptions, load_and_validate},
};
use image::RgbaImage;

struct VideoFixtureFactory {
    frame: crate::VideoFrame,
}

struct VideoFixtureSession {
    frame: crate::VideoFrame,
}

struct CountingVideoFactory {
    opens: Arc<AtomicUsize>,
    frame_requests: Arc<AtomicUsize>,
}

struct CountingVideoSession {
    frame: crate::VideoFrame,
    frame_requests: Arc<AtomicUsize>,
}

struct TimingVideoFactory {
    requests: Arc<Mutex<Vec<f64>>>,
}

struct TimingVideoSession {
    requests: Arc<Mutex<Vec<f64>>>,
}

struct TransitionVideoFactory {
    requests: Arc<Mutex<Vec<(String, f64)>>>,
}

struct TransitionVideoSession {
    asset_id: String,
    requests: Arc<Mutex<Vec<(String, f64)>>>,
}

impl crate::VideoDecoderSession for TimingVideoSession {
    fn frame_at(&mut self, seconds: f64) -> Result<crate::VideoFrame, String> {
        self.requests.lock().unwrap().push(seconds);
        let value = (seconds * 100.0).round().clamp(0.0, 255.0) as u8;
        Ok(crate::VideoFrame {
            pts: (seconds * 1_000.0).round() as i64,
            pixels: Arc::new(image::RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([value, 0, 255 - value, 255]),
            )),
        })
    }
}

impl crate::VideoDecoderFactory for TimingVideoFactory {
    fn open(
        &self,
        _asset: &vestra_core::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        Ok(Box::new(TimingVideoSession {
            requests: Arc::clone(&self.requests),
        }))
    }
}

impl crate::VideoDecoderSession for TransitionVideoSession {
    fn frame_at(&mut self, seconds: f64) -> Result<crate::VideoFrame, String> {
        self.requests
            .lock()
            .unwrap()
            .push((self.asset_id.clone(), seconds));
        let base = if self.asset_id == "video-a" { 32 } else { 224 };
        let value = (base as f64 + seconds * 16.0).round().clamp(0.0, 255.0) as u8;
        Ok(crate::VideoFrame {
            pts: (seconds * 4.0).floor() as i64,
            pixels: Arc::new(image::RgbaImage::from_pixel(
                1,
                1,
                image::Rgba([value, 0, 255 - value, 255]),
            )),
        })
    }
}

impl crate::VideoDecoderFactory for TransitionVideoFactory {
    fn open(
        &self,
        asset: &vestra_core::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        Ok(Box::new(TransitionVideoSession {
            asset_id: asset.id.clone(),
            requests: Arc::clone(&self.requests),
        }))
    }
}

impl crate::VideoDecoderSession for CountingVideoSession {
    fn frame_at(&mut self, _seconds: f64) -> Result<crate::VideoFrame, String> {
        self.frame_requests.fetch_add(1, Ordering::Relaxed);
        Ok(self.frame.clone())
    }
}

impl crate::VideoDecoderFactory for CountingVideoFactory {
    fn open(
        &self,
        _asset: &vestra_core::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        self.opens.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(CountingVideoSession {
            frame: crate::VideoFrame {
                pts: 0,
                pixels: Arc::new(image::RgbaImage::from_pixel(
                    1,
                    1,
                    image::Rgba([255, 0, 0, 255]),
                )),
            },
            frame_requests: Arc::clone(&self.frame_requests),
        }))
    }
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

#[test]
fn gpu_video_preparation_ignores_unused_assets_when_an_adapter_is_available() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "unused-video.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [
                {"id": "used", "type": "video", "source": "used.mp4"},
                {"id": "unused", "type": "video", "source": "unused.mp4"}
            ],
            "visual": {"clips": [{
                "id": "video-layer", "source": {"type": "video", "asset": "used"},
                "start": 0, "duration": 1, "layer": 0,
                "opacity": {"base_value": 1}
            }]}
        }"##,
    )
    .expect("unused-video fixture parses");
    let paths = std::collections::BTreeMap::from([
        ("used".to_owned(), std::path::PathBuf::from("used.mp4")),
        ("unused".to_owned(), std::path::PathBuf::from("unused.mp4")),
    ]);
    let durations =
        std::collections::BTreeMap::from([("used".to_owned(), 1.0), ("unused".to_owned(), 1.0)]);
    let dimensions = std::collections::BTreeMap::from([("used".to_owned(), (1, 1))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            1,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("unused-video fixture compiles");
    let opens = Arc::new(AtomicUsize::new(0));
    let frame_requests = Arc::new(AtomicUsize::new(0));
    let decoded = crate::DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(CountingVideoFactory {
            opens: Arc::clone(&opens),
            frame_requests: Arc::clone(&frame_requests),
        })),
    )
    .expect("unused-video fixture decodes");
    let Some(mut backend) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    assert_eq!(opens.load(Ordering::Relaxed), 1);
    assert_eq!(frame_requests.load(Ordering::Relaxed), 1);
    assert_eq!(backend.video_upload_count(), 0);

    let frame = vestra_core::plan::evaluate(&plan, &[vestra_core::plan::ScheduledItem(0)], 0)
        .expect("renderer fixture evaluates");
    let mut output = RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("unused-video fixture renders");
    assert_eq!(frame_requests.load(Ordering::Relaxed), 2);
    assert_eq!(backend.video_upload_count(), 0);
    backend
        .render_frame(&frame, &mut output)
        .expect("held-video fixture renders");
    assert_eq!(frame_requests.load(Ordering::Relaxed), 3);
    assert_eq!(backend.video_upload_count(), 0);
}

#[test]
fn gpu_video_source_timing_matches_cpu_when_an_adapter_is_available() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "video-timing.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "video.mp4"}],
            "visual": {"clips": [{
                "id": "video-layer", "source": {"type": "video", "asset": "video"},
                "start": 0, "duration": 2, "layer": 0,
                "source_start": 0.25, "playback_rate": 2,
                "opacity": {"base_value": 1}
            }]}
        }"##,
    )
    .expect("Video timing project parses");
    let paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("video.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 4.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = vestra_core::plan::compile(
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
    .expect("Video timing project compiles");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let decoded = crate::DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimingVideoFactory {
            requests: Arc::clone(&requests),
        })),
    )
    .expect("Video timing assets decode");
    let frame =
        vestra_core::plan::evaluate(&plan, &[vestra_core::plan::ScheduledItem(0)], 500_000_000)
            .expect("renderer fixture evaluates");
    let mut cpu_output = RgbaImage::new(1, 1);
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    cpu.render_frame(&frame, &mut cpu_output)
        .expect("CPU timing frame renders");
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    let mut gpu_output = RgbaImage::new(1, 1);
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("WGPU timing frame renders");

    assert!(requests.lock().unwrap().contains(&1.25));
    assert_eq!(
        cpu_output,
        RgbaImage::from_pixel(1, 1, image::Rgba([125, 0, 130, 255]))
    );
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
    assert!(
        difference.maximum_absolute_channel_error <= 2,
        "Video timing parity exceeded tolerance: {difference:?}"
    );
    let first_cpu = cpu_output.clone();
    let first_gpu = gpu_output.clone();
    for time in [1_500_000_000, 250_000_000, 750_000_000, 500_000_000] {
        let frame =
            vestra_core::plan::evaluate(&plan, &[vestra_core::plan::ScheduledItem(0)], time)
                .expect("renderer fixture evaluates");
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU random-access timing frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("WGPU random-access timing frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "random-access Video timing parity exceeded tolerance: {difference:?}"
        );
    }
    assert_eq!(cpu_output, first_cpu);
    assert_eq!(gpu_output, first_gpu);
}

#[test]
fn gpu_video_to_video_transition_advances_both_sources_and_matches_cpu() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "video-transition.mp4", "width": 1, "height": 1,
                "frame_rate": "4/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "explicit", "duration": 1
            },
            "assets": [
                {"id": "video-a", "type": "video", "source": "a.mp4"},
                {"id": "video-b", "type": "video", "source": "b.mp4"}
            ],
            "visual": {
                "clips": [
                    {"id": "video-a", "source": {"type": "video", "asset": "video-a"}, "start": 0, "duration": 0.75, "layer": 0, "opacity": {"base_value": 1}},
                    {"id": "video-b", "source": {"type": "video", "asset": "video-b"}, "start": 0.25, "duration": 0.75, "layer": 0, "opacity": {"base_value": 1}}
                ],
                "transitions": [{
                    "id": "video-crossfade", "outgoing": "video-a", "incoming": "video-b", "start": 0.25, "duration": 0.5,
                    "definition": {
                        "outgoing": {"opacity": {"keyframes": [{"progress": 0, "value": 1, "interpolation": "linear"}, {"progress": 1, "value": 0, "interpolation": "linear"}]}},
                        "incoming": {"opacity": {"keyframes": [{"progress": 0, "value": 0, "interpolation": "linear"}, {"progress": 1, "value": 1, "interpolation": "linear"}]}}
                    }
                }]
            }
        }"##,
    )
    .expect("Video transition project parses");
    let paths = std::collections::BTreeMap::from([
        ("video-a".to_owned(), std::path::PathBuf::from("a.mp4")),
        ("video-b".to_owned(), std::path::PathBuf::from("b.mp4")),
    ]);
    let durations = std::collections::BTreeMap::from([
        ("video-a".to_owned(), 1.0),
        ("video-b".to_owned(), 1.0),
    ]);
    let dimensions = std::collections::BTreeMap::from([
        ("video-a".to_owned(), (1, 1)),
        ("video-b".to_owned(), (1, 1)),
    ]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            4,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("Video transition project compiles");
    assert_eq!(plan.video_slot_count(), 2);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let decoded = crate::DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TransitionVideoFactory {
            requests: Arc::clone(&requests),
        })),
    )
    .expect("Video transition assets decode");
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    for time in [500_000_000, 750_000_000] {
        let frame = vestra_core::plan::evaluate(
            &plan,
            &[
                vestra_core::plan::ScheduledItem(0),
                vestra_core::plan::ScheduledItem(1),
            ],
            time,
        )
        .expect("renderer fixture evaluates");
        let mut cpu_output = RgbaImage::new(1, 1);
        let mut gpu_output = RgbaImage::new(1, 1);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU transition frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("WGPU transition frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "Video transition parity exceeded tolerance: {difference:?}"
        );
    }
    let requests = requests.lock().unwrap();
    assert!(requests.iter().any(|(asset, _)| asset == "video-a"));
    assert!(requests.iter().any(|(asset, _)| asset == "video-b"));
    assert!(
        requests
            .iter()
            .filter(|(asset, _)| asset == "video-a")
            .count()
            > 1
    );
    assert!(
        requests
            .iter()
            .filter(|(asset, _)| asset == "video-b")
            .count()
            > 1
    );
}

#[test]
fn gpu_video_crop_matches_cpu_for_a_dynamic_frame_when_an_adapter_is_available() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "video-crop.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "fixture.mp4"}],
            "visual": {"clips": [{
                "id": "video-layer", "source": {"type": "video", "asset": "video"},
                "start": 0, "duration": 1, "layer": 0,
                "opacity": {"base_value": 1},
                "crop": {"base_value": {"x": 0.5, "y": 0, "width": 0.5, "height": 1}}
            }]}
        }"##,
    )
    .expect("video fixture project parses");
    let paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("fixture.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 1.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (2, 1))]);
    let plan = vestra_core::plan::compile(
        vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            1,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        vestra_core::plan::CompileOptions::default(),
    )
    .expect("video fixture compiles");
    assert_eq!(plan.video_slot_count(), 1);
    let decoded = crate::DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(VideoFixtureFactory {
            frame: crate::VideoFrame {
                pts: 0,
                pixels: Arc::new(image::RgbaImage::from_fn(2, 1, |x, _| {
                    if x == 0 {
                        image::Rgba([255, 0, 0, 255])
                    } else {
                        image::Rgba([0, 0, 255, 255])
                    }
                })),
            },
        })),
    )
    .expect("video fixture decodes");
    let frame = vestra_core::plan::evaluate(&plan, &[vestra_core::plan::ScheduledItem(0)], 0)
        .expect("renderer fixture evaluates");
    let mut cpu_output = RgbaImage::new(1, 1);
    crate::render::CpuBackend::new(&plan, Arc::clone(&decoded))
        .render_frame(&frame, &mut cpu_output)
        .expect("CPU video frame renders");
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    let mut gpu_output = RgbaImage::new(1, 1);
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("GPU video frame renders");
    assert_eq!(
        cpu_output,
        RgbaImage::from_pixel(1, 1, image::Rgba([0, 0, 255, 255]))
    );
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
    assert!(
        difference.maximum_absolute_channel_error <= 2,
        "video crop: {difference:?}"
    );
}

#[test]
fn gpu_static_crops_match_cpu_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("canonical fixture validates");
    let canonical = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&canonical).expect("fixture decodes");
    let image_layer = canonical
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                vestra_core::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");

    for crop in [
        Crop {
            x: 0.13,
            y: 0.17,
            width: 0.61,
            height: 0.59,
        },
        Crop {
            x: 0.0,
            y: 0.11,
            width: 0.71,
            height: 0.73,
        },
        Crop {
            x: 0.29,
            y: 0.0,
            width: 0.71,
            height: 0.73,
        },
        Crop {
            x: 0.29,
            y: 0.27,
            width: 0.71,
            height: 0.73,
        },
        Crop {
            x: 0.13,
            y: 0.27,
            width: 0.61,
            height: 0.73,
        },
    ] {
        let mut plan = canonical.clone();
        let vestra_core::plan::CompiledVisualSource::Image {
            crop: track,
            cacheable_crop,
            ..
        } = &mut plan.layers[image_layer].source
        else {
            unreachable!()
        };
        *track = Track::new(crop);
        *cacheable_crop = true;
        let frame =
            vestra_core::plan::evaluate(&plan, &[vestra_core::plan::ScheduledItem(image_layer)], 0)
                .expect("renderer fixture evaluates");
        let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
            return;
        };
        let mut cpu_output = RgbaImage::new(frame.width, frame.height);
        let mut gpu_output = RgbaImage::new(frame.width, frame.height);
        cpu.render_frame(&frame, &mut cpu_output)
            .expect("CPU frame renders");
        gpu.render_frame(&frame, &mut gpu_output)
            .expect("GPU frame renders");
        let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
        assert!(
            difference.maximum_absolute_channel_error <= 2,
            "static crop {crop:?}: {difference:?}"
        );
    }
}

#[test]
fn gpu_animated_crop_matches_cpu_when_an_adapter_is_available() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("canonical fixture validates");
    let mut plan = compile(validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = crate::DecodedAssets::build(&plan).expect("fixture decodes");
    let image_layer = plan
        .layers
        .iter()
        .position(|layer| {
            matches!(
                layer.source,
                vestra_core::plan::CompiledVisualSource::Image { .. }
            )
        })
        .expect("fixture has image");
    let vestra_core::plan::CompiledVisualSource::Image {
        crop,
        cacheable_crop,
        ..
    } = &mut plan.layers[image_layer].source
    else {
        unreachable!()
    };
    *crop = Track {
        base_value: Crop {
            x: 0.08,
            y: 0.14,
            width: 0.78,
            height: 0.72,
        },
        keyframes: vec![Keyframe {
            time: 1_000_000_000,
            value: Crop {
                x: 0.19,
                y: 0.21,
                width: 0.67,
                height: 0.63,
            },
            interpolation: Interpolation::Linear,
        }],
    };
    *cacheable_crop = false;
    let frame = vestra_core::plan::evaluate(
        &plan,
        &[vestra_core::plan::ScheduledItem(image_layer)],
        500_000_000,
    )
    .expect("renderer fixture evaluates");
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let Some(mut gpu) = wgpu_backend_or_skip(&plan, Arc::clone(&decoded)) else {
        return;
    };
    let mut cpu_output = RgbaImage::new(frame.width, frame.height);
    let mut gpu_output = RgbaImage::new(frame.width, frame.height);
    cpu.render_frame(&frame, &mut cpu_output)
        .expect("CPU frame renders");
    gpu.render_frame(&frame, &mut gpu_output)
        .expect("GPU frame renders");
    let difference = compare_rgba(cpu_output.as_raw(), gpu_output.as_raw(), 2);
    assert!(
        difference.maximum_absolute_channel_error <= 2,
        "animated crop: {difference:?}"
    );
}
