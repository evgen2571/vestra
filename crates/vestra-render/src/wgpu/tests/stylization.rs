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

#[test]
fn gpu_stylization_input_exposure_negative_stop_keeps_upward_half_byte_ties() {
    let source = RgbaImage::from_fn(256, 4, |x, _| Rgba([x as u8, x as u8, x as u8, 128]));
    let mut effect = palette(&MONO, "gradient");
    effect["input_exposure"] = json!({"base_value": -1.0});
    let name = "input-exposure-half-byte";
    let Some(mut backends) = Backends::new(fixture(
        name,
        &source,
        &project(256, 4, vec![image_clip(vec![])], vec![effect]),
    )) else {
        return;
    };
    let output = backends.render(name, 0, 0);
    for x in 0..256 {
        assert_eq!(
            output.get_pixel(x, 0).0,
            [
                x.div_ceil(2) as u8,
                x.div_ceil(2) as u8,
                x.div_ceil(2) as u8,
                128
            ]
        );
    }
}

#[test]
#[ignore = "explicit staged detail quantization rounding diagnostic"]
fn gpu_stylization_input_detail_rounding_stages() {
    let source = rich_source(320, 180);
    let sharpen = json!({"type": "sharpen", "id": "detail", "amount": {"base_value": 1.5}, "radius": {"base_value": 1}});
    let gamma = json!({"type": "color_adjust", "id": "tone", "exposure": {"base_value": 0}, "gamma": {"base_value": 1.5},
        "black_point": {"base_value": 0}, "white_point": {"base_value": 1}});
    let mut differences = Vec::new();
    for (name, effects) in [
        (
            "blur",
            vec![json!({"type": "gaussian_blur", "id": "blur", "radius": {"base_value": 1}})],
        ),
        (
            "detail8",
            vec![
                json!({"type": "palette_map", "id": "detail", "palette": MONO, "mode": "gradient",
            "amount": {"base_value": 1}, "phase": {"base_value": 0}, "input_detail": {"base_value": 0.5}, "input_detail_radius": {"base_value": 8}}),
            ],
        ),
        ("sharpen", vec![sharpen.clone()]),
        ("gamma", vec![gamma.clone()]),
        ("sharpen-gamma", vec![sharpen, gamma]),
    ] {
        let name = format!("input-detail-stage-{name}");
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(320, 180, vec![image_clip(vec![])], effects),
        )) else {
            return;
        };
        let (_, _, difference) = backends.render_pair(&name, 0, 0);
        eprintln!("DETAIL_STAGE {name} {difference:?}");
        if difference.maximum_absolute_channel_error > 0 {
            differences.push((name, difference));
        }
    }
    assert!(
        differences.is_empty(),
        "detail rounding differences: {differences:?}"
    );
}

