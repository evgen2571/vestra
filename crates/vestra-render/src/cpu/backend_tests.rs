use super::*;
use crate::{
    plan::{
        ColourTransform, CompileOptions, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer,
        EvaluatedSource, ScheduledItem, TemporalDependency, compile, evaluate,
    },
    project::{ValidationOptions, load_and_validate},
};
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
        _asset: &crate::plan::VideoAsset,
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
}

impl crate::VideoDecoderSession for TimelineVideoSession {
    fn frame_at(&mut self, seconds: f64) -> Result<crate::VideoFrame, String> {
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
}

impl crate::VideoDecoderFactory for TimelineVideoFactory {
    fn open(
        &self,
        _asset: &crate::plan::VideoAsset,
        _cache_budget_bytes: u64,
    ) -> Result<Box<dyn crate::VideoDecoderSession>, String> {
        Ok(Box::new(TimelineVideoSession {
            requested_times: Arc::clone(&self.requested_times),
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
        asset: &crate::plan::VideoAsset,
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

#[test]
fn cpu_video_renderer_uses_layer_local_source_timing_end_to_end() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "fixture.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "fixture.mp4"}],
            "visual": {"clips": [{
                "id": "video-layer", "source": {"type": "video", "asset": "video"},
                "start": 0, "duration": 2, "layer": 0,
                "source_start": 0.25, "playback_rate": 0.5,
                "opacity": {"base_value": 1}
            }]}
        }"##,
    )
    .expect("video timing fixture parses");
    let asset_paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("fixture.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 2.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = crate::plan::compile(
        &crate::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &asset_paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            2,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        crate::plan::CompileOptions::default(),
    )
    .expect("video timing fixture compiles");
    let time = 1_000_000_000;
    let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0)], time);
    let requested_times = Arc::new(Mutex::new(Vec::new()));
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&requested_times),
        })),
    )
    .expect("video timing fixture decodes");
    let mut backend = CpuBackend::new(&plan, decoded);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("video timing fixture renders");

    assert_eq!(requested_times.lock().unwrap().last().copied(), Some(0.75));
    assert_eq!(output.get_pixel(0, 0), &image::Rgba([75, 0, 0, 255]));
    let first_pixels = output.clone();
    for time in [200_000_000, 700_000_000, 1_000_000_000] {
        let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0)], time);
        backend
            .render_frame(&frame, &mut output)
            .expect("random-access video frame renders");
    }
    assert_eq!(output, first_pixels);
}

#[test]
fn cpu_video_to_video_transition_advances_both_endpoints() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "video-transition.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [
                {"id": "red", "type": "video", "source": "red.mp4"},
                {"id": "blue", "type": "video", "source": "blue.mp4"}
            ],
            "visual": {
                "clips": [
                    {"id": "out", "source": {"type": "video", "asset": "red"},
                     "start": 0, "duration": 2, "layer": 0,
                     "source_start": 0, "playback_rate": 1,
                     "opacity": {"base_value": 1}},
                    {"id": "in", "source": {"type": "video", "asset": "blue"},
                     "start": 0, "duration": 2, "layer": 1,
                     "source_start": 1, "playback_rate": 2,
                     "opacity": {"base_value": 1}}
                ],
                "transitions": [{"id": "fade", "outgoing": "out", "incoming": "in",
                    "start": 0.5, "duration": 1.0,
                    "definition": {
                        "outgoing": {"opacity": {"keyframes": [
                            {"progress": 0, "value": 1, "interpolation": "linear"},
                            {"progress": 1, "value": 0, "interpolation": "linear"}
                        ]}},
                        "incoming": {"opacity": {"keyframes": [
                            {"progress": 0, "value": 0, "interpolation": "linear"},
                            {"progress": 1, "value": 1, "interpolation": "linear"}
                        ]}}
                    }}]
            }
        }"##,
    )
    .expect("Video transition fixture parses");
    let paths = std::collections::BTreeMap::from([
        ("red".to_owned(), std::path::PathBuf::from("red.mp4")),
        ("blue".to_owned(), std::path::PathBuf::from("blue.mp4")),
    ]);
    let durations =
        std::collections::BTreeMap::from([("red".to_owned(), 4.0), ("blue".to_owned(), 4.0)]);
    let dimensions =
        std::collections::BTreeMap::from([("red".to_owned(), (1, 1)), ("blue".to_owned(), (1, 1))]);
    let plan = crate::plan::compile(
        &crate::plan::PlanCompileInput::new(
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
        crate::plan::CompileOptions::default(),
    )
    .expect("Video transition fixture compiles");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(ColourVideoFactory {
            requests: Arc::clone(&requests),
        })),
    )
    .expect("Video transition fixture decodes");
    let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 1_000_000_000);
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("Video transition fixture renders");

    let requests = requests.lock().unwrap();
    assert!(
        requests
            .iter()
            .any(|(id, time)| id == "red" && *time == 1.0)
    );
    assert!(
        requests
            .iter()
            .any(|(id, time)| id == "blue" && *time == 3.0)
    );
    let pixel = output.get_pixel(0, 0);
    assert!(pixel[0] > 0 && pixel[2] > 0 && pixel[1] == 0);
}

