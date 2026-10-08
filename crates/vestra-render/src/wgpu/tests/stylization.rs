//! Canonical palette/dither projects, actual backend parity, and visual artifacts.

use std::{fs, path::PathBuf, sync::Arc};

use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use vestra_core::plan::{CompileOptions, RenderPlan, ScheduledItem, compile};

use super::{FrameDifference, compare_rgba, gpu::wgpu_backend_or_skip};
use crate::{
    DecodedAssets,
    render::{CpuBackend, RenderBackend, WgpuBackend},
    test_support::{ValidationOptions, load_and_validate},
};

const MONO: [&str; 2] = ["#000000", "#ffffff"];
const MONO_FIVE: [&str; 5] = ["#000000", "#404040", "#808080", "#bfbfbf", "#ffffff"];
const EMBER: [&str; 5] = ["#080508", "#351120", "#9e3341", "#efa463", "#fff1c5"];
const OCEAN: [&str; 5] = ["#040b19", "#18324c", "#277d89", "#88c1bc", "#eef8d9"];
const VIOLET: [&str; 5] = ["#090714", "#34234f", "#7d528a", "#ca95bd", "#f7eddf"];

fn artifact_directory(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/stylization/frames")
        .join(name);
    fs::create_dir_all(&directory).expect("create stylization artifact directory");
    directory
}

fn palette(palette: &[&str], mode: &str) -> Value {
    json!({"type": "palette_map", "id": "palette", "palette": palette,
        "mode": mode, "amount": {"base_value": 1.0}, "phase": {"base_value": 0.0}})
}

fn dither(palette: &[&str], matrix: &str, scale: u8) -> Value {
    json!({"type": "ordered_dither", "id": "dither", "palette": palette,
        "mode": "nearest", "amount": {"base_value": 1.0}, "phase": {"base_value": 0.0},
        "strength": {"base_value": 1.0}, "matrix": matrix, "scale": scale})
}

fn image_clip(effects: Vec<Value>) -> Value {
    json!({"id": "image", "source": {"type": "image", "asset": "source"},
        "start": 0.0, "duration": 6.0, "layer": 0,
        "sizing": {"mode": "fit"}, "opacity": {"base_value": 1.0}, "effects": effects,
        "transform": {"position": {"base_value": {"x": 0.5, "y": 0.5}},
            "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
            "scale": {"base_value": {"x": 1.0, "y": 1.0}}}})
}

fn project(width: u32, height: u32, clips: Vec<Value>, effects: Vec<Value>) -> Value {
    json!({"schema_version": 1, "name": "Deterministic stylization fixture",
        "output": {"path": "output.mp4", "width": width, "height": height,
            "frame_rate": "24/1", "background": "#00000000", "quality": "preview",
            "audio": false, "duration_mode": "explicit", "duration": 6.0},
        "assets": [{"id": "source", "type": "image", "source": "source.png"}],
        "visual": {"clips": clips, "post_effects": effects}})
}

fn fixture(name: &str, source: &RgbaImage, value: &Value) -> RenderPlan {
    let directory = artifact_directory(name);
    source
        .save(directory.join("source.png"))
        .expect("save source fixture");
    let path = directory.join("project.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(value).expect("serialize fixture"),
    )
    .expect("save canonical fixture");
    let canonical = serde_json::from_value(value.clone()).expect("parse canonical project");
    let validation = vestra_core::validation::validate(
        &canonical,
        vestra_core::validation::ResourceLimits::default(),
    );
    assert!(
        validation.is_valid(),
        "invalid stylization fixture: {:?}",
        validation.diagnostics()
    );
    let input = load_and_validate(
        &path,
        &ValidationOptions {
            check_backend: false,
        },
    )
    .expect("load stylization canonical JSON");
    compile(input, CompileOptions::default()).expect("compile stylization fixture")
}

struct Backends {
    plan: RenderPlan,
    cpu: CpuBackend,
    gpu: WgpuBackend,
}