#[test]
fn gpu_stylization_input_detail_preserves_blend_source_and_scene_contrast() {
    for (kind, mut effect) in [
        ("map", palette(&MONO, "nearest")),
        ("dither", dither(&MONO, "blue_noise", 1)),
    ] {
        effect["input_detail"] = json!({"base_value": 4.0});
        effect["amount"] = json!({"base_value": 0.5});
        if kind == "dither" {
            effect["strength"] = json!({"base_value": 0.0});
        }
        let source = RgbaImage::from_fn(8, 8, |x, y| {
            let value = if x == 4 && y == 4 { 127 } else { 96 };
            Rgba([value, value, value, 128])
        });
        let name = format!("input-detail-original-{kind}");
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(8, 8, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        assert_eq!(
            backends.render(&name, 0, 0).get_pixel(4, 4).0,
            [191, 191, 191, 128]
        );
    }
    let source = rich_source(320, 180);
    for colours in [&MONO_FIVE[..], &EMBER[..], &OCEAN[..]] {
        for (radius, detail) in [(1.0, 1.5), (8.0, 0.5)] {
            for (kind, mut effect) in [
                ("map", palette(colours, "gradient")),
                ("bayer", dither(colours, "bayer8", 1)),
                ("blue", dither(colours, "blue_noise", 1)),
            ] {
                effect["input_detail"] = json!({"base_value": 0.0, "keyframes": [
                    {"time": 0.0, "value": 0.0, "interpolation": "linear"},
                    {"time": 2.0, "value": detail, "interpolation": "linear"}]});
                effect["input_detail_radius"] = json!({"base_value": radius});
                effect["input_gamma"] = json!({"base_value": 1.5});
                let name = format!("input-detail-{kind}-{radius}-{}", colours[1]);
                let Some(mut backends) = Backends::new(fixture(
                    &name,
                    &source,
                    &project(320, 180, vec![image_clip(vec![])], vec![effect.clone()]),
                )) else {
                    return;
                };
                let frames =
                    [0, 777_000_000, 2_000_000_000, 0].map(|time| backends.render(&name, time, 0));
                assert_eq!(frames[0], frames[3]);
                assert_ne!(frames[0], frames[2]);
                let mut contact = RgbaImage::new(960, 180);
                for (index, frame) in [&source, &frames[0], &frames[2]].into_iter().enumerate() {
                    image::imageops::replace(&mut contact, frame, (index * 320) as i64, 0);
                }
                contact
                    .save(artifact_directory(&name).join("contact.png"))
                    .expect("save detail contact sheet");
            }
        }
    }
}

#[test]
fn gpu_stylization_input_tone_changes_quantization_without_changing_blend_source() {
    for (kind, mut effect) in [
        ("map", palette(&MONO, "nearest")),
        ("dither", dither(&MONO, "blue_noise", 1)),
    ] {
        effect["amount"] = json!({"base_value": 0.5});
        effect["input_exposure"] = json!({"base_value": 1.0});
        if kind == "dither" {
            effect["strength"] = json!({"base_value": 0.0});
        }
        let source = RgbaImage::from_pixel(16, 16, Rgba([64, 64, 64, 128]));
        let name = format!("input-tone-original-{kind}");
        let plan = fixture(
            &name,
            &source,
            &project(16, 16, vec![image_clip(vec![effect])], vec![]),
        );
        let Some(mut backends) = Backends::new(plan) else {
            return;
        };
        let output = backends.render(&name, 0, 0);
        assert!(output.pixels().all(|p| p.0 == [160, 160, 160, 128]));
    }
    for colours in [&MONO_FIVE[..], &EMBER[..], &OCEAN[..]] {
        for (kind, mut effect) in [
            ("map", palette(colours, "gradient")),
            ("bayer", dither(colours, "bayer8", 1)),
            ("blue", dither(colours, "blue_noise", 1)),
        ] {
            effect["input_gamma"] = json!({"base_value": 1.0, "keyframes": [
                {"time": 0.0, "value": 1.0, "interpolation": "linear"},
                {"time": 2.0, "value": 2.0, "interpolation": "linear"}]});
            effect["input_exposure"] = json!({"base_value": 0.5});
            let name = format!("input-tone-{kind}-{}", colours[1]);
            let source = rich_source(320, 180);
            let plan = fixture(
                &name,
                &source,
                &project(320, 180, vec![image_clip(vec![effect])], vec![]),
            );
            let Some(mut backends) = Backends::new(plan) else {
                return;
            };
            let frames =
                [0, 777_000_000, 2_000_000_000, 0].map(|time| backends.render(&name, time, 0));
            assert_eq!(frames[0], frames[3]);
            assert_ne!(frames[0], frames[2]);
            let mut contact = RgbaImage::new(960, 180);
            for (index, frame) in [&source, &frames[0], &frames[2]].into_iter().enumerate() {
                image::imageops::replace(&mut contact, frame, (index * 320) as i64, 0);
            }
            contact
                .save(artifact_directory(&name).join("contact.png"))
                .expect("save tone contact sheet");
        }
    }
}

fn artifact_directory(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/stylization/frames")
        .join(std::env::var("VESTRA_WGPU_BACKEND").unwrap_or_else(|_| "auto".to_owned()))
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
        let adapter = self.gpu.adapter().expect("adapter metadata");
        fs::write(
            directory.join("adapter.json"),
            serde_json::to_vec_pretty(&json!({
                "adapter": adapter.adapter_name,
                "backend": adapter.graphics_backend,
                "classification": format!("{:?}", adapter.performance_class()),
            }))
            .expect("serialize adapter metadata"),
        )
        .expect("save adapter metadata");
        let resources = self.gpu.resource_estimates();
        fs::write(
            directory.join("resources.json"),
            serde_json::to_vec_pretty(&json!({
                "source_texture_bytes": resources.source_texture_bytes,
                "working_texture_bytes": resources.working_texture_bytes,
                "effect_texture_bytes": resources.effect_texture_bytes,
                "readback_buffer_bytes": resources.readback_buffer_bytes,
                "parameter_buffer_bytes": resources.parameter_buffer_bytes,
                "total_persistent_bytes": resources.total_persistent_bytes,
                "total_staging_bytes": resources.total_staging_bytes,
                "note": "renderer estimates exclude driver metadata and device texture padding",
            }))
            .expect("serialize resource estimates"),
        )
        .expect("save resource estimates");
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
    for matrix in ["bayer8", "blue_noise"] {
        let source = rich_source(640, 360);
        let mut reference_indices = None;
        for (name, colours) in [
            ("monochrome", &MONO_FIVE),
            ("ember", &EMBER),
            ("ocean", &OCEAN),
        ] {
            let name = &format!("{name}-{matrix}");
            let plan = fixture(
                name,
                &source,
                &project(
                    640,
                    360,
                    vec![image_clip(vec![])],
                    vec![dither(colours, matrix, 1)],
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
        ("blue-noise", dither(&EMBER, "blue_noise", 1)),
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
        assert_eq!(
            i16::from(cpu[3]) - i16::from(gpu[3]),
            i16::from(original_cpu[3]) - i16::from(original_gpu[3]),
            "effect must not add to inherited alpha sampling error at {index}"
        );
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
        for matrix in ["bayer8", "blue_noise"] {
            let name = &format!("{name}-{matrix}");
            let plan = fixture(
                name,
                &source,
                &project(
                    width,
                    height,
                    vec![image_clip(vec![palette(&EMBER, "gradient")])],
                    vec![dither(&EMBER, matrix, 1)],
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

fn save_temporal_metrics(name: &str, frames: &[(u128, RgbaImage)]) {
    let metrics = frames
        .windows(2)
        .map(|pair| {
            json!({"from_ns": pair[0].0, "to_ns": pair[1].0,
                "difference": compare_rgba(pair[0].1.as_raw(), pair[1].1.as_raw(), 0)})
        })
        .collect::<Vec<_>>();
    fs::write(
        artifact_directory(name).join("temporal.json"),
        serde_json::to_vec_pretty(&metrics).expect("serialize temporal metrics"),
    )
    .expect("save temporal metrics");
}

#[test]
fn gpu_stylization_temporal_stationary_and_moving_patterns_stay_output_anchored() {
    for moving in [false, true] {
        let name = if moving {
            "temporal-moving"
        } else {
            "temporal-stationary"
        };
        let source = RgbaImage::from_pixel(96, 64, Rgba([128, 128, 128, 255]));
        let mut clip = image_clip(vec![]);
        clip["transform"]["scale"] = json!({"base_value": {"x": 0.5, "y": 0.5}});
        if moving {
            clip["transform"]["position"] = json!({"base_value": {"x": 0.375, "y": 0.5},
                "keyframes": [
                    {"time": 0.0, "value": {"x": 0.375, "y": 0.5}, "interpolation": "linear"},
                    {"time": 1.0, "value": {"x": 0.625, "y": 0.5}, "interpolation": "linear"}]});
        }
        let plan = fixture(
            name,
            &source,
            &project(96, 64, vec![clip], vec![dither(&MONO, "bayer2", 1)]),
        );
        let Some(mut backends) = Backends::new(plan) else {
            return;
        };
        let times = [0, 250_000_000, 500_000_000, 750_000_000, 1_000_000_000];
        let frames = times
            .into_iter()
            .map(|time| (time, backends.render(name, time, 0)))
            .collect::<Vec<_>>();
        for (_, frame) in &frames {
            // All translations are whole pixels; this shared interior stays gray.
            for y in 20..44 {
                for x in 40..56 {
                    let value = if (x + y) % 2 == 0 { 0 } else { 255 };
                    assert_eq!(frame.get_pixel(x, y).0, [value, value, value, 255]);
                }
            }
        }
        if moving {
            assert_ne!(
                frames[0].1, frames[4].1,
                "fixture must include actual source movement"
            );
        } else {
            assert!(
                frames.windows(2).all(|pair| pair[0].1 == pair[1].1),
                "a stationary scene must not flicker"
            );
        }
        assert_eq!(
            frames[1].1,
            backends.render(name, times[1], 0),
            "random-access repeat"
        );
        save_temporal_metrics(name, &frames);
    }
}

#[test]
fn gpu_stylization_temporal_threshold_sweep_changes_only_monotone_bayer_coverage() {
    let name = "temporal-threshold-sweep";
    let source = RgbaImage::from_pixel(64, 48, Rgba([255, 255, 255, 255]));
    let mut clip = image_clip(vec![]);
    clip["opacity"] = json!({"base_value": 0.45, "keyframes": [
        {"time": 0.0, "value": 0.45, "interpolation": "linear"},
        {"time": 1.0, "value": 0.55, "interpolation": "linear"}]});
    let mut value = project(64, 48, vec![clip], vec![dither(&MONO, "bayer8", 1)]);
    value["output"]["background"] = json!("#000000");
    let Some(mut backends) = Backends::new(fixture(name, &source, &value)) else {
        return;
    };
    let frames = (0..=8)
        .map(|index| {
            let time = index * 125_000_000;
            (time, backends.render(name, time, 0))
        })
        .collect::<Vec<_>>();
    assert_ne!(frames[0].1, frames[8].1, "sweep must cross tone thresholds");
    for pair in frames.windows(2) {
        for (before, after) in pair[0].1.pixels().zip(pair[1].1.pixels()) {
            assert!(
                before[0] <= after[0],
                "increasing tone cannot reverse a fixed Bayer decision"
            );
        }
    }
    assert_eq!(
        frames[3].1,
        backends.render(name, frames[3].0, 0),
        "threshold rendering is stateless"
    );
    save_temporal_metrics(name, &frames);
}

#[test]
fn gpu_stylization_temporal_period_seam_is_continuous_for_animated_colors() {
    for mode in ["gradient", "rainbow"] {
        let name = format!("temporal-seam-{mode}");
        let mut effect = palette(&EMBER, mode);
        effect["period"] = json!(2.0);
        let source = rich_source(96, 64);
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(96, 64, vec![image_clip(vec![effect])], vec![]),
        )) else {
            return;
        };
        let times = [
            1_999_000_000,
            1_999_999_999,
            2_000_000_000,
            2_000_000_001,
            2_001_000_000,
        ];
        let frames = times
            .into_iter()
            .map(|time| (time, backends.render(&name, time, 1)))
            .collect::<Vec<_>>();
        let seam = compare_rgba(frames[1].1.as_raw(), frames[3].1.as_raw(), 1);
        assert!(
            seam.maximum_absolute_channel_error <= 1,
            "period seam: {seam:?}"
        );
        assert_eq!(
            frames[2].1,
            backends.render(&name, 0, 1),
            "exact cycle boundary"
        );
        assert_eq!(
            frames[0].1,
            backends.render(&name, times[0], 1),
            "nonsequential seam repeat"
        );
        save_temporal_metrics(&name, &frames);
    }
}

#[test]
fn gpu_stylization_half_alpha_mask_uses_cpu_half_up_byte_rounding() {
    for (alpha, expected) in [(1, 1), (3, 2), (255, 128)] {
        let name = format!("half-alpha-{alpha}");
        let source = RgbaImage::from_pixel(16, 8, Rgba([32, 64, 96, alpha]));
        let mut clip = image_clip(vec![]);
        clip["masks"] = json!([{"id": "half", "input": {"type": "shape",
            "geometry": {"type": "rectangle", "width": 16.0, "height": 8.0},
            "fill": "#ffffff"}, "operation": "subtract", "strength": 0.5}]);
        let Some(mut backends) =
            Backends::new(fixture(&name, &source, &project(16, 8, vec![clip], vec![])))
        else {
            return;
        };
        let (cpu, gpu, _) = backends.render_pair(&name, 0, 0);
        assert_eq!(cpu.get_pixel(8, 4)[3], expected, "CPU half-alpha {alpha}");
        assert_eq!(gpu.get_pixel(8, 4)[3], expected, "WGPU half-alpha {alpha}");
    }
}

fn scalar_fields(mut effect: Value, fields: &[&str]) -> Value {
    for field in fields {
        effect[*field] = json!({"base_value": effect[*field]});
    }
    effect
}

fn ascii_effect(glyph_style: &str, mode: &str, color_mode: &str) -> Value {
    scalar_fields(
        json!({"type": "ascii", "id": "ascii", "characters": " .:-=+*#%@",
        "edge_characters": "-|/\\", "glyph_style": glyph_style, "mode": mode,
        "color_mode": color_mode, "foreground": "#ffffff", "background": "#000000",
        "palette": EMBER, "invert": false, "amount": 1.0, "phase": 0.0,
        "cell_width": 8.0, "cell_height": 12.0, "edge_threshold": 0.15,
        "edge_strength": 1.0, "source_mix": 0.0}),
        &[
            "amount",
            "phase",
            "cell_width",
            "cell_height",
            "edge_threshold",
            "edge_strength",
            "source_mix",
        ],
    )
}

fn halftone_effect(mode: &str) -> Value {
    scalar_fields(
        json!({"type": "halftone", "id": "halftone", "cell_size": 6.0,
        "angle_degrees": 15.0, "softness": 0.5, "mode": mode,
        "foreground": "#ffffff", "background": "#000000", "invert": false,
        "amount": 1.0}),
        &["cell_size", "angle_degrees", "softness", "amount"],
    )
}

fn sort_effect(direction: &str) -> Value {
    scalar_fields(
        json!({"type": "pixel_sort", "id": "sort", "direction": direction,
        "order": "ascending", "lower_threshold": 0.15, "upper_threshold": 0.9,
        "segment_length": 32, "amount": 1.0}),
        &["lower_threshold", "upper_threshold", "amount"],
    )
}

fn crt_effect() -> Value {
    scalar_fields(
        json!({"type": "crt", "id": "crt", "amount": 1.0, "curvature": 0.08,
        "scanline_strength": 0.2, "scanline_spacing": 2.0, "mask_strength": 0.15,
        "mask_spacing": 1, "grain": 0.025, "jitter": 0.35, "flicker": 0.025,
        "rolling_strength": 0.06, "rolling_width": 0.12, "phase": 0.0, "seed": 7}),
        &[
            "amount",
            "curvature",
            "scanline_strength",
            "scanline_spacing",
            "mask_strength",
            "grain",
            "jitter",
            "flicker",
            "rolling_strength",
            "rolling_width",
            "phase",
        ],
    )
}

fn remaining_families() -> Vec<(&'static str, Value)> {
    vec![
        (
            "ascii-characters",
            ascii_effect("characters", "hybrid", "source"),
        ),
        (
            "ascii-geometric",
            ascii_effect("geometric", "fill", "palette"),
        ),
        ("halftone-luminance", halftone_effect("luminance")),
        ("halftone-source", halftone_effect("source")),
        ("halftone-rgb", halftone_effect("rgb")),
        ("sort-horizontal", sort_effect("horizontal")),
        ("sort-vertical", sort_effect("vertical")),
        ("crt", crt_effect()),
    ]
}

#[test]
#[ignore = "explicit 1080p/4K all-family correctness and resource validation"]
fn gpu_stylization_all_families_1080p_and_4k_resource_validation() {
    for (resolution, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        let source = rich_source(width, height);
        for (family, effect) in remaining_families() {
            verify_resolution_case(&format!("{resolution}-{family}"), &source, effect);
        }
    }
}

#[test]
#[ignore = "explicit CRT transparent-border quantization at 1080p/4K"]
fn gpu_stylization_crt_1080p_and_4k_transparent_border_parity() {
    for (resolution, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        verify_resolution_case(
            &format!("{resolution}-crt"),
            &rich_source(width, height),
            crt_effect(),
        );
    }
}

fn verify_resolution_case(name: &str, source: &RgbaImage, effect: Value) {
    let (width, height) = source.dimensions();
    let Some(mut backends) = Backends::new(fixture(
        name,
        source,
        &project(width, height, vec![image_clip(vec![])], vec![effect]),
    )) else {
        return;
    };
    let (cpu, gpu, difference) = backends.render_pair(name, 250_000_000, 3);
    assert_eq!(cpu.dimensions(), (width, height));
    assert_eq!(gpu.dimensions(), (width, height));
    assert!(
        difference.maximum_absolute_channel_error <= 3,
        "{name}: {difference:?}"
    );
    assert_ne!(&cpu, source, "{name} must exercise actual stylization");
    let resources = backends.gpu.resource_estimates();
    eprintln!(
        "STYLIZATION_RESOURCES fixture={name} persistent_bytes={} working_bytes={} staging_bytes={}",
        resources.total_persistent_bytes,
        resources.working_texture_bytes,
        resources.total_staging_bytes
    );
}

#[test]
fn gpu_stylization_temporal_remaining_families_amount_and_random_access() {
    let source = rich_source(96, 64);
    for (family, mut effect) in remaining_families() {
        let name = format!("temporal-amount-{family}");
        effect["amount"] = json!({"base_value": 0.0, "keyframes": [
            {"time": 0.0, "value": 0.0, "interpolation": "linear"},
            {"time": 1.0, "value": 1.0, "interpolation": "linear"}]});
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(96, 64, vec![image_clip(vec![effect])], vec![]),
        )) else {
            return;
        };
        let times = [0, 250_000_000, 500_000_000, 750_000_000, 1_000_000_000];
        let frames = times
            .into_iter()
            .map(|time| (time, backends.render(&name, time, 3)))
            .collect::<Vec<_>>();
        assert_eq!(frames[0].1, source, "{family} amount zero is identity");
        assert_ne!(frames[4].1, source, "{family} full amount applies effect");
        assert_eq!(
            frames[2].1,
            backends.render(&name, times[2], 3),
            "{family} out-of-order repeat"
        );
        save_temporal_metrics(&name, &frames);
    }
}

#[test]
fn gpu_stylization_temporal_crt_seeded_noise_period_seam_and_random_access() {
    let name = "temporal-crt-period";
    let source = rich_source(96, 64);
    let mut effect = crt_effect();
    effect["period"] = json!(2.0);
    let Some(mut backends) = Backends::new(fixture(
        name,
        &source,
        &project(96, 64, vec![image_clip(vec![])], vec![effect]),
    )) else {
        return;
    };
    let times = [
        0,
        1_999_000_000,
        1_999_999_999,
        2_000_000_000,
        2_000_000_001,
        2_001_000_000,
    ];
    let frames = times
        .into_iter()
        .map(|time| (time, backends.render(name, time, 3)))
        .collect::<Vec<_>>();
    assert_eq!(frames[0].1, frames[3].1, "CRT exact periodic endpoint");
    let seam = compare_rgba(frames[2].1.as_raw(), frames[4].1.as_raw(), 1);
    assert!(
        seam.maximum_absolute_channel_error <= 1,
        "CRT noise seam: {seam:?}"
    );
    assert_eq!(
        frames[1].1,
        backends.render(name, times[1], 3),
        "CRT out-of-order repeat"
    );
    save_temporal_metrics(name, &frames);
}

#[test]
fn gpu_stylization_temporal_remaining_families_static_controls_and_moving_subjects() {
    let source = rich_source(96, 64);
    for moving in [false, true] {
        for (family, mut effect) in remaining_families() {
            let name = format!(
                "temporal-{}-{family}",
                if moving { "motion" } else { "static" }
            );
            if family == "crt" {
                // Isolate source motion from the intentionally animated analog controls.
                for property in ["grain", "jitter", "flicker", "rolling_strength"] {
                    effect[property] = json!({"base_value": 0.0});
                }
            }
            let mut clip = image_clip(vec![]);
            if moving {
                clip["transform"]["position"] = json!({"base_value": {"x": 0.5, "y": 0.5},
                    "keyframes": [
                        {"time": 0.0, "value": {"x": 0.5, "y": 0.5}, "interpolation": "linear"},
                        {"time": 1.0, "value": {"x": 0.625, "y": 0.5}, "interpolation": "linear"}]});
            }
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(96, 64, vec![clip], vec![effect]),
            )) else {
                return;
            };
            let times = [0, 250_000_000, 500_000_000, 750_000_000, 1_000_000_000];
            let frames = times
                .into_iter()
                .map(|time| (time, backends.render(&name, time, 3)))
                .collect::<Vec<_>>();
            if moving {
                assert_ne!(
                    frames[0].1, frames[4].1,
                    "{family} moving fixture must change"
                );
            } else {
                assert!(
                    frames.windows(2).all(|pair| pair[0].1 == pair[1].1),
                    "{family} stationary control must not flicker"
                );
            }
            assert_eq!(
                frames[1].1,
                backends.render(&name, times[1], 3),
                "{family} random-access source motion"
            );
            save_temporal_metrics(&name, &frames);
        }
    }
}

#[test]
fn gpu_stylization_temporal_ascii_and_halftone_tonal_cell_threshold_sweeps() {
    for (family, effect) in [
        ("ascii", ascii_effect("characters", "fill", "monochrome")),
        ("halftone-soft", halftone_effect("luminance")),
        ("halftone-crisp", {
            let mut effect = halftone_effect("luminance");
            effect["softness"] = json!({"base_value": 0.0});
            effect
        }),
    ] {
        let name = format!("temporal-threshold-{family}");
        // Non-multiple dimensions exercise area analysis in partial border cells.
        let source = RgbaImage::from_pixel(98, 66, Rgba([255, 255, 255, 255]));
        let mut clip = image_clip(vec![]);
        clip["opacity"] = json!({"base_value": 0.4, "keyframes": [
            {"time": 0.0, "value": 0.4, "interpolation": "linear"},
            {"time": 1.0, "value": 0.6, "interpolation": "linear"}]});
        let mut value = project(98, 66, vec![clip], vec![effect]);
        value["output"]["background"] = json!("#000000");
        let Some(mut backends) = Backends::new(fixture(&name, &source, &value)) else {
            return;
        };
        let frames = (0..=8)
            .map(|index| {
                let time = index * 125_000_000;
                (time, backends.render(&name, time, 3))
            })
            .collect::<Vec<_>>();
        assert_ne!(
            frames[0].1, frames[8].1,
            "{family} sweep must change cell coverage"
        );
        assert_eq!(
            frames[3].1,
            backends.render(&name, frames[3].0, 3),
            "{family} thresholds are stateless"
        );
        save_temporal_metrics(&name, &frames);
    }
}

#[test]
fn gpu_stylization_temporal_sort_ties_segments_and_threshold_membership_are_stable() {
    let red = Rgba([19, 0, 0, 255]);
    let blue = Rgba([0, 0, 54, 255]);
    // Byte luminance keys are exactly equal: 54*19 == 19*54.
    let gray = |tone| Rgba([tone, tone, tone, 255]);
    let pixels = [
        gray(128),
        red,
        blue,
        gray(64),
        gray(0),
        gray(64),
        blue,
        red,
        gray(128),
        gray(0),
    ];
    for direction in ["horizontal", "vertical"] {
        let (width, height) = if direction == "horizontal" {
            (10, 2)
        } else {
            (2, 10)
        };
        let source = RgbaImage::from_fn(width, height, |x, y| {
            pixels[if direction == "horizontal" { x } else { y } as usize]
        });
        for (order, expected) in [
            (
                "ascending",
                [
                    red,
                    blue,
                    gray(64),
                    gray(128),
                    gray(0),
                    blue,
                    red,
                    gray(64),
                    gray(128),
                    gray(0),
                ],
            ),
            (
                "descending",
                [
                    gray(128),
                    gray(64),
                    red,
                    blue,
                    gray(0),
                    gray(64),
                    blue,
                    red,
                    gray(128),
                    gray(0),
                ],
            ),
        ] {
            let name = format!("temporal-sort-ties-{direction}-{order}");
            let mut effect = sort_effect(direction);
            effect["order"] = json!(order);
            effect["segment_length"] = json!(8);
            effect["upper_threshold"] = json!({"base_value": 1.0});
            effect["lower_threshold"] = json!({"base_value": 0.015,
                "keyframes": [
                    {"time": 0.0, "value": 0.015, "interpolation": "linear"},
                    {"time": 1.0, "value": 0.016, "interpolation": "linear"}]});
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(width, height, vec![image_clip(vec![])], vec![effect]),
            )) else {
                return;
            };
            let first = backends.render(&name, 0, 0);
            for line in 0..2 {
                let observed = (0..10)
                    .map(|position| {
                        let (x, y) = if direction == "horizontal" {
                            (position, line)
                        } else {
                            (line, position)
                        };
                        *first.get_pixel(x, y)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    observed, expected,
                    "stable equal-tone colors and ineligible separators in {direction}/{order} line {line}"
                );
            }
            let frames = [0, 500_000_000, 750_000_000, 1_000_000_000]
                .into_iter()
                .map(|time| (time, backends.render(&name, time, 0)))
                .collect::<Vec<_>>();
            assert_eq!(
                frames[3].1, source,
                "ineligible tie colors split remaining runs"
            );
            assert_eq!(
                first,
                backends.render(&name, 0, 0),
                "threshold revisit restores exact tie order"
            );
            save_temporal_metrics(&name, &frames);
        }
    }
}

#[test]
fn gpu_stylization_temporal_ascii_color_periods_repeat_with_cached_atlases() {
    let source = rich_source(96, 64);
    for color_mode in ["palette", "rainbow"] {
        for global in [false, true] {
            let name = format!(
                "temporal-ascii-period-{color_mode}-{}",
                if global { "global" } else { "clip" }
            );
            let mut effect = ascii_effect("characters", "hybrid", color_mode);
            effect["period"] = json!(2.0);
            let (clip_effects, post_effects) = if global {
                (vec![], vec![effect])
            } else {
                (vec![effect], vec![])
            };
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(96, 64, vec![image_clip(clip_effects)], post_effects),
            )) else {
                return;
            };
            let later = backends.render(&name, 2_250_000_000, 3);
            let middle = backends.render(&name, 900_000_000, 3);
            let early = backends.render(&name, 250_000_000, 3);
            assert_eq!(
                early, later,
                "{color_mode} generated color repeats after period"
            );
            assert_ne!(
                early, middle,
                "ASCII atlas caching must preserve animated color"
            );
            let frames = [
                0,
                1_999_000_000,
                1_999_999_999,
                2_000_000_000,
                2_000_000_001,
                2_001_000_000,
            ]
            .into_iter()
            .map(|time| (time, backends.render(&name, time, 3)))
            .collect::<Vec<_>>();
            assert_eq!(frames[0].1, frames[3].1, "ASCII exact period boundary");
            let seam = compare_rgba(frames[2].1.as_raw(), frames[4].1.as_raw(), 1);
            assert!(
                seam.maximum_absolute_channel_error <= 1,
                "ASCII color seam {color_mode}: {seam:?}"
            );
            assert_eq!(
                early,
                backends.render(&name, 250_000_000, 3),
                "ASCII color nonsequential repeat"
            );
            save_temporal_metrics(&name, &frames);
        }
    }
}