#[test]
fn cpu_same_video_asset_can_render_two_source_times_in_one_frame() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
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
    let plan = crate::plan::compile(
        &crate::plan::PlanCompileInput::new(
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
        crate::plan::CompileOptions::default(),
    )
    .expect("same-asset Video fixture compiles");
    let single_worker_requests = Arc::new(Mutex::new(Vec::new()));
    let multi_worker_requests = Arc::new(Mutex::new(Vec::new()));
    let single_worker_decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&single_worker_requests),
        })),
    )
    .expect("same-asset Video fixture decodes");
    let multi_worker_decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&multi_worker_requests),
        })),
    )
    .expect("same-asset Video fixture decodes for multiple workers");
    let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0), ScheduledItem(1)], 500_000_000);
    let mut single_worker = CpuBackend::new_with_worker_count(&plan, single_worker_decoded, 1);
    let mut multi_worker = CpuBackend::new_with_worker_count(&plan, multi_worker_decoded, 2);
    let mut single_output = image::RgbaImage::new(1, 1);
    let mut multi_output = image::RgbaImage::new(1, 1);
    single_worker
        .render_frame(&frame, &mut single_output)
        .expect("same-asset Video fixture renders with one worker");
    multi_worker
        .render_frame(&frame, &mut multi_output)
        .expect("same-asset Video fixture renders with multiple workers");

    let single_requests = single_worker_requests.lock().unwrap();
    let multi_requests = multi_worker_requests.lock().unwrap();
    assert!(single_requests.contains(&0.5));
    assert!(single_requests.contains(&1.5));
    assert!(multi_requests.contains(&0.5));
    assert!(multi_requests.contains(&1.5));
    assert_eq!(single_output, multi_output);
    let pixel = single_output.get_pixel(0, 0);
    assert!(
        pixel[0] > 0 && pixel[2] > 0,
        "same-asset layers should both contribute, got {pixel:?}"
    );
}

#[test]
fn cpu_nested_video_renderer_uses_nested_local_time() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "nested-video.mp4", "width": 1, "height": 1,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [{"id": "video", "type": "video", "source": "video.mp4"}],
            "visual": {"clips": [{
                "id": "group", "source": {"type": "group", "clips": [{
                    "id": "child", "source": {"type": "video", "asset": "video"},
                    "start": 0.25, "duration": 1.0, "layer": 0,
                    "source_start": 0.1, "playback_rate": 2,
                    "opacity": {"base_value": 1}
                }]}, "start": 0.5, "duration": 2.0, "layer": 0,
                "opacity": {"base_value": 1}
            }]}
        }"##,
    )
    .expect("nested Video fixture parses");
    let paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("video.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 4.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = crate::plan::compile(
        &crate::plan::PlanCompileInput::new(
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
        crate::plan::CompileOptions::default(),
    )
    .expect("nested Video fixture compiles");
    let requested_times = Arc::new(Mutex::new(Vec::new()));
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(TimelineVideoFactory {
            requested_times: Arc::clone(&requested_times),
        })),
    )
    .expect("nested Video fixture decodes");
    let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0)], 1_000_000_000);
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("nested Video fixture renders");

    assert_eq!(requested_times.lock().unwrap().last().copied(), Some(0.6));
    assert_eq!(output.get_pixel(0, 0), &image::Rgba([60, 0, 0, 255]));
}

#[test]
fn cpu_video_renderer_samples_the_authored_crop_from_a_dynamic_frame() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "fixture.mp4", "width": 1, "height": 1,
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
    let asset_paths = std::collections::BTreeMap::from([(
        "video".to_owned(),
        std::path::PathBuf::from("fixture.mp4"),
    )]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 1.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (2, 1))]);
    let plan = crate::plan::compile(
        &crate::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &asset_paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            1,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        crate::plan::CompileOptions::default(),
    )
    .expect("video fixture compiles");
    let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0)], 0);
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(std::sync::Arc::new(VideoFixtureFactory {
            frame: crate::VideoFrame {
                pts: 0,
                pixels: std::sync::Arc::new(image::RgbaImage::from_fn(2, 1, |x, _| {
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
    let mut backend = CpuBackend::new(&plan, decoded);
    let mut output = image::RgbaImage::new(1, 1);
    backend
        .render_frame(&frame, &mut output)
        .expect("video fixture renders");
    assert_eq!(output.get_pixel(0, 0), &image::Rgba([0, 0, 255, 255]));
}

#[test]
fn cpu_mixed_static_and_video_sources_render_through_one_layer_pipeline() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 3,
            "output": {
                "path": "mixed.mp4", "width": 4, "height": 4,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false, "duration_mode": "automatic"
            },
            "assets": [
                {"id": "image", "type": "image", "source": "red.png"},
                {"id": "video", "type": "video", "source": "fixture.mp4"},
                {"id": "font", "type": "font", "source": "VestraTest-Regular.ttf"}
            ],
            "visual": {"clips": [
                {"id": "image", "source": {"type": "image", "asset": "image"},
                 "start": 0, "duration": 1, "layer": 0,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 1}},
                {"id": "shape", "source": {"type": "shape",
                 "geometry": {"type": "rectangle", "width": 2, "height": 2},
                 "fill": "#00ff00"}, "start": 0, "duration": 1, "layer": 1,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 1}},
                {"id": "text", "source": {"type": "text", "text": "A",
                 "font": "font", "font_size": 2, "fill": "#ffffff"},
                 "start": 0, "duration": 1, "layer": 2,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 1}},
                {"id": "video", "source": {"type": "video", "asset": "video"},
                 "start": 0, "duration": 1, "layer": 3,
                 "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
                  "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                  "scale": {"base_value": {"x": 1, "y": 1}},
                  "rotation_degrees": {"base_value": 0}},
                 "opacity": {"base_value": 0.5}}
            ]}
        }"##,
    )
    .expect("mixed source fixture parses");
    let image_path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets/red.png");
    let asset_paths = std::collections::BTreeMap::from([
        ("image".to_owned(), image_path),
        ("video".to_owned(), std::path::PathBuf::from("fixture.mp4")),
        (
            "font".to_owned(),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/assets/VestraTest-Regular.ttf"),
        ),
    ]);
    let durations = std::collections::BTreeMap::from([("video".to_owned(), 1.0)]);
    let dimensions = std::collections::BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = crate::plan::compile(
        &crate::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &asset_paths,
            &std::collections::BTreeMap::new(),
            1.0,
            (1, 1),
            1,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        crate::plan::CompileOptions::default(),
    )
    .expect("mixed source fixture compiles");
    let decoded = DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(VideoFixtureFactory {
            frame: crate::VideoFrame {
                pts: 0,
                pixels: Arc::new(image::RgbaImage::from_pixel(
                    1,
                    1,
                    image::Rgba([0, 0, 255, 255]),
                )),
            },
        })),
    )
    .expect("mixed source fixture decodes");
    let frame = crate::plan::evaluate(&plan, &[ScheduledItem(0)], 0);
    let mut backend = CpuBackend::new(&plan, decoded);
    let mut output = image::RgbaImage::new(4, 4);
    backend
        .render_frame(&frame, &mut output)
        .expect("mixed source fixture renders");

    assert!(output.pixels().any(|pixel| pixel[1] > 0));
    assert!(output.pixels().any(|pixel| pixel[2] > 0));
    assert!(output.pixels().any(|pixel| pixel[0] > 0));
}

