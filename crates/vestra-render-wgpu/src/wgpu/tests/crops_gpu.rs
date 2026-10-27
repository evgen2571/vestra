//! WGPU-only unused-video preparation, source reads and upload accounting.
use super::gpu::wgpu_backend_or_skip;
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