#[test]
fn gpu_stylization_halftone_analysis_scale_rotation_and_transparent_border_parity() {
    let source = RgbaImage::from_fn(98, 66, |x, y| {
        if (x + y) % 7 == 0 {
            Rgba([255, 0, 255, 0])
        } else {
            Rgba([
                (x * 2) as u8,
                (y * 3) as u8,
                ((x + y) * 3 % 256) as u8,
                if (x + y) % 3 == 0 { 64 } else { 255 },
            ])
        }
    });
    for mode in ["luminance", "source", "rgb"] {
        for size in [2.0, 6.0, 64.0] {
            for angle in [0.0, 15.0, 45.0, 90.0] {
                let name = format!("halftone-cells-{mode}-{size}-{angle}");
                let mut effect = halftone_effect(mode);
                effect["cell_size"] = json!({"base_value": size});
                effect["angle_degrees"] = json!({"base_value": angle});
                let Some(mut backends) = Backends::new(fixture(
                    &name,
                    &source,
                    &project(98, 66, vec![image_clip(vec![])], vec![effect]),
                )) else {
                    return;
                };
                let output = backends.render(&name, 0, 3);
                for (original, rendered) in source.pixels().zip(output.pixels()) {
                    assert_eq!(
                        original[3], rendered[3],
                        "halftone cell must preserve per-pixel alpha"
                    );
                    if original[3] == 0 {
                        assert_eq!(rendered.0, [0, 0, 0, 0], "hidden RGB cannot create dots");
                    }
                }
            }
        }
    }
}