impl Backends {
    fn new(plan: RenderPlan) -> Option<Self> {
        let decoded = DecodedAssets::build(&plan).expect("decode stylization fixture");
        let cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
        let gpu = wgpu_backend_or_skip(&plan, decoded)?;
        let adapter = gpu.adapter().expect("WGPU reports adapter metadata");
        eprintln!(
            "STYLIZATION_ADAPTER classification={:?} adapter={} backend={}",
            adapter.performance_class(),
            adapter.adapter_name,
            adapter.graphics_backend
        );
        Some(Self { plan, cpu, gpu })
    }

    fn render(&mut self, name: &str, time: u128, tolerance: u8) -> RgbaImage {
        let (cpu, _, difference) = self.render_pair(name, time, tolerance);
        assert!(
            difference.maximum_absolute_channel_error <= tolerance,
            "{name} at {time} exceeded {tolerance}: {difference:?}"
        );
        cpu
    }

    fn render_pair(
        &mut self,
        name: &str,
        time: u128,
        tolerance: u8,
    ) -> (RgbaImage, RgbaImage, FrameDifference) {
        let active = (0..self.plan.layers.len())
            .map(ScheduledItem)
            .collect::<Vec<_>>();
        let frame = vestra_core::plan::evaluate(&self.plan, &active, time)
            .expect("evaluate stylization frame");
        let mut cpu = RgbaImage::new(frame.width, frame.height);
        let mut gpu = RgbaImage::new(frame.width, frame.height);
        self.cpu
            .render_frame(&frame, &mut cpu)
            .expect("CPU stylization frame");
        self.gpu
            .render_frame(&frame, &mut gpu)
            .expect("WGPU stylization frame");
        let directory = artifact_directory(name);
        cpu.save(directory.join(format!("cpu-{time}.png")))
            .expect("save CPU frame");
        gpu.save(directory.join(format!("wgpu-{time}.png")))
            .expect("save WGPU frame");
        let difference = compare_rgba(cpu.as_raw(), gpu.as_raw(), tolerance);
        fs::write(
            directory.join(format!("parity-{time}.json")),
            serde_json::to_vec_pretty(&difference).expect("serialize parity"),
        )
        .expect("save parity metrics");
        (cpu, gpu, difference)
    }
}

// An original, deterministic scene with a person, moon, mountains, fine trees,
// midtone gradients, and shadow texture. No external media or font licenses.
fn rich_source(width: u32, height: u32) -> RgbaImage {
    RgbaImage::from_fn(width, height, |x, y| {
        let u = x as f64 / f64::from(width);
        let v = y as f64 / f64::from(height);
        let texture = ((x.wrapping_mul(13) ^ y.wrapping_mul(29)) % 11) as f64;
        let mut rgb = [18.0 + 120.0 * v, 25.0 + 130.0 * u, 42.0 + 115.0 * v];
        if (u - 0.76).powi(2) + ((v - 0.24) * 0.56).powi(2) < 0.005 {
            rgb = [235.0, 218.0, 172.0];
        }
        let ridge = 0.55 + 0.12 * (u * 18.0).sin() + 0.025 * (u * 51.0).sin();
        if v > ridge {
            rgb = [18.0 + texture, 30.0 + texture, 44.0 + texture];
        }
        let head = ((u - 0.43) / 0.036).powi(2) + ((v - 0.43) / 0.064).powi(2) < 1.0;
        let torso = v > 0.485 && v < 0.77 && (u - 0.43).abs() < 0.046 + (v - 0.5) * 0.065;
        let legs =
            (0.77..0.95).contains(&v) && ((u - 0.401).abs() < 0.021 || (u - 0.46).abs() < 0.021);
        if head || torso || legs {
            rgb = [99.0 + texture, 76.0 + texture, 64.0 + texture];
        }
        if (x % 41 == 0 && v > 0.57) || (y % 23 == 0 && u > 0.6 && v > 0.7) {
            rgb = [182.0, 156.0, 112.0];
        }
        if y < height / 12 {
            let ramp = u * 255.0;
            rgb = [ramp, ramp, ramp];
        }
        Rgba([
            rgb[0].round() as u8,
            rgb[1].round() as u8,
            rgb[2].round() as u8,
            255,
        ])
    })
}

