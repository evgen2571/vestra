//! Manual prepared-work measurements; run profiling separately from timing.

use super::*;
use std::{hint::black_box, path::Path};
use vestra_core::plan::{
    ColourTransform, CompileOptions, CompiledSizing, EvaluatedComposition, EvaluatedEffect,
    EvaluatedLayer, EvaluatedSource, EvaluatedTrackMatte, ScheduledItem, TemporalDependency,
    compile, evaluate,
};

fn fixture(path: &str) -> RenderPlan {
    let validated = crate::test_support::load_and_validate(
        Path::new(path),
        &crate::test_support::ValidationOptions {
            check_backend: false,
        },
    )
    .expect("benchmark fixture validates");
    compile(validated, CompileOptions::default()).expect("benchmark fixture compiles")
}

fn measure(plan: &RenderPlan, frame: &EvaluatedFrame, label: &str) -> Vec<u8> {
    let decoded = DecodedAssets::build(plan).expect("benchmark assets prepare");
    let mut backend = CpuBackend::new_with_worker_count(plan, decoded, 1);
    backend.submit_frame(0, frame).expect("warmup submits");
    let expected = backend
        .poll_completed(PollMode::WaitForOne)
        .expect("warmup polls")
        .expect("warmup completes")
        .rgba;
    for sample in 0..5 {
        let before = backend.stats();
        backend.reset_operation_metrics();
        let started = Instant::now();
        let mut last = None;
        for number in 0..30 {
            backend.submit_frame(number, frame).expect("frame submits");
            last = backend
                .poll_completed(PollMode::WaitForOne)
                .expect("frame polls");
        }
        let elapsed = started.elapsed();
        assert_eq!(last.expect("frame completes").rgba, expected);
        let after = backend.stats();
        println!(
            "cpu_work label={label} sample={sample} width={} height={} frames=30 wall_ms={:.3} static_hits={} static_renders={} scratch_reuses={} copy_bytes={} gaussian_ms={:.3} raster_ms={:.3} profiled={}",
            frame.width,
            frame.height,
            elapsed.as_secs_f64() * 1000.0,
            after.static_cache_hits - before.static_cache_hits,
            after.static_layers_rendered - before.static_layers_rendered,
            after.cpu_scratch_reuses - before.cpu_scratch_reuses,
            after.cpu_full_frame_copy_bytes - before.cpu_full_frame_copy_bytes,
            backend.worker_hot_path_timings.gaussian_blur.as_secs_f64() * 1000.0,
            backend
                .worker_hot_path_timings
                .source_rasterization
                .as_secs_f64()
                * 1000.0,
            backend.profiling_enabled
        );
    }
    expected
}

#[test]
#[ignore = "manual release culling and mask/matte cache benchmark"]
fn cpu_work_elimination_benchmark() {
    for (label, path) in [
        ("masks", "examples/projects/geometric-masks.json"),
        ("matte", "examples/projects/track-matte.json"),
    ] {
        let plan = fixture(path);
        let items = (0..plan.layers.len())
            .map(ScheduledItem)
            .collect::<Vec<_>>();
        let mut frame = evaluate(&plan, &items, 0).expect("fixture evaluates");
        for layer in &mut frame.layers {
            for mask in &mut layer.masks {
                mask.feather = 4.0;
            }
        }
        let cached = measure(&plan, &frame, &format!("{label}_cached"));
        for layer in &mut frame.layers {
            layer.content_dependency = TemporalDependency::Dynamic;
        }
        let uncached = measure(&plan, &frame, &format!("{label}_uncached"));
        assert_eq!(cached, uncached, "cache preserves {label} pixels");
    }

    let mut plan = fixture("examples/projects/animation-effects.json");
    plan.canvas.width = 1280;
    plan.canvas.height = 720;
    let image = EvaluatedLayer {
        compiled_layer_index: 40,
        visible: true,
        content_dependency: TemporalDependency::Dynamic,
        source: EvaluatedSource::Image {
            asset_index: 0,
            crop: crate::domain::Crop {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            sizing: CompiledSizing::Original,
            cacheable_crop: false,
        },
        transform: crate::animation::Transform2D::identity(
            crate::domain::Point { x: 0.5, y: 0.5 },
            crate::domain::Point { x: 0.5, y: 0.5 },
        ),
        opacity: 1.0,
        effects: vec![EvaluatedEffect::GaussianBlur { radius: 8.0 }],
        masks: Vec::new(),
        matte: None,
        colour_transform: ColourTransform::default(),
        blend_mode: crate::project::BlendMode::Normal,
    };
    let mut group = image.clone();
    group.compiled_layer_index = 41;
    group.effects.clear();
    group.source = EvaluatedSource::Group {
        composition: EvaluatedComposition {
            layers: vec![image.clone()],
        },
    };
    let mut frame = EvaluatedFrame {
        time: 0,
        width: 1280,
        height: 720,
        background: [0; 4],
        layers: Vec::new(),
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    };
    for (label, layer) in [("image", image), ("group", group.clone())] {
        for offscreen in [false, true] {
            let mut layer = layer.clone();
            if offscreen {
                layer.transform.position.x = 4.0;
            }
            frame.layers = vec![layer];
            let pixels = measure(&plan, &frame, &format!("{label}_offscreen_{offscreen}"));
            assert_eq!(pixels.iter().all(|byte| *byte == 0), offscreen);
        }
    }
    group.visible = false;
    let mut consumer = group.clone();
    consumer.compiled_layer_index = 42;
    consumer.visible = true;
    consumer.source = EvaluatedSource::SolidColor { colour: [255; 4] };
    consumer.matte = Some(EvaluatedTrackMatte {
        source_layer_identity: 41,
        mode: crate::project::MatteMode::Alpha,
        invert: false,
    });
    for offscreen in [false, true] {
        group.transform.position.x = if offscreen { 4.0 } else { 0.5 };
        frame.layers = vec![consumer.clone(), group.clone()];
        let pixels = measure(
            &plan,
            &frame,
            &format!("hidden_group_matte_offscreen_{offscreen}"),
        );
        assert_eq!(
            pixels.iter().all(|byte| *byte == 0),
            offscreen,
            "hidden group still supplies the consumer matte"
        );
    }
}

#[test]
#[ignore = "manual release repeated animation evaluation benchmark"]
fn cpu_animation_evaluation_benchmark() {
    for path in [
        "examples/projects/animation-effects.json",
        "benchmarks/projects/nested-groups.json",
    ] {
        let plan = fixture(path);
        for repeated in [false, true] {
            for sample in 0..5 {
                let started = Instant::now();
                let mut tracks = 0;
                for number in 0..10_000_u128 {
                    let time = if repeated {
                        500_000_000
                    } else {
                        number % 30 * 33_333_333
                    };
                    let frame =
                        evaluate(&plan, &[ScheduledItem(0)], time).expect("frame evaluates");
                    tracks += frame.evaluated_track_count;
                    black_box(frame);
                }
                println!(
                    "cpu_evaluation fixture={path} repeated={repeated} sample={sample} calls=10000 tracks={tracks} wall_ms={:.3}",
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
    }
}