fn static_frame() -> EvaluatedFrame {
    EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 255],
        width: 4,
        height: 4,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 7,
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
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    }
}

#[test]
fn aggregate_snapshots_sums_independent_cache_gauges_and_keeps_shared_decode_once() {
    let worker = |budget, current_entries, peak_entries, current_bytes, peak_bytes, hits| {
        let mut stats = PreparationStats {
            decoded_image_count: 9,
            decoded_source_bytes: 900,
            peak_decoded_bytes: 1_000,
            cache_budget_bytes: budget,
            cache_current_entries: current_entries,
            peak_cache_entries: peak_entries,
            cache_current_bytes: current_bytes,
            cache_peak_bytes: peak_bytes,
            bitmap_cache_hits: hits,
            bitmap_cache_requests: 10,
            ..PreparationStats::default()
        };
        stats.bitmap_cache_misses = 10 - hits;
        WorkerSnapshot {
            stats,
            timings: PreparationTimings::default(),
            hot_path_timings: CpuHotPathTimings::default(),
        }
    };

    let aggregate =
        aggregate_snapshots(&[worker(100, 2, 3, 40, 60, 3), worker(100, 1, 4, 20, 70, 5)]);

    assert_eq!(aggregate.cache_budget_bytes, 200);
    assert_eq!(aggregate.cache_current_entries, 3);
    assert_eq!(aggregate.peak_cache_entries, 7);
    assert_eq!(aggregate.cache_current_bytes, 60);
    assert_eq!(aggregate.cache_peak_bytes, 130);
    assert_eq!(aggregate.decoded_image_count, 9);
    assert_eq!(aggregate.decoded_source_bytes, 900);
    assert_eq!(aggregate.peak_decoded_bytes, 1_000);
    assert_eq!(aggregate.bitmap_cache_hit_rate, Some(0.4));
}

#[test]
fn aggregate_hot_path_timings_sums_worker_local_durations() {
    let first = CpuHotPathTimings {
        source_rasterization: Duration::from_millis(3),
        zoom_blur: Duration::from_millis(5),
        ..CpuHotPathTimings::default()
    };
    let second = CpuHotPathTimings {
        source_rasterization: Duration::from_millis(7),
        zoom_blur: Duration::from_millis(11),
        global_post_effect: Duration::from_millis(13),
        ..CpuHotPathTimings::default()
    };
    let snapshots = [
        WorkerSnapshot {
            stats: PreparationStats::default(),
            timings: PreparationTimings::default(),
            hot_path_timings: first,
        },
        WorkerSnapshot {
            stats: PreparationStats::default(),
            timings: PreparationTimings::default(),
            hot_path_timings: second,
        },
    ];

    let aggregate = aggregate_hot_path_timings(&snapshots);
    assert_eq!(aggregate.source_rasterization, Duration::from_millis(10));
    assert_eq!(aggregate.zoom_blur, Duration::from_millis(16));
    assert_eq!(aggregate.global_post_effect, Duration::from_millis(13));
    assert!(aggregate.layer_composition.is_zero());
}

#[test]
fn automatic_worker_policy_reserves_cpu_and_applies_frame_memory_limit() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");

    plan.canvas.width = 1_920;
    plan.canvas.height = 1_080;
    assert_eq!(automatic_worker_count(&plan, 1), 1);
    assert_eq!(automatic_worker_count(&plan, 2), 1);
    assert_eq!(automatic_worker_count(&plan, 4), 3);
    assert_eq!(automatic_worker_count(&plan, 64), 8);

    plan.canvas.width = 2_560;
    plan.canvas.height = 1_440;
    assert_eq!(automatic_worker_count(&plan, 64), 6);
    plan.canvas.width = 3_840;
    plan.canvas.height = 2_160;
    assert_eq!(automatic_worker_count(&plan, 64), 2);
}