#[test]
fn gpu_stylization_palette_map_gradient_matches_literal_gray_to_red_ramp() {
    let source = RgbaImage::from_fn(6, 4, |x, _| {
        let value = [0, 32, 64, 128, 192, 255][x as usize];
        Rgba([value, value, value, 255])
    });
    let name = "literal-ramp";
    let plan = fixture(
        name,
        &source,
        &project(
            6,
            4,
            vec![image_clip(vec![palette(
                &["#000000", "#ff0000"],
                "gradient",
            )])],
            vec![],
        ),
    );
    let Some(mut backends) = Backends::new(plan) else {
        return;
    };
    let output = backends.render(name, 0, 0);
    for (x, expected) in [0, 32, 64, 128, 192, 255].into_iter().enumerate() {
        assert_eq!(output.get_pixel(x as u32, 1).0, [expected, 0, 0, 255]);
    }
}

#[test]
fn gpu_stylization_ordered_dither_bayer2_has_literal_fine_checker_pattern() {
    let source = RgbaImage::from_pixel(8, 4, Rgba([128, 128, 128, 255]));
    let name = "literal-bayer2";
    let plan = fixture(
        name,
        &source,
        &project(
            8,
            4,
            vec![image_clip(vec![])],
            vec![dither(&MONO, "bayer2", 1)],
        ),
    );
    let Some(mut backends) = Backends::new(plan) else {
        return;
    };
    let output = backends.render(name, 0, 0);
    for y in 0..4 {
        for x in 0..8 {
            let value = if (x + y) % 2 == 0 { 0 } else { 255 };
            assert_eq!(output.get_pixel(x, y).0, [value, value, value, 255]);
        }
    }
}

#[test]
fn gpu_stylization_fine_dither_preserves_scene_detail_with_three_distinct_palettes() {
    let source = rich_source(640, 360);
    let mut reference_indices = None;
    for (name, colours) in [
        ("monochrome", &MONO_FIVE),
        ("ember", &EMBER),
        ("ocean", &OCEAN),
    ] {
        let plan = fixture(
            name,
            &source,
            &project(
                640,
                360,
                vec![image_clip(vec![])],
                vec![dither(colours, "bayer8", 1)],
            ),
        );
        let Some(mut backends) = Backends::new(plan) else {
            return;
        };
        let output = backends.render(name, 0, 0);
        assert_ne!(output, source, "stylization must alter source colors");
        let colours_used = output
            .pixels()
            .map(|pixel| pixel.0)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            colours_used.len(),
            colours.len(),
            "all tone stops should appear"
        );
        let stops = colours
            .iter()
            .map(|colour| {
                vestra_core::project::parse_colour(colour).expect("valid authored palette stop")
            })
            .collect::<Vec<_>>();
        let indices = output
            .pixels()
            .map(|pixel| {
                stops
                    .iter()
                    .position(|stop| *stop == pixel.0)
                    .expect("full-strength nearest dither must emit an authored palette stop")
            })
            .collect::<Vec<_>>();
        if let Some(reference) = &reference_indices {
            assert_eq!(
                &indices, reference,
                "palette hue must not change spatial tone structure"
            );
        } else {
            reference_indices = Some(indices);
        }
        let changes = (0..640 - 1)
            .filter(|x| output.get_pixel(*x, 12) != output.get_pixel(*x + 1, 12))
            .count();
        assert!(
            changes > 80,
            "fine ordered pattern lost tonal texture: {changes}"
        );
    }
}