#[test]
fn gpu_stylization_remaining_families_clip_global_masks_matte_and_order() {
    let source = rich_source(96, 64);
    for (family, effect) in remaining_families() {
        let mut plain = Vec::new();
        for global in [false, true] {
            let name = format!("scope-{family}-{}", if global { "global" } else { "clip" });
            let (clip_effects, post_effects) = if global {
                (vec![], vec![effect.clone()])
            } else {
                (vec![effect.clone()], vec![])
            };
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(96, 64, vec![image_clip(clip_effects)], post_effects),
            )) else {
                return;
            };
            plain.push(backends.render(&name, 250_000_000, 3));
        }
        assert_eq!(
            plain[0], plain[1],
            "{family} equivalent plain clip/global input"
        );
        let name = format!("masked-matte-{family}");
        let mut clip = image_clip(vec![effect.clone()]);
        clip["masks"] = json!([{"id": "rect", "input": {"type": "shape",
            "geometry": {"type": "rectangle", "width": 78.0, "height": 52.0},
            "fill": "#ffffff"}}]);
        clip["matte"] = json!({"source_layer": "matte", "mode": "alpha", "invert": false});
        let matte = json!({"id": "matte", "source": {"type": "shape",
            "geometry": {"type": "rectangle", "width": 96.0, "height": 64.0},
            "fill": "#ffffff80"}, "start": 0.0, "duration": 6.0, "layer": 1,
            "visible": false, "opacity": {"base_value": 1.0}});
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(96, 64, vec![clip, matte], vec![]),
        )) else {
            return;
        };
        let output = backends.render(&name, 250_000_000, 3);
        assert_eq!(
            output.get_pixel(0, 0)[3],
            0,
            "{family} masked canvas corner"
        );
        assert!(
            output.pixels().any(|pixel| pixel[3] == 128),
            "{family} partial matte alpha"
        );
        let mut ordered = Vec::new();
        let inverse = palette(&["#ffffff", "#000000"], "gradient");
        for (order, effects) in [
            ("before", vec![effect.clone(), inverse.clone()]),
            ("after", vec![inverse.clone(), effect.clone()]),
        ] {
            let name = format!("order-{family}-{order}");
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(96, 64, vec![image_clip(vec![])], effects),
            )) else {
                return;
            };
            ordered.push(backends.render(&name, 250_000_000, 3));
        }
        assert_ne!(
            ordered[0], ordered[1],
            "{family} preserves noncommuting authored effect order"
        );
    }
}