#[test]
fn automatic_worker_policy_is_never_zero_and_handles_overflow() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = u32::MAX;
    plan.canvas.height = u32::MAX;
    assert_eq!(automatic_worker_count(&plan, 0), 1);
    assert_eq!(automatic_worker_count(&plan, usize::MAX), 1);
}

#[test]
fn worker_budget_partitions_preserve_total_and_differ_by_at_most_one() {
    for (total, workers) in [(0, 1), (1, 4), (256 * 1024 * 1024, 2), (257, 4), (257, 8)] {
        let parts = (0..workers)
            .map(|worker_id| worker_budget(total, workers, worker_id))
            .collect::<Vec<_>>();
        assert_eq!(parts.iter().sum::<u64>(), total);
        let min = parts.iter().min().copied().unwrap_or(0);
        let max = parts.iter().max().copied().unwrap_or(0);
        assert!(max - min <= 1);
    }
}

#[test]
fn multiple_workers_partition_both_cache_classes_without_multiplying_capacity() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.limits.maximum_cache_bytes = 257;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 4);

    assert_eq!(backend.worker_cache_budgets.len(), 4);
    assert_eq!(
        backend
            .worker_cache_budgets
            .iter()
            .map(|budget| budget.crop_cache_budget_bytes)
            .sum::<u64>(),
        257
    );
    assert_eq!(
        backend
            .worker_cache_budgets
            .iter()
            .map(|budget| budget.static_cache_budget_bytes)
            .sum::<u64>(),
        257
    );
    assert_eq!(backend.stats().cache_budget_bytes, 257);
}

#[test]
fn completion_accounting_sums_render_work_and_reset_clears_it() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    backend.worker_busy.fill(true);
    backend
        .consume_completion(WorkerCompletion::Frame {
            worker_id: 0,
            frame: CompletedFrame {
                frame_number: 0,
                rgba: Vec::new(),
            },
            render_duration: Duration::from_millis(7),
        })
        .expect("first synthetic completion");
    backend
        .consume_completion(WorkerCompletion::Frame {
            worker_id: 1,
            frame: CompletedFrame {
                frame_number: 1,
                rgba: Vec::new(),
            },
            render_duration: Duration::from_millis(11),
        })
        .expect("second synthetic completion");
    assert_eq!(
        backend.staged_metrics().frame_render_work_duration,
        Duration::from_millis(18)
    );
    backend.reset_operation_metrics();
    assert_eq!(
        backend.staged_metrics().frame_render_work_duration,
        Duration::ZERO
    );
}

#[test]
fn worker_panic_preserves_the_failing_frame_number() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    backend
        .submit_panicking_frame(10, &static_frame())
        .expect("panic job submission");
    let error = backend
        .poll_completed(PollMode::WaitForOne)
        .expect_err("worker panic reaches backend");
    assert_eq!(error.code, "CPU-WORKER-PANIC");
    assert_eq!(backend.failed_frame_number(), Some(10));
    backend.abort();
}

#[test]
fn reuses_complete_static_layer_surfaces_without_mutating_them() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 4;
    plan.canvas.height = 4;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    assert_eq!(backend.capacity(), 1);
    let mut frame = static_frame();
    frame.layers[0].effects = vec![
        EvaluatedEffect::Brightness { amount: 0.1 },
        EvaluatedEffect::Tint {
            colour: [0, 0, 255, 255],
            amount: 0.5,
        },
    ];
    frame.layers[0].colour_transform =
        ColourTransform::from_effects(frame.layers[0].effects.clone());

    backend.submit_frame(0, &frame).expect("first submission");
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll")
        .expect("first completion");
    backend.submit_frame(1, &frame).expect("second submission");
    let second = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll")
        .expect("second completion");

    assert_eq!(first.rgba, second.rgba);
    let stats = backend.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_cache_hits, 1);
    assert_eq!(stats.static_cache_entries, 1);
    assert_eq!(stats.static_cache_population_renders, 1);
    assert_eq!(stats.static_layers_rendered, 1);
    assert_eq!(stats.cpu_scratch_allocations, 4);
}

#[test]
fn static_layer_renders_once_across_one_hundred_frames() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut frame = static_frame();
    frame.layers[0].effects = vec![
        EvaluatedEffect::Brightness { amount: 0.1 },
        EvaluatedEffect::Tint {
            colour: [0, 0, 255, 255],
            amount: 0.5,
        },
    ];
    frame.layers[0].colour_transform =
        ColourTransform::from_effects(frame.layers[0].effects.clone());

    for frame_number in 0..100 {
        backend
            .submit_frame(frame_number, &frame)
            .expect("frame submits");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion");
    }

    let stats = backend.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_cache_hits, 99);
    assert_eq!(stats.static_cache_population_renders, 1);
    assert_eq!(stats.static_layers_rendered, 1);
}

#[test]
fn static_layer_activity_does_not_affect_its_cache_identity() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut inactive = static_frame();
    inactive.layers.clear();
    let active = static_frame();

    for (number, frame) in [
        (10, &inactive),
        (30, &active),
        (45, &active),
        (100, &inactive),
    ] {
        backend.submit_frame(number, frame).expect("frame submits");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion");
    }

    let stats = backend.stats();
    assert_eq!(stats.static_cache_misses, 1);
    assert_eq!(stats.static_cache_hits, 1);
    assert_eq!(stats.static_cache_entries, 1);
}

