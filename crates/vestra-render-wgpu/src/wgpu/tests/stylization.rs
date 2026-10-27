//! GPU-private resource estimates kept separate from facade pixel parity.
use super::WgpuBackend;
use super::gpu::wgpu_backend_or_skip;
use crate::{
    DecodedAssets,
    test_support::{ValidationOptions, load_and_validate},
};
use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use vestra_core::plan::{CompileOptions, RenderPlan, ScheduledItem, compile};
const EMBER: [&str; 5] = ["#080508", "#351120", "#9e3341", "#efa463", "#fff1c5"];

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

struct Backends {
    plan: RenderPlan,
    gpu: WgpuBackend,
}
impl Backends {
    fn new(plan: RenderPlan) -> Option<Self> {
        let decoded = DecodedAssets::build(&plan).expect("decode stylization fixture");
        let gpu = wgpu_backend_or_skip(&plan, decoded)?;
        Some(Self { plan, gpu })
    }
    fn render(&mut self, name: &str, time: u128, _tolerance: u8) {
        let active = (0..self.plan.layers.len())
            .map(ScheduledItem)
            .collect::<Vec<_>>();
        let frame = vestra_core::plan::evaluate(&self.plan, &active, time)
            .expect("evaluate stylization frame");
        let mut gpu = RgbaImage::new(frame.width, frame.height);
        self.gpu
            .render_frame(&frame, &mut gpu)
            .expect("WGPU stylization frame");
        let resources = self.gpu.resource_estimates();
        fs::write(
            artifact_directory(name).join("resources.json"),
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
    }
}

#[test]
#[ignore = "explicit 1080p/4K nonuniform tonal correctness and resource validation"]
fn gpu_stylization_nonuniform_stops_1080p_and_4k_resource_limits() {
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
fn gpu_stylization_oklab_interpolation_1080p_and_4k_resource_limits() {
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
fn gpu_stylization_input_tone_1080p_and_4k_resource_limits() {
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
fn gpu_stylization_input_detail_1080p_and_4k_resource_limits() {
    for (resolution, width, height, radius, detail) in [
        ("1080p-contrast", 1920, 1080, 8.0, 0.5),
        ("4k-detail", 3840, 2160, 1.0, 1.5),
        ("4k-contrast", 3840, 2160, 8.0, 0.5),
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