#[test]
fn gpu_stylization_palette_and_dither_order_changes_rendered_color() {
    let source = RgbaImage::from_pixel(16, 8, Rgba([128, 128, 128, 255]));
    let map = palette(&["#000000", "#ff0000"], "gradient");
    let quantize = dither(&MONO, "bayer2", 1);
    let mut outputs = Vec::new();
    for (name, effects) in [
        ("map-before-dither", vec![map.clone(), quantize.clone()]),
        ("dither-before-map", vec![quantize, map]),
    ] {
        let plan = fixture(
            name,
            &source,
            &project(16, 8, vec![image_clip(effects)], vec![]),
        );
        let Some(mut backends) = Backends::new(plan) else {
            return;
        };
        outputs.push(backends.render(name, 0, 0));
    }
    assert_ne!(
        outputs[0], outputs[1],
        "declared effect order must affect output"
    );
    assert!(outputs[1].pixels().any(|pixel| pixel.0 == [255, 0, 0, 255]));
    assert!(
        outputs[0]
            .pixels()
            .all(|pixel| pixel[0] == pixel[1] && pixel[1] == pixel[2])
    );
}

#[test]
fn gpu_stylization_periodic_palette_invalidates_static_image_cache_for_out_of_order_frames() {
    for (kind, animated) in [
        ("palette", palette(&EMBER, "gradient")),
        ("dither", dither(&EMBER, "bayer8", 1)),
        ("rainbow", palette(&EMBER, "rainbow")),
    ] {
        for global in [false, true] {
            let name = format!("period-{kind}-{}", if global { "global" } else { "clip" });
            let mut animated = animated.clone();
            animated["period"] = json!(2.0);
            let source = rich_source(96, 64);
            let clip_effects = if global {
                vec![]
            } else {
                vec![animated.clone()]
            };
            let post_effects = if global { vec![animated] } else { vec![] };
            let plan = fixture(
                &name,
                &source,
                &project(96, 64, vec![image_clip(clip_effects)], post_effects),
            );
            assert_eq!(
                plan.visual_dependency,
                vestra_core::plan::TemporalDependency::Dynamic,
                "periodic color must be dynamic"
            );
            let Some(mut backends) = Backends::new(plan) else {
                return;
            };
            let later = backends.render(&name, 2_250_000_000, 0);
            let middle = backends.render(&name, 900_000_000, 0);
            let early = backends.render(&name, 250_000_000, 0);
            assert_eq!(early, later, "frame t and t + period must agree");
            assert_ne!(
                early, middle,
                "static source cache must retain animated effects"
            );
            assert_eq!(
                early,
                backends.render(&name, 250_000_000, 0),
                "repeat random frame"
            );
        }
    }
}

#[test]
fn gpu_stylization_keyframed_palette_amount_invalidates_static_image_cache() {
    let mut mapped = palette(&["#000000", "#ff0000"], "gradient");
    mapped["amount"] = json!({"base_value": 0.0, "keyframes": [
        {"time": 0.0, "value": 0.0, "interpolation": "linear"},
        {"time": 1.0, "value": 1.0, "interpolation": "linear"}]});
    let source = RgbaImage::from_pixel(32, 16, Rgba([128, 128, 128, 255]));
    let name = "keyframed-amount";
    let plan = fixture(
        name,
        &source,
        &project(32, 16, vec![image_clip(vec![mapped])], vec![]),
    );
    let Some(mut backends) = Backends::new(plan) else {
        return;
    };
    let full = backends.render(name, 1_000_000_000, 0);
    assert_eq!(full.get_pixel(16, 8).0, [128, 0, 0, 255]);
    let original = backends.render(name, 0, 0);
    assert_eq!(original, source, "amount zero must reproduce the source");
    let half = backends.render(name, 500_000_000, 0);
    assert_eq!(half.get_pixel(16, 8).0, [128, 64, 64, 255]);
}