#[test]
fn gpu_stylization_ascii_min_max_cells_custom_characters_and_font() {
    let source = rich_source(130, 130);
    for (name, width, height, custom_font) in [
        ("ascii-min-cells", 2.0, 2.0, false),
        ("ascii-max-cells", 64.0, 128.0, false),
        ("ascii-custom-glyphs-font", 7.0, 11.0, true),
    ] {
        let mut effect = ascii_effect("characters", "hybrid", "source");
        effect["cell_width"] = json!({"base_value": width});
        effect["cell_height"] = json!({"base_value": height});
        effect["source_mix"] = json!({"base_value": 0.2});
        effect["characters"] = json!(" .oO@░▒▓█");
        let mut value = project(130, 130, vec![image_clip(vec![effect])], vec![]);
        if custom_font {
            value["visual"]["clips"][0]["effects"][0]["font"] = json!("custom-font");
            let font = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/DejaVuSans.ttf");
            value["assets"]
                .as_array_mut()
                .expect("assets")
                .push(json!({"id": "custom-font",
                "type": "font", "source": font}));
        }
        let Some(mut backends) = Backends::new(fixture(name, &source, &value)) else {
            return;
        };
        let output = backends.render(name, 0, 3);
        assert_ne!(output, source, "custom/partial glyphs must actually render");
        assert!(
            output.pixels().all(|pixel| pixel[3] == 255),
            "glyph background preserves source alpha"
        );
    }
}

