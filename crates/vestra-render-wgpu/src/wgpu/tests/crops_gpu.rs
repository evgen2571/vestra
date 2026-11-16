//! WGPU-only unused-video preparation, source reads and upload accounting.
use super::{
    gpu::wgpu_backend_or_skip,
    video_pool_tests::{CursorFactory, CursorTrace},
};
use image::RgbaImage;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct CountingVideoFactory {
    opens: Arc<AtomicUsize>,
    frame_requests: Arc<AtomicUsize>,
}

struct CountingVideoSession {
    frame: crate::VideoFrame,
    frame_requests: Arc<AtomicUsize>,
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

#[test]
fn gpu_video_preparation_ignores_unused_assets_when_an_adapter_is_available() {
    let project = crate::project::Project::from_json(
        r##"{
            "schema_version": 1,
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
            }, {
                "id": "consumer", "source": {"type": "solid_color", "colour": "#00ff00"},
                "start": 0, "duration": 1, "layer": 1,
                "opacity": {"base_value": 1},
                "matte": {"source_layer": "video-layer", "mode": "alpha", "invert": false}
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

    let mut hidden = frame.clone();
    hidden.layers[0].visible = false;
    backend
        .render_frame(&hidden, &mut output)
        .expect("hidden video renders without a source read");
    assert_eq!(output.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(
        frame_requests.load(Ordering::Relaxed),
        3,
        "a hidden video without consumers must not request another frame"
    );

    let mut matte_frame = vestra_core::plan::evaluate(
        &plan,
        &[
            vestra_core::plan::ScheduledItem(0),
            vestra_core::plan::ScheduledItem(1),
        ],
        0,
    )
    .expect("video matte evaluates");
    matte_frame.layers[0].visible = false;
    backend
        .render_frame(&matte_frame, &mut output)
        .expect("hidden video still supplies a visible matte consumer");
    assert_eq!(frame_requests.load(Ordering::Relaxed), 4);
    assert_eq!(output.get_pixel(0, 0).0, [0, 255, 0, 255]);

    matte_frame.layers[0].visible = true;
    backend
        .render_frame(&matte_frame, &mut output)
        .expect("visible video and matte consumer share one source read");
    assert_eq!(frame_requests.load(Ordering::Relaxed), 5);
    matte_frame.layers[0].visible = false;

    let video_source = matte_frame.layers[0].source.clone();
    let mask_transform = matte_frame.layers[0].transform;
    matte_frame.layers[1].matte = None;
    matte_frame.layers[1]
        .masks
        .push(vestra_core::plan::EvaluatedMask {
            owner_transform: None,
            input: vestra_core::plan::EvaluatedMaskInput::Source {
                source: Box::new(video_source),
                mode: crate::project::MaskCoverageMode::Alpha,
            },
            operation: crate::project::MaskOperation::Intersect,
            invert: false,
            strength: 1.0,
            feather: 0.0,
            transform: mask_transform,
        });
    backend
        .render_frame(&matte_frame, &mut output)
        .expect("owned video mask requests its source");
    assert_eq!(frame_requests.load(Ordering::Relaxed), 6);
    assert_eq!(output.get_pixel(0, 0).0, [0, 255, 0, 255]);
    matte_frame.layers[1].visible = false;
    backend
        .render_frame(&matte_frame, &mut output)
        .expect("hidden mask owner needs no video read");
    assert_eq!(frame_requests.load(Ordering::Relaxed), 6);
    assert_eq!(output.get_pixel(0, 0).0, [0, 0, 0, 0]);
}

#[test]
fn gpu_concurrent_video_cursors_follow_placements_and_retire_when_inactive() {
    use crate::render::RenderBackend;
    use std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    };
    use vestra_core::plan::{PlanCompileInput, ScheduledItem, evaluate};

    let project = crate::project::Project::from_json(&serde_json::json!({
        "schema_version": 1,
        "output": {"path": "cursors.mp4", "width": 3, "height": 1,
            "frame_rate": "4/1", "background": "#00000000", "quality": "preview",
            "audio": false, "duration_mode": "automatic"},
        "assets": [{"id": "video", "type": "video", "source": "video.mp4"}],
        "visual": {"clips": (0..3).map(|index| serde_json::json!({
            "id": format!("clip-{index}"),
            "source": {"type": "video", "asset": "video"},
            "start": 0, "duration": 2, "layer": index,
            "source_start": if index == 1 {2.0} else {0.0},
            "playback_rate": if index == 1 {0.5} else {1.0},
            "sizing": {"mode": "original"},
            "transform": {"position": {"base_value": {"x": (index as f64 + 0.5) / 3.0, "y": 0.5}},
                "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
                "scale": {"base_value": {"x": 1, "y": 1}},
                "rotation_degrees": {"base_value": 0}},
            "opacity": {"base_value": 1}
        })).collect::<Vec<_>>()}
    }).to_string()).expect("cursor project parses");
    let paths = BTreeMap::from([("video".to_owned(), PathBuf::from("video.mp4"))]);
    let durations = BTreeMap::from([("video".to_owned(), 4.0)]);
    let dimensions = BTreeMap::from([("video".to_owned(), (1, 1))]);
    let plan = vestra_core::plan::compile(
        PlanCompileInput::new(
            &project,
            Default::default(),
            Path::new("."),
            &paths,
            &BTreeMap::new(),
            2.0,
            (3, 1),
            8,
            &[],
        )
        .with_video_durations(&durations)
        .with_video_dimensions(&dimensions),
        Default::default(),
    )
    .expect("cursor project compiles");
    let trace = Arc::new(std::sync::Mutex::new(CursorTrace::default()));
    let decoded = crate::DecodedAssets::build_with_video_factory(
        &plan,
        Some(Arc::new(CursorFactory(Arc::clone(&trace)))),
    )
    .expect("cursor assets prepare");
    let Some(mut backend) = wgpu_backend_or_skip(&plan, decoded) else {
        return;
    };
    let mut output = RgbaImage::new(3, 1);
    for time in [0, 250_000_000, 500_000_000] {
        let mut frame = evaluate(
            &plan,
            &[ScheduledItem(0), ScheduledItem(1), ScheduledItem(2)],
            time,
        )
        .expect("cursor frame evaluates");
        if time == 250_000_000 {
            frame.layers.reverse();
        }
        backend
            .render_frame(&frame, &mut output)
            .expect("cursor frame renders");
        let seconds = time as f64 / 1e9;
        assert_eq!(
            output.get_pixel(0, 0).0,
            [(seconds * 40.0).round() as u8, 0, 0, 255]
        );
        assert_eq!(
            output.get_pixel(1, 0).0,
            [((2.0 + seconds * 0.5) * 40.0).round() as u8, 0, 0, 255]
        );
        assert_eq!(output.get_pixel(2, 0), output.get_pixel(0, 0));
    }
    assert_eq!(
        backend.stats().video_seek_count,
        0,
        "independent forward timelines must not seek backward"
    );
    assert_eq!(
        backend.stats().video_decoder_open_count,
        2,
        "equal source/time reads share work"
    );
    assert_eq!(
        backend.stats().video_frame_requests,
        7,
        "one preparation read plus two reads per frame"
    );
    assert_eq!(
        trace.lock().unwrap().budgets,
        vec![plan.limits.maximum_cache_bytes, 0]
    );

    let mut divergent = evaluate(
        &plan,
        &[ScheduledItem(0), ScheduledItem(1), ScheduledItem(2)],
        750_000_000,
    )
    .unwrap();
    if let vestra_core::plan::EvaluatedSource::Video { source_time, .. } =
        &mut divergent.layers[2].source
    {
        *source_time = 1.5;
    }
    backend.render_frame(&divergent, &mut output).unwrap();
    assert_eq!(output.get_pixel(2, 0).0, [60, 0, 0, 255]);
    assert_eq!(backend.stats().video_decoder_open_count, 3);
    assert_eq!(
        trace.lock().unwrap().budgets,
        vec![plan.limits.maximum_cache_bytes, 0, 0]
    );
    assert_eq!(backend.stats().video_current_frame_bytes_retained, 12);
    assert_eq!(backend.stats().video_decoder_buffer_bytes_retained, 24);

    // Removing the primary placement must not reassign the surviving timeline.
    let frame = evaluate(&plan, &[ScheduledItem(1)], 1_000_000_000).unwrap();
    backend.render_frame(&frame, &mut output).unwrap();
    assert_eq!(backend.stats().video_seek_count, 0);
    let frame = evaluate(&plan, &[], 1_250_000_000).unwrap();
    backend.render_frame(&frame, &mut output).unwrap();
    assert_eq!(
        trace.lock().unwrap().live,
        1,
        "only the asset's primary cursor survives idle frames"
    );
    assert_eq!(backend.stats().video_decoder_session_count, 1);
    assert_eq!(
        backend.stats().video_frame_requests,
        11,
        "retired metrics remain cumulative"
    );

    assert_eq!(backend.stats().video_cache_bytes_retained, 4);
    assert_eq!(backend.stats().video_cache_bytes_peak, 4);
    assert_eq!(backend.stats().video_current_frame_bytes_retained, 4);
    assert_eq!(backend.stats().video_decoder_buffer_bytes_retained, 8);

    // Explicit seeks remain correct, including reactivation after retirement.
    for time in [1_250_000_000, 250_000_000] {
        let frame = evaluate(
            &plan,
            &[ScheduledItem(0), ScheduledItem(1), ScheduledItem(2)],
            time,
        )
        .unwrap();
        backend.render_frame(&frame, &mut output).unwrap();
        assert_eq!(
            output.get_pixel(0, 0).0[0],
            (time as f64 / 1e9 * 40.0).round() as u8
        );
        assert_eq!(
            output.get_pixel(1, 0).0[0],
            ((2.0 + time as f64 / 1e9 * 0.5) * 40.0).round() as u8
        );
    }
    drop(backend);
    assert_eq!(trace.lock().unwrap().live, 0);
}