#[test]
fn gpu_stylization_palette_and_dither_preserve_alpha_through_nested_masks_and_matte() {
    let source = rich_source(96, 64);
    let mut image = image_clip(vec![dither(&OCEAN, "bayer4", 1)]);
    image["transform"] = json!({"position": {"base_value": {"x": 0.5, "y": 0.5}},
        "anchor": {"base_value": {"x": 0.5, "y": 0.5}},
        "scale": {"base_value": {"x": 0.75, "y": 0.75}},
        "rotation_degrees": {"base_value": 0.0}});
    image["masks"] = json!([{"id": "oval", "input": {"type": "shape",
        "geometry": {"type": "ellipse", "width": 78.0, "height": 52.0},
        "fill": "#ffffff"}, "feather": {"base_value": 0.0}}]);
    let inner = json!({"id": "inner", "source": {"type": "group", "clips": [image]},
        "start": 0.0, "duration": 6.0, "layer": 0, "opacity": {"base_value": 1.0}});
    let outer = json!({"id": "outer", "source": {"type": "group", "clips": [inner]},
        "start": 0.0, "duration": 6.0, "layer": 0, "opacity": {"base_value": 1.0},
        "effects": [palette(&VIOLET, "gradient")],
        "matte": {"source_layer": "matte", "mode": "alpha", "invert": false}});
    let matte = json!({"id": "matte", "source": {"type": "shape",
        "geometry": {"type": "rectangle", "width": 84.0, "height": 58.0},
        "fill": "#ffffff80"}, "start": 0.0, "duration": 6.0, "layer": 1,
        "visible": false, "opacity": {"base_value": 1.0}});
    let name = "nested-mask-matte";
    let value = project(96, 64, vec![outer, matte], vec![]);
    let mut control = value.clone();
    control["visual"]["clips"][0]["effects"] = json!([]);
    control["visual"]["clips"][0]["source"]["clips"][0]["source"]["clips"][0]["effects"] =
        json!([]);
    let Some(mut control_backends) =
        Backends::new(fixture("nested-mask-matte-control", &source, &control))
    else {
        return;
    };
    let (control_cpu, control_gpu, baseline_difference) =
        control_backends.render_pair("nested-mask-matte-control", 0, 3);
    assert!(
        baseline_difference.maximum_absolute_channel_error <= 3,
        "pixel-aligned control has unexpected sampling error: {baseline_difference:?}"
    );
    let plan = fixture(name, &source, &value);
    let Some(mut backends) = Backends::new(plan) else {
        return;
    };
    let (output, gpu_output, difference) = backends.render_pair(name, 0, 3);
    // Dither amplifies inherited source sampling at threshold boundaries.
    // At (46,15), Vulkan control RGB was [36,87,59] vs [36,86,59]; just
    // one of 6144 styled pixels diverged (<0.02%). Allow only baseline sites.
    for (index, (((cpu, gpu), original_cpu), original_gpu)) in output
        .pixels()
        .zip(gpu_output.pixels())
        .zip(control_cpu.pixels())
        .zip(control_gpu.pixels())
        .enumerate()
    {
        assert_eq!(cpu[3], gpu[3], "styled alpha parity at {index}");
        assert_eq!(
            cpu[3], original_cpu[3],
            "CPU effect altered mask/matte alpha at {index}"
        );
        assert_eq!(
            gpu[3], original_gpu[3],
            "WGPU effect altered mask/matte alpha at {index}"
        );
        if cpu
            .0
            .iter()
            .zip(gpu.0)
            .any(|(left, right)| left.abs_diff(right) > 3)
        {
            assert_ne!(
                original_cpu.0[..3],
                original_gpu.0[..3],
                "stylization divergence at {index} has no inherited sampling difference"
            );
        }
    }
    let mismatch_fraction = difference.pixels_exceeding_tolerance as f64
        / (f64::from(output.width()) * f64::from(output.height()));
    assert!(
        mismatch_fraction <= 0.001 && difference.mean_absolute_channel_error <= 0.05,
        "inherited sampling amplification exceeds composed tolerance: {difference:?}"
    );
    eprintln!(
        "COMPOSED_SAMPLING_PARITY inherited_fraction={mismatch_fraction} difference={difference:?}"
    );
    assert_eq!(
        output.get_pixel(0, 0)[3],
        0,
        "effect must not fill transparent canvas"
    );
    assert!(
        output
            .pixels()
            .any(|pixel| pixel[3] > 100 && pixel[3] < 150),
        "semi-transparent track matte must survive stylization"
    );
}