#[test]
fn gpu_stylization_half_opacity_compositor_uses_cpu_half_up_byte_rounding() {
    let name = "half-opacity-compositor";
    let source = RgbaImage::from_pixel(16, 8, Rgba([255, 255, 255, 255]));
    let mut clip = image_clip(vec![]);
    clip["opacity"] = json!({"base_value": 0.5});
    let mut value = project(16, 8, vec![clip], vec![]);
    value["output"]["background"] = json!("#000000");
    let Some(mut backends) = Backends::new(fixture(name, &source, &value)) else {
        return;
    };
    let output = backends.render(name, 0, 0);
    assert!(
        output.pixels().all(|pixel| pixel.0 == [128, 128, 128, 255]),
        "byte127.5 must round to128 before downstream cell/threshold analysis"
    );
}

#[test]
fn gpu_stylization_blue_noise_is_seeded_spatial_and_preserves_palette_detail() {
    let source = rich_source(320, 180);
    let mut outputs = Vec::new();
    for seed in [0, 37, u32::MAX] {
        let name = format!("blue-noise-{seed}");
        let mut effect = dither(&EMBER, "blue_noise", 1);
        effect["seed"] = json!(seed);
        let plan = fixture(
            &name,
            &source,
            &project(320, 180, vec![image_clip(vec![])], vec![effect]),
        );
        let Some(mut backends) = Backends::new(plan) else {
            return;
        };
        let output = backends.render(&name, 2_000_000_000, 0);
        assert_eq!(
            output,
            backends.render(&name, 0, 0),
            "pattern must not change with frame time"
        );
        assert!(
            output.pixels().filter(|p| p[0] > 100).count() > 1000,
            "well-exposed source must retain highlights"
        );
        outputs.push(output);
    }
    assert_ne!(outputs[0], outputs[1]);
    assert_ne!(outputs[0], outputs[2]);
}

#[test]
fn gpu_stylization_chromatic_quantization_preserves_palette_colors_and_source_detail() {
    let colours = ["#ff0000", "#00ff00", "#0000ff", "#ffffff", "#000000"];
    let source = rich_source(320, 180);
    for mode in ["nearest", "nearest_rgb", "nearest_hue", "nearest_oklab"] {
        for matrix in ["map", "bayer8", "blue_noise"] {
            let name = format!("chromatic-{mode}-{matrix}");
            let effect = if matrix == "map" {
                palette(&colours, mode)
            } else {
                let mut effect = dither(&colours, matrix, 1);
                effect["mode"] = json!(mode);
                effect
            };
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(320, 180, vec![image_clip(vec![])], vec![effect]),
            )) else {
                return;
            };
            let output = backends.render(&name, 0, 0);
            assert_eq!(output, backends.render(&name, 2_000_000_000, 0));
            assert!(output.pixels().any(|p| p[0] == 255));
            assert!(output.pixels().any(|p| p[0] == 0));
            if mode == "nearest_oklab" && matrix == "map" {
                assert_eq!(output.get_pixel(137, 110).0, [255, 0, 0, 255]);
                assert_eq!(output.get_pixel(96, 110).0, [0, 0, 0, 255]);
            }
        }
    }
    for (mode, expected) in [
        ("nearest_rgb", [50, 50, 50, 255]),
        ("nearest_hue", [255, 0, 0, 255]),
    ] {
        let source = RgbaImage::from_pixel(4, 4, Rgba([50, 0, 0, 255]));
        let name = format!("chromatic-literal-{mode}");
        let effect = palette(&["#323232", "#ff0000"], mode);
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(4, 4, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        assert!(
            backends
                .render(&name, 0, 0)
                .pixels()
                .all(|p| p.0 == expected)
        );
    }
}

#[test]
fn gpu_stylization_oklab_lightness_and_exact_matches_have_literal_results() {
    for (label, input, colours, expected) in [
        (
            "lightness",
            [104, 104, 104, 255],
            ["#000000", "#ffffff"],
            [255, 255, 255, 255],
        ),
        (
            "rounded-tie",
            [1, 255, 255, 128],
            ["#00ffff", "#01ffff"],
            [1, 255, 255, 128],
        ),
    ] {
        for matrix in ["map", "blue_noise"] {
            let source = RgbaImage::from_pixel(32, 32, Rgba(input));
            let name = format!("oklab-{label}-{matrix}");
            let mut effect = if matrix == "map" {
                palette(&colours, "nearest_oklab")
            } else {
                dither(&colours, matrix, 1)
            };
            effect["mode"] = json!("nearest_oklab");
            if label == "lightness" {
                effect["strength"] = json!({"base_value": 0.0});
            }
            if matrix == "map" {
                effect.as_object_mut().unwrap().remove("strength");
            }
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(32, 32, vec![image_clip(vec![])], vec![effect]),
            )) else {
                return;
            };
            assert!(
                backends
                    .render(&name, 0, 0)
                    .pixels()
                    .all(|p| p.0 == expected)
            );
        }
    }
}

