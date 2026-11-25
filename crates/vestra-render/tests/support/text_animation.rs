//! Text animation semantics and independently ordered CPU/WGPU submissions.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use vestra_core::{
    plan::{CompileOptions, PlanCompileInput, RenderPlan, ScheduledItem, compile, evaluate},
    project::Project,
    timeline::frame_time_nanos,
    validation::{ResourceLimits, validate},
};
use vestra_render::{CpuBackend, DecodedAssets, RenderBackend, compare_rgba};

use super::{gpu::wgpu_backend_or_skip_depth, lifecycle::RenderFrame};

const BACKDROP: Rgba<u8> = Rgba([32, 32, 48, 255]);
const CLIP_START: f64 = 0.2;
const CLIP_DURATION: f64 = 0.6;
const FRAME_RATE: (u64, u64) = (30_000, 1001);

fn clip(id: &str, position: (f64, f64), source: Value) -> Value {
    json!({
        "id": id, "source": source, "start": CLIP_START, "duration": CLIP_DURATION, "layer": 0,
        "opacity": {"base_value": 1.0},
        "transform": {
            "position": {"base_value": {"x": position.0, "y": position.1}},
            "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
            "scale": {"base_value": {"x": 1.0, "y": 1.0}},
            "rotation_degrees": {"base_value": 0.0}
        }
    })
}

fn text_animation_project() -> Value {
    json!({
        "schema_version": 1,
        "output": {
            "path": "text-animation.mp4", "width": 240, "height": 200,
            "frame_rate": "30000/1001", "background": "#202030", "quality": "preview",
            "audio": false, "duration_mode": "explicit", "duration": 1.0
        },
        "assets": [{"id": "font", "type": "font", "source": "tests/assets/VestraTest-Regular.ttf"}],
        "visual": {"clips": [
            clip("reveal", (0.5, 0.18), json!({
                "type": "text", "text": "ffix\u{0301}", "font": "font", "font_size": 44.0,
                "fill": "#f0f0f0", "max_width": 150.0, "align": "center",
                "reveal": {"base_value": 0.0, "keyframes": [
                    {"time": 0.1, "value": 0.49, "interpolation": "hold"},
                    {"time": 0.2, "value": 0.5, "interpolation": "hold"},
                    {"time": 0.3, "value": 0.99, "interpolation": "hold"},
                    {"time": 0.4, "value": 1.0, "interpolation": "hold"}
                ]}
            })),
            clip("highlight", (0.5, 0.7), json!({
                "type": "text", "text": "écho écho", "font": "font", "font_size": 38.0,
                "fill": "#f0f0f0", "max_width": 110.0, "align": "center",
                "word_highlight": {"colour": "#f01010", "words": [
                    {"start": 0.0, "end": 0.2, "text_start": 0, "text_end": 4},
                    {"start": 0.3, "end": 0.5, "text_start": 5, "text_end": 9}
                ]}
            }))
        ]}
    })
}

fn compile_text_animation(project: &Project) -> RenderPlan {
    let limits = ResourceLimits::default();
    let report = validate(project, limits);
    assert!(report.is_valid(), "{:?}", report.diagnostics());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = BTreeMap::from([("font".to_owned(), root.join(&project.assets[0].source))]);
    compile(
        PlanCompileInput::new(
            project,
            limits,
            &root,
            &assets,
            &BTreeMap::new(),
            1.0,
            FRAME_RATE,
            30,
            &[],
        ),
        CompileOptions::default(),
    )
    .expect("compile text animation")
}