#[test]
#[ignore = "explicit 1080p/4K correctness and resource validation"]
fn gpu_stylization_1080p_and_4k_match_cpu_with_fine_patterns() {
    for (name, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        let source = rich_source(width, height);
        let plan = fixture(
            name,
            &source,
            &project(
                width,
                height,
                vec![image_clip(vec![palette(&EMBER, "gradient")])],
                vec![dither(&EMBER, "bayer8", 1)],
            ),
        );
        let Some(mut backends) = Backends::new(plan) else {
            return;
        };
        let output = backends.render(name, 0, 0);
        assert_eq!(output.dimensions(), (width, height));
        assert!(output.pixels().all(|pixel| pixel[3] == 255));
    }
}

#[test]
fn gpu_stylization_uniform_palette_dither_threshold_matches_cpu_at_each_stage() {
    let source = RgbaImage::from_pixel(8, 8, Rgba([36, 92, 60, 255]));
    let map = palette(&EMBER, "gradient");
    let quantize = dither(&EMBER, "bayer8", 1);
    let mut fractional_map = map.clone();
    fractional_map["amount"] = json!({"base_value": 0.5});
    let mut fractional_dither = quantize.clone();
    fractional_dither["amount"] = json!({"base_value": 0.5});
    fractional_dither["strength"] = json!({"base_value": 0.5});
    let mut failures = Vec::new();
    for (stage, effects) in [
        ("palette-only", vec![map.clone()]),
        ("palette-then-dither", vec![map, quantize.clone()]),
        ("direct-dither", vec![quantize]),
        ("fractional-palette", vec![fractional_map.clone()]),
        (
            "fractional-chain",
            vec![fractional_map, fractional_dither.clone()],
        ),
        ("fractional-dither", vec![fractional_dither]),
    ] {
        let name = format!("threshold-{stage}");
        let plan = fixture(
            &name,
            &source,
            &project(8, 8, vec![image_clip(effects)], vec![]),
        );
        let Some(mut backends) = Backends::new(plan) else {
            return;
        };
        let frame = vestra_core::plan::evaluate(&backends.plan, &[ScheduledItem(0)], 0)
            .expect("evaluate threshold diagnostic");
        let mut cpu = RgbaImage::new(8, 8);
        let mut gpu = RgbaImage::new(8, 8);
        backends
            .cpu
            .render_frame(&frame, &mut cpu)
            .expect("CPU threshold diagnostic");
        backends
            .gpu
            .render_frame(&frame, &mut gpu)
            .expect("WGPU threshold diagnostic");
        let difference = compare_rgba(cpu.as_raw(), gpu.as_raw(), 0);
        let directory = artifact_directory(&name);
        cpu.save(directory.join("cpu-0.png"))
            .expect("save threshold CPU");
        gpu.save(directory.join("wgpu-0.png"))
            .expect("save threshold GPU");
        fs::write(
            directory.join("parity-0.json"),
            serde_json::to_vec_pretty(&difference).expect("serialize threshold parity"),
        )
        .expect("save threshold parity");
        eprintln!(
            "THRESHOLD_STAGE stage={stage} pixel=(3,5) cpu={:?} gpu={:?} difference={difference:?}",
            cpu.get_pixel(3, 5).0,
            gpu.get_pixel(3, 5).0
        );
        if difference.maximum_absolute_channel_error != 0 {
            failures.push((stage, difference));
        }
    }
    assert!(
        failures.is_empty(),
        "uniform byte-space threshold parity: {failures:?}"
    );
}