#[test]
fn gpu_stylization_oklab_matches_three_palettes_with_visible_figure() {
    let source = rich_source(320, 180);
    let mono = [
        "#000000", "#202020", "#404040", "#606060", "#808080", "#a0a0a0", "#c0c0c0", "#e0e0e0",
        "#ffffff",
    ];
    for (label, colours) in [
        ("mono", &mono[..]),
        ("ember", &EMBER[..]),
        ("ocean", &OCEAN[..]),
    ] {
        for matrix in ["map", "bayer8", "blue_noise"] {
            let name = format!("oklab-{label}-{matrix}");
            let mut effect = if matrix == "map" {
                palette(colours, "nearest_oklab")
            } else {
                dither(colours, matrix, 1)
            };
            effect["mode"] = json!("nearest_oklab");
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(320, 180, vec![image_clip(vec![])], vec![effect]),
            )) else {
                return;
            };
            let output = backends.render(&name, 0, 0);
            let mean = |x0: u32, x1: u32, y0: u32, y1: u32| -> u64 {
                (y0..y1)
                    .flat_map(|y| (x0..x1).map(move |x| (x, y)))
                    .map(|(x, y)| {
                        let p = output.get_pixel(x, y);
                        u64::from(p[0]) + u64::from(p[1]) + u64::from(p[2])
                    })
                    .sum()
            };
            assert!(
                mean(130, 140, 105, 115) > mean(90, 100, 105, 115),
                "{name}: figure must contrast with its mountain background"
            );
            assert!(
                mean(238, 248, 38, 48) > 54000,
                "{name}: moon highlight must survive"
            );
        }
    }
}