#[test]
fn gpu_text_animation_reads_full_shaping_in_forward_and_out_of_order_frames() {
    let project: Project =
        serde_json::from_value(text_animation_project()).expect("parse text project");
    let plan = compile_text_animation(&project);
    assert_eq!(plan.compilation.dynamic_layer_count, 2);
    let decoded = DecodedAssets::build(&plan).expect("prepare text animation");
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let mut gpu = wgpu_backend_or_skip_depth(&plan, decoded, 2);
    let forward = [5, 6, 9, 12, 15, 18, 23, 24];
    let out_of_order = [18, 6, 24, 12, 9, 23, 5, 15];
    let mut frames = BTreeMap::new();
    let mut submission = 0;
    for order in [forward, out_of_order] {
        for batch in order.chunks_exact(2) {
            let mut expected = BTreeMap::new();
            for &index in batch {
                let time = frame_time_nanos(index, FRAME_RATE.0, FRAME_RATE.1).expect("frame time");
                let active = if (6..24).contains(&index) {
                    &[ScheduledItem(0), ScheduledItem(1)][..]
                } else {
                    &[]
                };
                let frame = evaluate(&plan, active, time).expect("evaluate text frame");
                let mut image = RgbaImage::new(frame.width, frame.height);
                cpu.render_frame(&frame, &mut image)
                    .expect("render CPU text frame");
                if let Some(previous) = frames.get(&index) {
                    assert_eq!(previous, &image, "CPU frame {index} changed on repeat");
                } else {
                    frames.insert(index, image.clone());
                }
                if let Some(gpu) = &mut gpu {
                    gpu.submit_frame(submission, &frame)
                        .expect("submit WGPU text frame");
                    expected.insert(submission, (index, image));
                    submission += 1;
                }
            }
            if let Some(gpu) = &mut gpu {
                for completed in gpu.flush().expect("flush WGPU text frames") {
                    let (index, cpu) = expected
                        .remove(&completed.frame_number)
                        .expect("expected frame");
                    let difference = compare_rgba(cpu.as_raw(), &completed.rgba, 2);
                    assert!(
                        difference.maximum_absolute_channel_error <= 2,
                        "text frame {index}: {difference:?}"
                    );
                }
                assert!(expected.is_empty(), "all submitted text frames complete");
            }
        }
    }

    let title = |index| image::imageops::crop_imm(&frames[&index], 0, 0, 240, 72).to_image();
    let partial = title(12);
    let full = title(18);
    for index in [5, 6, 9, 24] {
        assert!(title(index).pixels().all(|pixel| *pixel == BACKDROP));
    }
    assert_eq!(
        partial,
        title(15),
        "reveal holds the combining cluster until complete"
    );
    assert_eq!(full, title(23), "the last active frame retains full reveal");
    assert!(partial.pixels().any(|pixel| *pixel != BACKDROP));
    assert!(
        partial
            .enumerate_pixels()
            .all(|(x, y, pixel)| *pixel == BACKDROP || pixel == full.get_pixel(x, y)),
        "the complete ligature keeps its final pixels"
    );
    assert!(
        partial
            .pixels()
            .zip(full.pixels())
            .any(|(partial, full)| *partial == BACKDROP && *full != BACKDROP),
        "the last cluster appears at the final reveal step"
    );

    let caption = |index| image::imageops::crop_imm(&frames[&index], 0, 72, 240, 120).to_image();
    let first = caption(6);
    let gap = caption(12);
    let second = caption(15);
    for painted in [&first, &second] {
        // The alternate fill has the same red channel, so this compares glyph coverage exactly.
        assert!(
            painted
                .pixels()
                .zip(gap.pixels())
                .all(|(left, right)| left[0] == right[0])
        );
    }
    let red_rows = |image: &RgbaImage| {
        image
            .enumerate_pixels()
            .filter(|(_, _, pixel)| pixel[0] > pixel[1].saturating_add(20))
            .map(|(_, y, _)| y)
            .collect::<Vec<_>>()
    };
    let first_rows = red_rows(&first);
    let second_rows = red_rows(&second);
    assert!(!first_rows.is_empty() && !second_rows.is_empty());
    assert!(
        first_rows.iter().max() < second_rows.iter().min(),
        "repeated words color independent wrapped lines"
    );
    assert!(red_rows(&gap).is_empty());
    assert!(red_rows(&caption(23)).is_empty());
    assert_eq!(
        frames[&5], frames[&24],
        "cue start and end are sampled on the correct sides"
    );
}

#[test]
fn text_animation_validates_canonical_track_times_and_unicode_ranges() {
    let mut project = text_animation_project();
    project["visual"]["clips"][0]["source"]["reveal"]["keyframes"][3]["time"] = json!(0.7);
    let second = &mut project["visual"]["clips"][1]["source"]["word_highlight"]["words"][1];
    second["start"] = json!(0.19);
    second["text_end"] = json!(10);
    let project: Project =
        serde_json::from_value(project).expect("invalid animation parses structurally");
    let report = validate(&project, ResourceLimits::default());
    for (code, pointer) in [
        (
            "VESTRA-KEYFRAME-TIME",
            "/visual/clips/0/source/reveal/keyframes/3/time",
        ),
        (
            "VESTRA-TEXT-HIGHLIGHT-TIME",
            "/visual/clips/1/source/word_highlight/words/1",
        ),
        (
            "VESTRA-TEXT-HIGHLIGHT-RANGE",
            "/visual/clips/1/source/word_highlight/words/1/text_end",
        ),
    ] {
        assert!(
            report.diagnostics().iter().any(|diagnostic| {
                diagnostic.code == code && diagnostic.pointer.as_deref() == Some(pointer)
            }),
            "{code}: {:?}",
            report.diagnostics()
        );
    }
}