#[test]
fn static_cache_matches_the_dynamic_reference_path() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut frame = static_frame();
    frame.layers[0].effects = vec![
        EvaluatedEffect::Brightness { amount: 0.1 },
        EvaluatedEffect::Tint {
            colour: [0, 0, 255, 255],
            amount: 0.5,
        },
    ];
    frame.layers[0].colour_transform =
        ColourTransform::from_effects(frame.layers[0].effects.clone());
    let mut cached = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    let mut reference = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut reference_frame = frame.clone();
    reference_frame.layers[0].content_dependency = TemporalDependency::Dynamic;

    cached.submit_frame(0, &frame).expect("cached submission");
    reference
        .submit_frame(0, &reference_frame)
        .expect("reference submission");
    let cached = cached
        .poll_completed(PollMode::WaitForOne)
        .expect("cached poll")
        .expect("cached completion");
    let reference = reference
        .poll_completed(PollMode::WaitForOne)
        .expect("reference poll")
        .expect("reference completion");

    assert_eq!(cached.rgba, reference.rgba);
}

#[test]
fn dynamic_effect_scratch_allocations_stabilize_after_warmup() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 4;
    plan.canvas.height = 4;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut frame = static_frame();
    frame.layers[0].content_dependency = TemporalDependency::Dynamic;
    frame.layers[0].effects = vec![EvaluatedEffect::GaussianBlur { radius: 1.0 }];

    for frame_number in 0..100 {
        backend
            .submit_frame(frame_number, &frame)
            .expect("frame submits");
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion");
    }

    let stats = backend.stats();
    assert_eq!(stats.cpu_full_frame_allocations, 100);
    assert_eq!(stats.cpu_scratch_allocations, 3);
    assert_eq!(stats.cpu_scratch_reuses, 200);
    assert_eq!(stats.cpu_scratch_buffers_retained, 3);
    assert_eq!(stats.cpu_scratch_bytes_retained, 192);
}

#[test]
fn dynamic_layers_bypass_the_whole_layer_cache() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut frame = static_frame();
    frame.layers[0].content_dependency = TemporalDependency::Dynamic;

    backend.submit_frame(0, &frame).expect("first submission");
    backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll");
    backend.submit_frame(1, &frame).expect("second submission");
    backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll");

    let stats = backend.stats();
    assert_eq!(stats.static_cache_hits, 0);
    assert_eq!(stats.static_cache_misses, 0);
    assert_eq!(stats.static_cache_entries, 0);
    assert_eq!(stats.static_layers_rendered, 0);
}

#[test]
fn over_budget_static_layers_render_without_retention() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.limits.maximum_cache_bytes = 1;
    plan.canvas.width = 4;
    plan.canvas.height = 4;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    let mut reference = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let frame = static_frame();
    let mut reference_frame = frame.clone();
    reference_frame.layers[0].content_dependency = TemporalDependency::Dynamic;

    let mut outputs = Vec::new();
    for frame_number in 0..100 {
        backend
            .submit_frame(frame_number, &frame)
            .expect("frame submits");
        let output = backend
            .poll_completed(PollMode::WaitForOne)
            .expect("frame poll")
            .expect("frame completion")
            .rgba;
        reference
            .submit_frame(frame_number, &reference_frame)
            .expect("reference frame submits");
        let reference = reference
            .poll_completed(PollMode::WaitForOne)
            .expect("reference frame poll")
            .expect("reference frame completion");
        assert_eq!(output, reference.rgba);
        outputs.push(output);
    }

    assert!(outputs.windows(2).all(|frames| frames[0] == frames[1]));
    let stats = backend.stats();
    assert_eq!(stats.static_cache_entries, 0);
    assert_eq!(stats.static_cache_hits, 0);
    assert_eq!(stats.static_cache_misses, 100);
    assert_eq!(stats.static_cache_budget_bypasses, 100);
    assert_eq!(stats.static_layers_rendered, 100);
    assert_eq!(stats.cpu_scratch_allocations, 3);
}

#[test]
fn distinct_static_layers_do_not_alias_or_mutate_under_dynamic_composition() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    let mut base = static_frame();
    let mut second = base.layers[0].clone();
    second.compiled_layer_index = 8;
    second.source = EvaluatedSource::SolidColor {
        colour: [180, 20, 40, 255],
    };
    base.layers.push(second);

    backend.submit_frame(0, &base).expect("base submission");
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("base poll")
        .expect("base completion");

    let mut changed = base.clone();
    let mut dynamic = changed.layers[0].clone();
    dynamic.compiled_layer_index = 9;
    dynamic.content_dependency = TemporalDependency::Dynamic;
    dynamic.source = EvaluatedSource::SolidColor {
        colour: [5, 220, 30, 255],
    };
    dynamic.opacity = 0.4;
    changed.layers.push(dynamic);
    backend
        .submit_frame(1, &changed)
        .expect("dynamic submission");
    backend
        .poll_completed(PollMode::WaitForOne)
        .expect("dynamic poll")
        .expect("dynamic completion");

    backend.submit_frame(2, &base).expect("restored submission");
    let restored = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("restored poll")
        .expect("restored completion");

    assert_eq!(first.rgba, restored.rgba);
    let stats = backend.stats();
    assert_eq!(stats.static_cache_entries, 2);
    assert_eq!(stats.static_cache_misses, 2);
    assert_eq!(stats.static_cache_hits, 4);
}