#[test]
fn gpu_stylization_nonuniform_stops_match_literal_ramp_and_animated_scenes() {
    let source = RgbaImage::from_fn(256, 32, |x, _| Rgba([x as u8, x as u8, x as u8, 255]));
    for mode in [
        "gradient",
        "nearest",
        "bayer2",
        "bayer4",
        "bayer8",
        "blue_noise",
    ] {
        let name = format!("nonuniform-ramp-{mode}");
        let mut effect = if matches!(mode, "gradient" | "nearest") {
            palette(&["#000000", "#ff0000", "#ffffff"], mode)
        } else {
            dither(&["#000000", "#ff0000", "#ffffff"], mode, 1)
        };
        effect["stops"] = json!([0.0, 64.0 / 255.0, 1.0]);
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(256, 32, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        let output = backends.render(&name, 0, 0);
        assert_eq!(output.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(output.get_pixel(64, 0).0, [255, 0, 0, 255]);
        assert_eq!(output.get_pixel(255, 0).0, [255; 4]);
        if mode == "gradient" {
            assert_eq!(output.get_pixel(32, 0).0, [128, 0, 0, 255]);
        } else if mode == "nearest" {
            assert_eq!(output.get_pixel(32, 0).0, [255, 0, 0, 255]);
        } else if mode != "blue_noise" {
            assert_eq!(
                (0..32)
                    .filter(|&y| output.get_pixel(32, y)[0] == 255)
                    .count(),
                16
            );
        }
    }
    // A blue-noise permutation balances a whole tile, not each individual column.
    let source = RgbaImage::from_pixel(32, 32, Rgba([32, 32, 32, 255]));
    let name = "nonuniform-blue-noise-coverage";
    let mut effect = dither(&["#000000", "#ff0000", "#ffffff"], "blue_noise", 1);
    effect["stops"] = json!([0.0, 64.0 / 255.0, 1.0]);
    let Some(mut backends) = Backends::new(fixture(
        name,
        &source,
        &project(32, 32, vec![image_clip(vec![])], vec![effect]),
    )) else {
        return;
    };
    let output = backends.render(name, 0, 0);
    assert_eq!(output.pixels().filter(|p| p[0] == 255).count(), 512);
    let source = rich_source(320, 180);
    for (label, colours) in [
        ("mono", &MONO_FIVE[..]),
        ("ember", &EMBER[..]),
        ("ocean", &OCEAN[..]),
    ] {
        let name = format!("nonuniform-{label}-blue_noise");
        let mut effect = dither(colours, "blue_noise", 1);
        effect["stops"] = json!([0.0, 0.12, 0.35, 0.7, 1.0]);
        effect["period"] = json!(2.0);
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(320, 180, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        let first = backends.render(&name, 0, 0);
        backends.render(&name, 777_000_000, 0);
        assert_eq!(first, backends.render(&name, 2_000_000_000, 0));
        assert_eq!(first, backends.render(&name, 0, 0));
        let sum = |x0: u32| -> u32 {
            (105..115)
                .flat_map(|y| (x0..x0 + 10).map(move |x| (x, y)))
                .map(|(x, y)| {
                    first.get_pixel(x, y).0[..3]
                        .iter()
                        .map(|&c| u32::from(c))
                        .sum::<u32>()
                })
                .sum()
        };
        assert!(
            sum(130) > sum(90),
            "{name}: figure must retain tonal contrast"
        );
    }
}

#[test]
fn gpu_stylization_oklab_interpolation_preserves_stops_and_animated_colour_parity() {
    let source = RgbaImage::from_fn(256, 32, |x, _| Rgba([x as u8, x as u8, x as u8, 255]));
    for custom in [false, true] {
        let name = format!("oklab-interpolation-ramp-{custom}");
        let mut effect = palette(&["#ff0000", "#0000ff", "#ffffff"], "gradient");
        effect["interpolation"] = json!("oklab");
        if custom {
            effect["stops"] = json!([0.0, 64.0 / 255.0, 1.0]);
        }
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(256, 32, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        let output = backends.render(&name, 0, 0);
        assert_eq!(output.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(output.get_pixel(255, 0).0, [255; 4]);
        if custom {
            assert_eq!(output.get_pixel(64, 0).0, [0, 0, 255, 255]);
            assert!((81..=87).contains(&output.get_pixel(32, 0)[1]));
        }
    }
    let source = rich_source(320, 180);
    for matrix in ["map", "bayer8", "blue_noise"] {
        let name = format!("oklab-interpolation-animated-{matrix}");
        let mut effect = if matrix == "map" {
            palette(&EMBER, "gradient")
        } else {
            dither(&EMBER, matrix, 1)
        };
        effect["interpolation"] = json!("oklab");
        effect["stops"] = json!([0.0, 0.12, 0.35, 0.7, 1.0]);
        effect["period"] = json!(2.0);
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(320, 180, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        let first = backends.render(&name, 0, 0);
        assert_ne!(first, backends.render(&name, 777_000_000, 0));
        assert_eq!(first, backends.render(&name, 2_000_000_000, 0));
        assert_eq!(first, backends.render(&name, 0, 0));
    }
}

#[test]
#[ignore = "explicit 1080p/4K chromatic palette correctness and resource validation"]
fn gpu_stylization_chromatic_1080p_and_4k_match_cpu() {
    for (resolution, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        let source = rich_source(width, height);
        for mode in ["nearest_rgb", "nearest_hue", "nearest_oklab"] {
            let name = format!("{resolution}-{mode}-blue_noise");
            let mut effect = dither(&EMBER, "blue_noise", 1);
            effect["mode"] = json!(mode);
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(width, height, vec![image_clip(vec![])], vec![effect]),
            )) else {
                return;
            };
            let output = backends.render(&name, 0, 0);
            assert_eq!(output.dimensions(), (width, height));
            assert!(output.pixels().all(|p| p[3] == 255));
            assert!(output.pixels().any(|p| p[0] == 255));
            assert!(output.pixels().any(|p| p[0] == 8));
        }
    }
}

#[test]
#[ignore = "explicit 1080p/4K nonuniform tonal correctness and resource validation"]
fn gpu_stylization_nonuniform_stops_1080p_and_4k_match_cpu() {
    for (resolution, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        let source = rich_source(width, height);
        let name = format!("{resolution}-nonuniform-blue-noise");
        let mut effect = dither(&EMBER, "blue_noise", 1);
        effect["stops"] = json!([0.0, 0.12, 0.35, 0.7, 1.0]);
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(width, height, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        backends.render(&name, 0, 0);
        assert!(backends.gpu.resource_estimates().total_persistent_bytes < 512 * 1024 * 1024);
    }
}

#[test]
#[ignore = "explicit 1080p/4K Oklab gradient correctness and resource validation"]
fn gpu_stylization_oklab_interpolation_1080p_and_4k_match_cpu() {
    for (resolution, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        let source = rich_source(width, height);
        let name = format!("{resolution}-oklab-gradient");
        let mut effect = palette(&EMBER, "gradient");
        effect["interpolation"] = json!("oklab");
        effect["stops"] = json!([0.0, 0.12, 0.35, 0.7, 1.0]);
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(width, height, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        backends.render(&name, 0, 0);
        assert!(backends.gpu.resource_estimates().total_persistent_bytes < 512 * 1024 * 1024);
    }
}

#[test]
#[ignore = "explicit 1080p/4K prepared quantization input correctness and resources"]
fn gpu_stylization_input_tone_1080p_and_4k_match_cpu() {
    for (resolution, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        let source = rich_source(width, height);
        let name = format!("{resolution}-input-tone-blue-noise");
        let mut effect = dither(&EMBER, "blue_noise", 1);
        effect["input_exposure"] = json!({"base_value": 0.5});
        effect["input_gamma"] = json!({"base_value": 1.5});
        effect["amount"] = json!({"base_value": 0.5});
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(width, height, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        backends.render(&name, 0, 0);
        let resources = backends.gpu.resource_estimates();
        eprintln!(
            "INPUT_TONE_RESOURCES {resolution} persistent={} staging={}",
            resources.total_persistent_bytes, resources.total_staging_bytes
        );
        assert!(resources.total_persistent_bytes < 512 * 1024 * 1024);
    }
}

#[test]
#[ignore = "explicit 1080p/4K integer detail preparation correctness and resources"]
fn gpu_stylization_input_detail_1080p_and_4k_match_cpu() {
    for (resolution, width, height, radius, detail) in [
        ("1080p-contrast", 1920, 1080, 8.0, 0.5),
        ("4k-detail", 3840, 2160, 1.0, 1.5),
    ] {
        let source = rich_source(width, height);
        let name = format!("{resolution}-input-detail-blue-noise");
        let mut effect = dither(&EMBER, "blue_noise", 1);
        effect["input_detail"] = json!({"base_value": detail});
        effect["input_detail_radius"] = json!({"base_value": radius});
        effect["input_gamma"] = json!({"base_value": 1.5});
        effect["amount"] = json!({"base_value": 0.5});
        let Some(mut backends) = Backends::new(fixture(
            &name,
            &source,
            &project(width, height, vec![image_clip(vec![])], vec![effect]),
        )) else {
            return;
        };
        backends.render(&name, 0, 0);
        let resources = backends.gpu.resource_estimates();
        eprintln!(
            "INPUT_DETAIL_RESOURCES {resolution} persistent={} staging={}",
            resources.total_persistent_bytes, resources.total_staging_bytes
        );
        assert!(resources.total_persistent_bytes < 512 * 1024 * 1024);
    }
}

#[test]
fn gpu_stylization_channel_levels_preserve_detail_and_match_cpu_exactly() {
    let source = rich_source(320, 180);
    for levels in [2, 4, 8, 256] {
        for matrix in ["map", "bayer8", "blue_noise"] {
            let name = format!("channels-{levels}-{matrix}");
            let mut effect = if matrix == "map" {
                palette(&EMBER, "rgb_channels")
            } else {
                dither(&EMBER, matrix, 1)
            };
            effect["mode"] = json!("rgb_channels");
            effect["levels"] = json!(levels);
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(320, 180, vec![image_clip(vec![])], vec![effect]),
            )) else {
                return;
            };
            let output = backends.render(&name, 0, 0);
            assert_eq!(output, backends.render(&name, 2_000_000_000, 0));
            let values: Vec<u8> = (0..levels)
                .map(|i| ((i * 255 + (levels - 1) / 2) / (levels - 1)) as u8)
                .collect();
            assert!(
                output
                    .pixels()
                    .all(|p| p[3] == 255 && p.0[..3].iter().all(|c| values.contains(c)))
            );
            assert!(output.pixels().any(|p| p[0] >= 240));
            assert!(output.pixels().any(|p| p[0] == 0));
            if levels == 256 {
                assert_eq!(output, source);
            }
        }
    }
}

#[test]
#[ignore = "explicit 1080p/4K channel quantization correctness and resources"]
fn gpu_stylization_channel_levels_1080p_and_4k() {
    for (resolution, width, height) in [("1080p", 1920, 1080), ("4k", 3840, 2160)] {
        let source = rich_source(width, height);
        for levels in [2, 256] {
            let name = format!("{resolution}-channels-{levels}-blue_noise");
            let mut effect = dither(&EMBER, "blue_noise", 1);
            effect["mode"] = json!("rgb_channels");
            effect["levels"] = json!(levels);
            let Some(mut backends) = Backends::new(fixture(
                &name,
                &source,
                &project(width, height, vec![image_clip(vec![])], vec![effect]),
            )) else {
                return;
            };
            let output = backends.render(&name, 0, 0);
            assert!(output.pixels().all(|p| p[3] == 255));
            if levels == 256 {
                assert_eq!(output, source);
            } else {
                assert!(
                    output
                        .pixels()
                        .all(|p| p.0[..3].iter().all(|c| [0, 255].contains(c)))
                );
            }
        }
    }
}