#[test]
fn completed_pixels_remain_owned_after_later_submission_and_polling() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame = evaluate(&plan, &[ScheduledItem(0)], 0);
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);

    backend.submit_frame(3, &frame).expect("first submission");
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll")
        .expect("first completion");
    let first_pixels = first.rgba.clone();

    backend.submit_frame(4, &frame).expect("second submission");
    let second = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll")
        .expect("second completion");

    assert_eq!(first.frame_number, 3);
    assert_eq!(second.frame_number, 4);
    assert_eq!(first.rgba, first_pixels);
}

#[test]
fn explicit_workers_accept_multiple_frames_and_reuse_slots() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame = evaluate(&plan, &[ScheduledItem(0)], 0);
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    assert_eq!(backend.capacity(), 2);
    backend.submit_frame(0, &frame).expect("first submission");
    backend.submit_frame(1, &frame).expect("second submission");
    assert_eq!(backend.in_flight(), 2);
    let first = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("first poll")
        .expect("completion");
    let second = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("second poll")
        .expect("completion");
    assert_eq!(backend.in_flight(), 0);
    assert_ne!(first.frame_number, second.frame_number);
    backend
        .submit_frame(2, &frame)
        .expect("reused worker submission");
    assert_eq!(
        backend
            .poll_completed(PollMode::WaitForOne)
            .expect("reuse poll")
            .expect("reuse completion")
            .frame_number,
        2
    );
    backend.flush().expect("flush");
    backend.verify_idle().expect("idle after flush");
}

#[test]
fn one_and_two_workers_produce_identical_frame_pixels() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frames: Vec<_> = (0..4)
        .map(|number| evaluate(&plan, &[ScheduledItem(0)], number))
        .collect();
    let mut single = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    let mut multi = CpuBackend::new_with_worker_count(&plan, decoded, 2);
    let mut single_pixels = Vec::new();
    for (number, frame) in frames.iter().enumerate() {
        single
            .submit_frame(number as u64, frame)
            .expect("single submission");
        single_pixels.push(
            single
                .poll_completed(PollMode::WaitForOne)
                .expect("single poll")
                .expect("single completion"),
        );
    }
    let mut multi_pixels = Vec::new();
    for (number, frame) in frames.iter().enumerate() {
        if multi.in_flight() == multi.capacity() {
            multi_pixels.push(
                multi
                    .poll_completed(PollMode::WaitForOne)
                    .expect("multi poll")
                    .expect("multi completion"),
            );
        }
        multi
            .submit_frame(number as u64, frame)
            .expect("multi submission");
    }
    while multi.in_flight() > 0 {
        multi_pixels.push(
            multi
                .poll_completed(PollMode::WaitForOne)
                .expect("multi poll")
                .expect("multi completion"),
        );
    }
    single_pixels.sort_by_key(|frame| frame.frame_number);
    multi_pixels.sort_by_key(|frame| frame.frame_number);
    assert_eq!(single_pixels, multi_pixels);
}

#[test]
fn one_and_four_workers_produce_identical_frame_pixels() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frames: Vec<_> = (0..8)
        .map(|number| evaluate(&plan, &[ScheduledItem(0)], number))
        .collect();
    let collect = |worker_count, decoded: Arc<DecodedAssets>| {
        let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, worker_count);
        let mut completed = Vec::new();
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                completed.push(
                    backend
                        .poll_completed(PollMode::WaitForOne)
                        .expect("poll")
                        .expect("completion"),
                );
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("submission");
        }
        while backend.in_flight() > 0 {
            completed.push(
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("drain poll")
                    .expect("drain completion"),
            );
        }
        completed.sort_by_key(|frame| frame.frame_number);
        completed
    };
    let single = collect(1, Arc::clone(&decoded));
    let four = collect(4, decoded);
    assert_eq!(single, four);
}

#[test]
fn spectrum2d_one_two_and_four_workers_are_byte_identical() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frames: Vec<_> = (0..6)
        .map(|frame_number| {
            let mut frame = evaluate(&plan, &[ScheduledItem(0)], frame_number);
            frame.layers[0].source = EvaluatedSource::Spectrum2D {
                bands: vec![
                    (frame_number as f32 * 0.17).sin().abs(),
                    0.2 + frame_number as f32 * 0.03,
                    0.7 - frame_number as f32 * 0.04,
                    1.0,
                ],
                x: 0.0,
                y: 0.1,
                width: 1.0,
                height: 0.8,
                bar_gap_ratio: 0.08,
                min_bar_height_ratio: 0.10,
                layout: crate::project::Spectrum2DLayout::Radial(
                    crate::project::Spectrum2DRadialLayout {
                        inner_radius_ratio: 0.35,
                        start_angle_degrees: 450.0,
                        sweep_angle_degrees: 270.0,
                        direction: crate::project::Spectrum2DRadialDirection::Outward,
                        band_mapping: crate::project::Spectrum2DBandMapping::Reverse,
                    },
                ),
                gradient: Some((
                    crate::project::Spectrum2DGradientDirection::AcrossBands,
                    [20, 90, 180, 80],
                    [220, 40, 240, 220],
                )),
                colour: [40, 90, 180, 160],
            };
            frame.layers[0].effects = vec![EvaluatedEffect::Bloom {
                threshold: 0.4,
                radius: 2.0,
                intensity: 0.5,
            }];
            frame
        })
        .collect();
    let collect = |worker_count, decoded: Arc<DecodedAssets>| {
        let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, worker_count);
        let mut completed = Vec::new();
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                completed.push(
                    backend
                        .poll_completed(PollMode::WaitForOne)
                        .expect("poll")
                        .expect("completion"),
                );
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("submission");
        }
        while backend.in_flight() > 0 {
            completed.push(
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("drain poll")
                    .expect("drain completion"),
            );
        }
        completed.sort_by_key(|frame| frame.frame_number);
        completed
    };
    let one = collect(1, Arc::clone(&decoded));
    let two = collect(2, Arc::clone(&decoded));
    let four = collect(4, decoded);
    assert_eq!(one, two);
    assert_eq!(one, four);
}

#[test]
fn spectrum2d_uses_normal_opacity_and_bloom_pipeline() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let spectrum = EvaluatedSource::Spectrum2D {
        bands: vec![1.0],
        x: 0.0,
        y: 0.0,
        width: 0.25,
        height: 1.0,
        bar_gap_ratio: 0.0,
        min_bar_height_ratio: 0.0,
        layout: crate::project::Spectrum2DLayout::default(),
        gradient: None,
        colour: [255, 255, 255, 255],
    };
    let frame = EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 0],
        width: 8,
        height: 8,
        layers: vec![EvaluatedLayer {
            compiled_layer_index: 99,
            content_dependency: TemporalDependency::Dynamic,
            transform: crate::animation::Transform2D::identity(
                crate::domain::Point { x: 0.5, y: 0.5 },
                crate::domain::Point { x: 0.5, y: 0.5 },
            ),
            source: spectrum,
            opacity: 1.0,
            effects: Vec::new(),
            colour_transform: ColourTransform::default(),
            blend_mode: crate::project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    };
    let mut backend = CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), 1);
    backend
        .submit_frame(0, &frame)
        .expect("spectrum submission");
    let without_bloom = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("spectrum poll")
        .expect("spectrum completion");
    let mut bloom_frame = frame.clone();
    bloom_frame.layers[0].effects = vec![EvaluatedEffect::Bloom {
        threshold: 0.1,
        radius: 2.0,
        intensity: 1.0,
    }];
    backend
        .submit_frame(1, &bloom_frame)
        .expect("bloom submission");
    let with_bloom = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("bloom poll")
        .expect("bloom completion");
    assert_eq!(&without_bloom.rgba[2 * 4..3 * 4], &[0, 0, 0, 0]);
    assert!(with_bloom.rgba[2 * 4] > 0);
    assert!(with_bloom.rgba[2 * 4 + 3] > 0);
    assert!(with_bloom.rgba[0..4].iter().any(|value| *value > 0));
}

#[test]
#[ignore = "manual release CPU Spectrum2D benchmark"]
fn spectrum2d_cpu_benchmark() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 1_920;
    plan.canvas.height = 1_080;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let available = thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);

    let automatic = automatic_worker_count(&plan, available);
    let cases = [
        (
            "linear_24",
            24,
            1,
            crate::project::Spectrum2DLayout::default(),
            None,
            false,
        ),
        (
            "linear_24_auto",
            24,
            automatic,
            crate::project::Spectrum2DLayout::default(),
            None,
            false,
        ),
        (
            "center_out_24_auto",
            24,
            automatic,
            crate::project::Spectrum2DLayout::Linear(crate::project::Spectrum2DLinearLayout {
                anchor: crate::project::Spectrum2DLinearAnchor::Bottom,
                band_mapping: crate::project::Spectrum2DBandMapping::CenterOut,
            }),
            None,
            false,
        ),
        (
            "radial_24",
            24,
            1,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "radial_24_auto",
            24,
            automatic,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "radial_48",
            48,
            1,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "radial_48_auto",
            48,
            automatic,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            None,
            false,
        ),
        (
            "neon_circle_48_auto",
            48,
            automatic,
            crate::project::Spectrum2DLayout::Radial(crate::project::Spectrum2DRadialLayout {
                inner_radius_ratio: 0.55,
                start_angle_degrees: 0.0,
                sweep_angle_degrees: 360.0,
                direction: crate::project::Spectrum2DRadialDirection::Outward,
                band_mapping: crate::project::Spectrum2DBandMapping::Forward,
            }),
            Some((
                crate::project::Spectrum2DGradientDirection::AcrossBands,
                [0, 255, 255, 255],
                [255, 0, 255, 255],
            )),
            true,
        ),
    ];

    for (name, band_count, worker_count, layout, gradient, neon_circle) in cases {
        let frames: Vec<_> = (0..24)
            .map(|frame_number| {
                let mut frame = evaluate(&plan, &[ScheduledItem(0)], frame_number);
                if let EvaluatedSource::Image { .. } = frame.layers[0].source {
                    frame.layers[0].source = EvaluatedSource::Spectrum2D {
                        bands: (0..band_count)
                            .map(|band| {
                                (((band + frame_number as usize) % band_count) as f32
                                    / band_count as f32)
                                    .max(0.05)
                            })
                            .collect(),
                        x: if neon_circle { 0.20 } else { 0.0 },
                        y: if neon_circle { 0.20 } else { 0.1 },
                        width: if neon_circle { 0.60 } else { 1.0 },
                        height: if neon_circle { 0.60 } else { 0.8 },
                        bar_gap_ratio: if neon_circle { 0.12 } else { 0.1 },
                        min_bar_height_ratio: if neon_circle { 0.035 } else { 0.0 },
                        layout: layout.clone(),
                        gradient,
                        colour: [255, 255, 255, 255],
                    };
                }
                if neon_circle {
                    frame.layers[0].effects = vec![
                        EvaluatedEffect::Glow {
                            threshold: 0.35,
                            radius: 3.0,
                            intensity: 0.85,
                            colour: [255, 255, 255, 255],
                        },
                        EvaluatedEffect::Bloom {
                            threshold: 0.55,
                            radius: 4.0,
                            intensity: 0.65,
                        },
                    ];
                }
                frame
            })
            .collect();
        let mut backend =
            CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), worker_count);
        let started = Instant::now();
        for (frame_number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("benchmark poll")
                    .expect("benchmark completion");
            }
            backend
                .submit_frame(frame_number as u64, frame)
                .expect("benchmark submission");
        }
        while backend.in_flight() > 0 {
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("benchmark drain poll")
                .expect("benchmark drain completion");
        }
        let elapsed = started.elapsed();
        println!(
            "spectrum2d_cpu case={} workers={} resolution={}x{} analysis_bands={} displayed_bars={} frames={} wall_ms={:.3} effective_fps={:.2} effect_bundle={} gradient={} release=true renderer_only=true",
            name,
            worker_count,
            plan.canvas.width,
            plan.canvas.height,
            band_count,
            if matches!(layout, crate::project::Spectrum2DLayout::Linear(value) if matches!(value.band_mapping, crate::project::Spectrum2DBandMapping::CenterOut))
            {
                band_count * 2
            } else {
                band_count
            },
            frames.len(),
            elapsed.as_secs_f64() * 1_000.0,
            frames.len() as f64 / elapsed.as_secs_f64(),
            if neon_circle { "glow+bloom" } else { "none" },
            if gradient.is_some() {
                "enabled"
            } else {
                "solid"
            },
        );
    }
}

#[test]
#[ignore = "manual release CPU scaling benchmark"]
fn cpu_parallel_scaling_benchmark() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let mut plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    plan.canvas.width = 1_920;
    plan.canvas.height = 1_080;
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame_count = plan.frame_count.min(60);
    let frames: Vec<_> = (0..frame_count)
        .map(|number| evaluate(&plan, &[ScheduledItem(0)], u128::from(number)))
        .collect();
    let automatic = automatic_worker_count(
        &plan,
        thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get),
    );

    for (label, worker_count) in [("1", 1), ("2", 2), ("4", 4), ("auto", automatic)] {
        let mut backend = if label == "auto" {
            CpuBackend::new(&plan, Arc::clone(&decoded))
        } else {
            CpuBackend::new_with_worker_count(&plan, Arc::clone(&decoded), worker_count)
        };
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("warmup poll")
                    .expect("warmup completion");
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("warmup submission");
        }
        while backend.in_flight() > 0 {
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("warmup drain poll")
                .expect("warmup drain completion");
        }
        backend.reset_operation_metrics();
        let started = Instant::now();
        for (number, frame) in frames.iter().enumerate() {
            if backend.in_flight() == backend.capacity() {
                backend
                    .poll_completed(PollMode::WaitForOne)
                    .expect("measured poll")
                    .expect("measured completion");
            }
            backend
                .submit_frame(number as u64, frame)
                .expect("measured submission");
        }
        while backend.in_flight() > 0 {
            backend
                .poll_completed(PollMode::WaitForOne)
                .expect("measured drain poll")
                .expect("measured drain completion");
        }
        let elapsed = started.elapsed();
        let stats = backend.stats();
        let staged = backend.staged_metrics();
        let effective_fps = frame_count as f64 / elapsed.as_secs_f64();
        println!(
            "cpu_parallel_scaling workers={label} actual_workers={} resolution={}x{} frames={} wall_ms={:.3} effective_fps={effective_fps:.2} aggregate_frame_render_ms={} peak_in_flight={} cache_current_bytes={} cache_peak_bytes={} scratch_retained_bytes={}",
            backend.capacity(),
            plan.canvas.width,
            plan.canvas.height,
            frame_count,
            elapsed.as_secs_f64() * 1_000.0,
            staged.frame_render_work_duration.as_secs_f64() * 1_000.0,
            staged.peak_frames_in_flight,
            stats.cache_current_bytes,
            stats.cache_peak_bytes,
            stats.cpu_scratch_bytes_retained,
        );
    }
}

#[test]
fn cancellation_does_not_block_on_cpu_completion() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("fixture validates");
    let plan = compile(&validated, CompileOptions::default()).expect("fixture compiles");
    let decoded = DecodedAssets::build(&plan).expect("fixture images decode");
    let frame = evaluate(&plan, &[ScheduledItem(0)], 0);
    let mut backend = CpuBackend::new_with_worker_count(&plan, decoded, 1);
    backend.submit_frame(0, &frame).expect("submission");
    let cancelled = AtomicBool::new(true);
    assert!(
        backend
            .poll_completed_cancellable(PollMode::WaitForOne, &cancelled)
            .expect("cancelled poll")
            .is_none()
    );
    backend.abort();
    backend.abort();
    assert!(backend.verify_idle().is_err());
}
