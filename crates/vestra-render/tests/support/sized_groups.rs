//! Sized-group goldens are built from explicit pixel regions, independently of evaluation.

use std::{collections::BTreeMap, path::Path, sync::Arc};

use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use vestra_core::{
    plan::{CompileOptions, PlanCompileInput, ScheduledItem, compile, evaluate},
    project::Project,
    validation::{ResourceLimits, validate},
};
use vestra_render::{CpuBackend, DecodedAssets, compare_rgba};

use super::{gpu::wgpu_backend_or_skip, lifecycle::RenderFrame};

fn presentation(
    position: (f64, f64),
    anchor: (f64, f64),
    scale: (f64, f64),
    rotation: f64,
) -> Value {
    json!({
        "position": {"base_value": {"x": position.0, "y": position.1}},
        "anchor": {"base_value": {"x": anchor.0, "y": anchor.1}},
        "scale": {"base_value": {"x": scale.0, "y": scale.1}},
        "rotation_degrees": {"base_value": rotation}
    })
}

fn clip(id: &str, source: Value) -> Value {
    json!({
        "id": id, "source": source, "start": 0.0, "duration": 3.0,
        "layer": 0, "opacity": {"base_value": 1.0}
    })
}

fn rectangle(id: &str, width: f64, height: f64, colour: &str, position: (f64, f64)) -> Value {
    let mut value = clip(
        id,
        json!({
            "type": "shape", "geometry": {"type": "rectangle", "width": width, "height": height},
            "fill": colour
        }),
    );
    value["transform"] = presentation(position, (0.5, 0.5), (1.0, 1.0), 0.0);
    value
}

fn group(id: &str, size: Option<(f64, f64)>, children: Vec<Value>) -> Value {
    let mut source = json!({"type": "group", "clips": children});
    if let Some((width, height)) = size {
        source["size"] = json!({"width": width, "height": height});
    }
    clip(id, source)
}

fn project(width: u32, height: u32, clips: Vec<Value>) -> Value {
    json!({
        "schema_version": 1,
        "output": {
            "path": "sized-groups.mp4", "width": width, "height": height,
            "frame_rate": "30000/1001", "background": "#000000", "quality": "preview",
            "audio": false, "duration_mode": "explicit", "duration": 3.0
        },
        "assets": [], "visual": {"clips": clips}
    })
}

fn render(project: Value, times: &[u128], compare_gpu: bool) -> Vec<RgbaImage> {
    render_with_tolerance(project, times, compare_gpu, 2)
}

fn render_with_tolerance(
    project: Value,
    times: &[u128],
    compare_gpu: bool,
    tolerance: u8,
) -> Vec<RgbaImage> {
    let project: Project = serde_json::from_value(project).expect("sized-group project parses");
    let limits = ResourceLimits::default();
    let report = validate(&project, limits);
    assert!(
        report.is_valid(),
        "sized-group validation: {:?}",
        report.diagnostics()
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = project
        .assets
        .iter()
        .map(|asset| (asset.id.clone(), root.join(&asset.source)))
        .collect::<BTreeMap<_, _>>();
    let durations = BTreeMap::new();
    let plan = compile(
        PlanCompileInput::new(
            &project,
            limits,
            &root,
            &assets,
            &durations,
            3.0,
            (30_000, 1_001),
            90,
            &[],
        ),
        CompileOptions::default(),
    )
    .expect("sized-group plan compiles");
    let decoded = DecodedAssets::build(&plan).expect("sized-group assets decode");
    let mut cpu = CpuBackend::new(&plan, Arc::clone(&decoded));
    let mut gpu = compare_gpu
        .then(|| wgpu_backend_or_skip(&plan, decoded))
        .flatten();
    times
        .iter()
        .map(|&time| {
            let active = plan
                .layers
                .iter()
                .enumerate()
                .filter_map(|(index, layer)| {
                    (layer.start_nanos <= time && time < layer.start_nanos + layer.duration_nanos)
                        .then_some(ScheduledItem(index))
                })
                .collect::<Vec<_>>();
            let frame = evaluate(&plan, &active, time).expect("sized-group frame evaluates");
            let mut output = RgbaImage::new(frame.width, frame.height);
            cpu.render_frame(&frame, &mut output)
                .expect("CPU sized group renders");
            if let Some(gpu) = &mut gpu {
                let mut gpu_output = RgbaImage::new(frame.width, frame.height);
                gpu.render_frame(&frame, &mut gpu_output)
                    .expect("WGPU sized group renders");
                let difference = compare_rgba(output.as_raw(), gpu_output.as_raw(), tolerance);
                eprintln!("sized-group parity at {time}: {difference:?}");
                let artifacts =
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sized-groups");
                std::fs::create_dir_all(&artifacts).expect("create parity artifact directory");
                let thread = std::thread::current();
                let label = thread.name().expect("named test thread").replace("::", "-");
                output
                    .save(artifacts.join(format!("{label}-{time}-cpu.png")))
                    .expect("save CPU frame");
                gpu_output
                    .save(artifacts.join(format!("{label}-{time}-wgpu.png")))
                    .expect("save WGPU frame");
                assert!(
                    difference.maximum_absolute_channel_error <= tolerance,
                    "sized-group parity at {time}: {difference:?}"
                );
            }
            output
        })
        .collect()
}

fn golden(width: u32, height: u32, regions: &[(u32, u32, u32, u32, [u8; 4])]) -> RgbaImage {
    let mut expected = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 255]));
    for &(left, top, right, bottom, colour) in regions {
        for y in top..bottom {
            for x in left..right {
                expected.put_pixel(x, y, Rgba(colour));
            }
        }
    }
    expected
}

fn assert_golden(actual: &RgbaImage, expected: &RgbaImage, tolerance: u8) {
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/sized-groups");
    std::fs::create_dir_all(&artifacts).expect("create golden artifact directory");
    let thread = std::thread::current();
    let label = thread.name().expect("named test thread").replace("::", "-");
    let extent = format!("{}x{}", actual.width(), actual.height());
    actual
        .save(artifacts.join(format!("{label}-{extent}-actual.png")))
        .expect("save actual frame");
    expected
        .save(artifacts.join(format!("{label}-{extent}-expected.png")))
        .expect("save expected frame");
    let difference = compare_rgba(actual.as_raw(), expected.as_raw(), tolerance);
    assert!(
        difference.maximum_absolute_channel_error <= tolerance,
        "golden mismatch: {difference:?}"
    );
}

fn large_group_project() -> Value {
    let mut card = group(
        "card",
        Some((64.0, 32.0)),
        vec![rectangle("right", 8.0, 8.0, "#ff0000", (0.75, 0.5))],
    );
    card["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (0.5, 0.5), 0.0);
    project(32, 24, vec![card])
}

#[test]
fn sized_group_larger_than_output_retains_content_before_scaling() {
    let output = render(large_group_project(), &[0], false).remove(0);
    assert_golden(
        &output,
        &golden(32, 24, &[(22, 10, 26, 14, [255, 0, 0, 255])]),
        0,
    );
}

fn overflow_project() -> Value {
    project(
        32,
        24,
        vec![group(
            "card",
            Some((8.0, 8.0)),
            vec![
                rectangle("right", 8.0, 4.0, "#ff0000", (1.0, 0.5)),
                rectangle("negative", 4.0, 4.0, "#00ff00", (-0.125, 0.5)),
            ],
        )],
    )
}

#[test]
fn sized_group_clips_positive_and_negative_child_overflow_at_local_bounds() {
    let output = render(overflow_project(), &[0], false).remove(0);
    assert_golden(
        &output,
        &golden(
            32,
            24,
            &[
                (16, 10, 20, 14, [255, 0, 0, 255]),
                (12, 10, 13, 14, [0, 255, 0, 255]),
            ],
        ),
        0,
    );
}

fn nested_project() -> Value {
    let child = rectangle("leaf", 4.0, 4.0, "#ff0000", (0.5, 0.5));
    let mut inner = group("inner", Some((8.0, 8.0)), vec![child]);
    inner["transform"] = presentation((0.75, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    let inherited = group("inherited", None, vec![inner]);
    let mut outer = group("outer", Some((24.0, 12.0)), vec![inherited]);
    outer["transform"] = presentation((0.5, 0.25), (0.25, 0.5), (1.5, 0.5), 90.0);
    project(40, 32, vec![outer])
}

#[test]
fn sized_group_mixed_nesting_preserves_parent_anchor_rotation_and_nonuniform_scale() {
    let output = render(nested_project(), &[0], false).remove(0);
    // Local [16,20]x[4,8] is scaled around (6,6), rotated clockwise,
    // then translated to (20,8): destination [19,21]x[23,29].
    // Bilinear sampling of the 1.5x stretched edge gives 1/6 and 5/6
    // coverage in the two edge rows; four interior rows remain opaque.
    assert_golden(
        &output,
        &golden(
            40,
            32,
            &[
                (19, 22, 21, 23, [43, 0, 0, 255]),
                (19, 23, 21, 24, [213, 0, 0, 255]),
                (19, 24, 21, 28, [255, 0, 0, 255]),
                (19, 28, 21, 29, [213, 0, 0, 255]),
                (19, 29, 21, 30, [43, 0, 0, 255]),
            ],
        ),
        1,
    );
}

#[test]
fn sized_group_source_reuse_keeps_geometry_size_across_project_resolutions() {
    let source = group(
        "left",
        Some((16.0, 8.0)),
        vec![rectangle("leaf", 4.0, 4.0, "#ff0000", (0.25, 0.5))],
    );
    for (width, height) in [(32, 24), (64, 48)] {
        let mut left = source.clone();
        left["transform"] = presentation((0.25, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
        let mut right = source.clone();
        right["id"] = json!("right");
        right["transform"] = presentation((0.75, 0.5), (0.5, 0.5), (0.5, 0.5), 0.0);
        let output = render(project(width, height, vec![left, right]), &[0], false).remove(0);
        let (left_center, right_center, cy) = (width / 4 - 4, width * 3 / 4 - 2, height / 2);
        assert_golden(
            &output,
            &golden(
                width,
                height,
                &[
                    (
                        left_center - 2,
                        cy - 2,
                        left_center + 2,
                        cy + 2,
                        [255, 0, 0, 255],
                    ),
                    (
                        right_center - 1,
                        cy - 1,
                        right_center + 1,
                        cy + 1,
                        [255, 0, 0, 255],
                    ),
                ],
            ),
            0,
        );
    }
}

fn sibling_extents_project() -> Value {
    let mut large = group(
        "large",
        Some((128.0, 64.0)),
        vec![clip(
            "red",
            json!({"type": "solid_color", "colour": "#ff0000"}),
        )],
    );
    large["transform"] = presentation((0.25, 0.5), (0.5, 0.5), (0.25, 0.25), 0.0);
    let mut small = group(
        "small",
        Some((8.0, 8.0)),
        vec![clip(
            "blue",
            json!({"type": "solid_color", "colour": "#0000ff"}),
        )],
    );
    small["transform"] = presentation((0.75, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    project(64, 32, vec![large, small])
}

#[test]
fn sized_group_siblings_reuse_surfaces_without_leaking_different_extents() {
    let output = render(sibling_extents_project(), &[0], false).remove(0);
    assert_golden(
        &output,
        &golden(
            64,
            32,
            &[
                (0, 8, 32, 24, [255, 0, 0, 255]),
                (44, 12, 52, 20, [0, 0, 255, 255]),
            ],
        ),
        0,
    );
}

fn animated_project() -> Value {
    let mut leaf = rectangle("leaf", 4.0, 4.0, "#ff0000", (0.25, 0.5));
    leaf["transform"]["position"]["keyframes"] = json!([
        {"time": 0.0, "value": {"x": 0.25, "y": 0.5}, "interpolation": "linear"},
        {"time": 1.0, "value": {"x": 0.75, "y": 0.5}, "interpolation": "linear"}
    ]);
    let card = group("card", Some((16.0, 8.0)), vec![leaf]);
    project(32, 24, vec![card])
}

#[test]
fn sized_group_animated_positions_use_local_dimensions_in_any_request_order() {
    let output = render(
        animated_project(),
        &[1_000_000_000, 0, 500_000_000, 0],
        false,
    );
    for (frame, center) in output.iter().zip([20, 12, 16, 12]) {
        assert_golden(
            frame,
            &golden(
                32,
                24,
                &[(center - 2, 10, center + 2, 14, [255, 0, 0, 255])],
            ),
            0,
        );
    }
}

fn animated_parent_project() -> Value {
    let mut card = group(
        "card",
        Some((16.0, 8.0)),
        vec![rectangle("leaf", 4.0, 4.0, "#ff0000", (0.25, 0.5))],
    );
    card["transform"] = presentation((0.25, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    for (property, first, last) in [
        (
            "position",
            json!({"x": 0.25, "y": 0.5}),
            json!({"x": 0.75, "y": 0.5}),
        ),
        (
            "scale",
            json!({"x": 1.0, "y": 1.0}),
            json!({"x": 2.0, "y": 1.0}),
        ),
        ("rotation_degrees", json!(0.0), json!(90.0)),
    ] {
        card["transform"][property]["keyframes"] = json!([
            {"time": 0.0, "value": first, "interpolation": "linear"},
            {"time": 1.0, "value": last, "interpolation": "linear"}
        ]);
    }
    project(32, 24, vec![card])
}

#[test]
fn sized_group_animated_parent_translation_rotation_and_scale_preserve_local_arrangement() {
    let output = render(
        animated_parent_project(),
        &[1_000_000_000, 0, 500_000_000, 0],
        false,
    );
    for (frame, (x, y)) in output.iter().zip([(24, 4), (4, 12), (12, 8), (4, 12)]) {
        assert_eq!(frame.get_pixel(x, y).0, [255, 0, 0, 255]);
    }
    assert_eq!(output[0].get_pixel(4, 12).0, [0, 0, 0, 255]);
    assert_eq!(output[1].get_pixel(24, 4).0, [0, 0, 0, 255]);
    assert_eq!(output[1], output[3]);
}

#[test]
fn sized_group_fractional_size_uses_authored_dimensions_for_position_and_anchor() {
    let mut card = group(
        "card",
        Some((15.5, 7.5)),
        vec![rectangle("leaf", 4.0, 4.0, "#ff0000", (0.5, 0.5))],
    );
    card["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    let output = render(project(32, 24, vec![card]), &[0], false).remove(0);
    // Fractional local center (7.75,3.75) and the identical group anchor cancel.
    assert_eq!(output.get_pixel(15, 11).0, [255, 0, 0, 255]);
    assert_eq!(output.get_pixel(16, 12).0, [255, 0, 0, 255]);
    assert_eq!(output.get_pixel(12, 12).0, [0, 0, 0, 255]);
    assert_eq!(output.get_pixel(19, 12).0, [0, 0, 0, 255]);
}

fn masked_project(source_mask: bool) -> Value {
    let mut owner = group(
        "owner",
        Some((16.0, 12.0)),
        vec![clip(
            "fill",
            json!({"type": "solid_color", "colour": "#ff0000"}),
        )],
    );
    let input = if source_mask {
        json!({"type": "source", "mode": "alpha", "source": {
            "type": "group", "size": {"width": 8.0, "height": 8.0}, "clips": [
                clip("white", json!({"type": "solid_color", "colour": "#ffffff"}))
            ]
        }})
    } else {
        json!({"type": "shape", "geometry": {"type": "rectangle", "width": 8.0, "height": 8.0}, "fill": "#ffffff"})
    };
    owner["masks"] = json!([{
        "id": "window", "input": input, "operation": "replace",
        "transform": presentation((0.25, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0)
    }]);
    project(32, 24, vec![owner])
}

#[test]
fn sized_group_owner_and_sized_group_mask_source_share_owner_local_coordinates() {
    for source_mask in [false, true] {
        let output = render(masked_project(source_mask), &[0], false).remove(0);
        assert_golden(
            &output,
            &golden(32, 24, &[(8, 8, 16, 16, [255, 0, 0, 255])]),
            0,
        );
    }
}

fn stacked_masks_project(source_first: bool) -> Value {
    let mut value = masked_project(true);
    let owner = &mut value["visual"]["clips"][0];
    let mut source = owner["masks"][0].clone();
    source["operation"] = json!("intersect");
    source["input"]["source"]["clips"][0]["masks"] = json!([{
        "id": "child-window", "operation": "intersect", "input": {
            "type": "shape", "geometry": {"type": "rectangle", "width": 6.0, "height": 8.0},
            "fill": "#ffffff"
        }, "transform": presentation((0.375, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0)
    }]);
    let shape = json!({
        "id": "shape", "operation": "intersect", "input": {
            "type": "shape", "geometry": {"type": "rectangle", "width": 8.0, "height": 8.0},
            "fill": "#ffffff"
        }, "transform": presentation((0.5, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0)
    });
    owner["masks"] = if source_first {
        json!([source, shape])
    } else {
        json!([shape, source])
    };
    value
}

#[test]
fn sized_group_source_masks_preserve_coverage_from_other_masks_in_both_orders() {
    for source_first in [false, true] {
        let output = render(stacked_masks_project(source_first), &[0], false).remove(0);
        assert_golden(
            &output,
            &golden(32, 24, &[(12, 8, 14, 16, [255, 0, 0, 255])]),
            0,
        );
    }
}

fn parent_space_matte_project() -> Value {
    let mut owner = group(
        "owner",
        Some((16.0, 12.0)),
        vec![clip(
            "red",
            json!({"type": "solid_color", "colour": "#ff0000"}),
        )],
    );
    owner["transform"] = presentation((0.75, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    owner["matte"] = json!({"source_layer": "matte", "mode": "alpha"});
    let mut matte = rectangle("matte", 8.0, 8.0, "#ffffff", (0.25, 0.5));
    matte["visible"] = json!(false);
    project(32, 24, vec![owner, matte])
}

#[test]
fn sized_group_track_matte_keeps_sibling_source_in_parent_coordinates() {
    let output = render(parent_space_matte_project(), &[0], false).remove(0);
    assert_golden(&output, &golden(32, 24, &[]), 0);
}

fn effects_project() -> Value {
    let mut child = clip("fill", json!({"type": "solid_color", "colour": "#204060"}));
    child["effects"] = json!([
        {"id": "vignette", "type": "vignette", "amount": {"base_value": 0.4},
         "radius": {"base_value": 0.6}, "softness": {"base_value": 0.5}, "colour": "#000000"},
        {"id": "blur", "type": "gaussian_blur", "radius": {"base_value": 1.25}}
    ]);
    let mut card = group("card", Some((12.0, 6.0)), vec![child]);
    card["effects"] = json!([
        {"id": "brightness", "type": "brightness", "amount": {"base_value": 0.15}},
        {"id": "blur", "type": "gaussian_blur", "radius": {"base_value": 0.75}}
    ]);
    project(32, 24, vec![card])
}

#[test]
fn sized_group_solid_fill_and_effects_use_local_canvas_then_parent_canvas() {
    let fill = group(
        "card",
        Some((12.0, 6.0)),
        vec![clip(
            "fill",
            json!({"type": "solid_color", "colour": "#204060"}),
        )],
    );
    let output = render(project(32, 24, vec![fill]), &[0], false).remove(0);
    assert_golden(
        &output,
        &golden(32, 24, &[(10, 9, 22, 15, [32, 64, 96, 255])]),
        0,
    );
    let effected = render(effects_project(), &[0], false).remove(0);
    assert_ne!(effected.get_pixel(16, 12).0, [0, 0, 0, 255]);
    assert_ne!(
        effected.get_pixel(9, 12).0,
        [0, 0, 0, 255],
        "parent blur can extend past group bounds"
    );
}

fn image_sizing_project(mode: &str) -> Value {
    let mut image = clip("image", json!({"type": "image", "asset": "blue"}));
    image["sizing"] = json!({"mode": mode});
    image["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    let card = group("card", Some((12.0, 12.0)), vec![image]);
    let mut value = project(32, 24, vec![card]);
    value["assets"] =
        json!([{"id": "blue", "type": "image", "source": "examples/assets/blue.png"}]);
    value
}

#[test]
fn sized_group_image_fit_and_cover_use_local_canvas_dimensions() {
    let source =
        image::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/assets/blue.png"))
            .expect("blue fixture")
            .into_rgba8();
    let colour = source.get_pixel(100, 50).0;
    for (mode, top, bottom) in [("fit", 9, 15), ("cover", 6, 18)] {
        let output = render(image_sizing_project(mode), &[0], false).remove(0);
        assert_golden(
            &output,
            &golden(32, 24, &[(10, top, 22, bottom, colour)]),
            0,
        );
    }
}

fn local_edge_effects_project() -> Value {
    let mut edge = rectangle("edge", 8.0, 8.0, "#e08040", (0.9, 0.5));
    edge["effects"] = json!([
        {"id": "local-blur", "type": "gaussian_blur", "radius": {"base_value": 2.0}}
    ]);
    let mut large = group("large", Some((48.0, 18.0)), vec![edge]);
    large["transform"] = presentation((0.35, 0.5), (0.5, 0.5), (0.8, 1.2), 25.0);
    large["effects"] = json!([
        {"id": "parent-blur", "type": "gaussian_blur", "radius": {"base_value": 1.25}}
    ]);
    let mut small = group(
        "small",
        Some((8.0, 6.0)),
        vec![clip(
            "blue",
            json!({"type": "solid_color", "colour": "#2040c0"}),
        )],
    );
    small["transform"] = presentation((0.8, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    let mut value = project(32, 24, vec![large, small]);
    value["visual"]["post_effects"] = json!([
        {"id": "global-blur", "type": "gaussian_blur", "radius": {"base_value": 0.75}},
        {"id": "global-brightness", "type": "brightness", "amount": {"base_value": 0.05}}
    ]);
    value
}

fn feathered_mask_project() -> Value {
    let mut value = masked_project(false);
    let owner = &mut value["visual"]["clips"][0];
    owner["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (1.5, 0.75), 30.0);
    owner["masks"][0]["feather"] = json!({"base_value": 1.5});
    value
}

#[test]
fn gpu_sized_group_feathered_owner_mask_matches_cpu_under_parent_transform() {
    render(feathered_mask_project(), &[0], true);
    let mut owner = rectangle("owner", 20.0, 16.0, "#ff0000", (0.5, 0.5));
    owner["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (1.4, 0.7), 25.0);
    let source = group(
        "mask",
        Some((12.0, 8.0)),
        vec![clip(
            "white",
            json!({"type": "solid_color", "colour": "#ffffff"}),
        )],
    );
    owner["masks"] = json!([{
        "id": "sized-mask", "input": {"type": "source", "mode": "alpha", "source": source["source"]},
        "operation": "replace", "feather": {"base_value": 2.0}
    }]);
    render(project(32, 24, vec![owner]), &[0], true);
}

#[test]
fn gpu_sized_groups_match_cpu_for_nested_masks_effects_sizing_and_animation() {
    for fixture in [
        large_group_project(),
        overflow_project(),
        nested_project(),
        sibling_extents_project(),
        masked_project(false),
        masked_project(true),
        stacked_masks_project(false),
        stacked_masks_project(true),
        parent_space_matte_project(),
        effects_project(),
        local_edge_effects_project(),
        image_sizing_project("fit"),
        image_sizing_project("cover"),
    ] {
        render(fixture, &[0], true);
    }
    render(
        animated_parent_project(),
        &[1_000_000_000, 0, 500_000_000, 0],
        true,
    );
    render(
        animated_project(),
        &[1_000_000_000, 0, 500_000_000, 0],
        true,
    );
}

fn mask_rectangle(
    id: &str,
    width: f64,
    height: f64,
    position: (f64, f64),
    operation: &str,
) -> Value {
    json!({
        "id": id, "operation": operation, "input": {
            "type": "shape", "geometry": {"type": "rectangle", "width": width, "height": height},
            "fill": "#ffffff"
        }, "transform": presentation(position, (0.5, 0.5), (1.0, 1.0), 0.0)
    })
}

fn mixed_mask_operations_project(nested_source: bool, luma: bool) -> Value {
    let mut value = stacked_masks_project(false);
    let owner = &mut value["visual"]["clips"][0];
    let mut replace = mask_rectangle("replace", 8.0, 12.0, (0.25, 0.5), "replace");
    replace["strength"] = json!({"base_value": 0.75});
    let mut union = mask_rectangle("union", 8.0, 12.0, (0.25, 0.5), "union");
    union["invert"] = json!(true);
    union["strength"] = json!({"base_value": 0.4});
    let mut subtract = mask_rectangle("subtract", 16.0, 6.0, (0.5, 0.25), "subtract");
    subtract["strength"] = json!({"base_value": 0.3});
    let mut source = owner["masks"][1].clone();
    source["strength"] = json!({"base_value": 0.6});
    source["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    if luma {
        source["input"]["mode"] = json!("luma");
        source["input"]["source"]["clips"][0]["source"]["colour"] = json!("#808080");
    }
    if nested_source {
        let child = source["input"]["source"]["clips"][0].clone();
        source["input"]["source"]["clips"] = json!([group("inherited", None, vec![child])]);
    }
    owner["masks"] = json!([replace, union, subtract, source]);
    value
}

#[test]
fn gpu_sized_group_mixed_mask_operations_match_independent_coverage_golden() {
    for luma in [false, true] {
        let mut expected = golden(32, 24, &[]);
        for y in 0..12 {
            for x in 0..16 {
                // Replace leaves 1 or 1/4; inverted union raises the right half to 0.55.
                // Subtract removes 30% in the top half; final intersection removes
                // 60% outside the masked source's [4,10]x[2,10] window.
                let left_right = if x < 8 { 1.0_f64 } else { 0.55 };
                let top_bottom = if y < 6 { 0.7 } else { 1.0 };
                let window = if (4..10).contains(&x) && (2..10).contains(&y) {
                    if luma { 0.4 + 0.6 * 128.0 / 255.0 } else { 1.0 }
                } else {
                    0.4
                };
                let red = (255.0 * left_right * top_bottom * window).round() as u8;
                expected.put_pixel(x + 8, y + 6, Rgba([red, 0, 0, 255]));
            }
        }
        for nested in [false, true] {
            for frame in render(
                mixed_mask_operations_project(nested, luma),
                &[1_000_000_000, 0, 500_000_000, 0],
                true,
            ) {
                assert_golden(&frame, &expected, 1);
            }
        }
    }
}

fn animated_mask_owner_project() -> Value {
    let mut value = masked_project(true);
    let owner = &mut value["visual"]["clips"][0];
    owner["transform"] = presentation((0.25, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    owner["transform"]["position"]["keyframes"] = json!([
        {"time": 0.0, "value": {"x": 0.25, "y": 0.5}, "interpolation": "linear"},
        {"time": 1.0, "value": {"x": 0.75, "y": 0.5}, "interpolation": "linear"}
    ]);
    let source = &mut owner["masks"][0];
    source["transform"]["position"]["keyframes"] = json!([
        {"time": 0.0, "value": {"x": 0.25, "y": 0.5}, "interpolation": "linear"},
        {"time": 1.0, "value": {"x": 0.75, "y": 0.5}, "interpolation": "linear"}
    ]);
    source["input"]["source"]["clips"][0]["masks"] = json!([mask_rectangle(
        "source-window",
        4.0,
        8.0,
        (0.5, 0.5),
        "intersect"
    )]);
    let mut subtract = mask_rectangle("subtract", 16.0, 6.0, (0.5, 0.25), "subtract");
    subtract["strength"] = json!({"base_value": 0.5});
    owner["masks"]
        .as_array_mut()
        .expect("mask list")
        .push(subtract);
    value
}

#[test]
fn gpu_sized_group_mask_and_owner_animation_match_goldens_in_random_access_order() {
    let fixture = animated_mask_owner_project();
    let times = [1_000_000_000, 0, 500_000_000, 0, 1_000_000_000];
    let frames = render(fixture.clone(), &times, true);
    for (frame, left) in frames.iter().zip([26, 2, 14, 2, 26]) {
        assert_golden(
            frame,
            &golden(
                32,
                24,
                &[
                    (left, 8, left + 4, 12, [128, 0, 0, 255]),
                    (left, 12, left + 4, 16, [255, 0, 0, 255]),
                ],
            ),
            0,
        );
    }
    let fresh = render(fixture, &[500_000_000], true);
    assert_eq!(frames[2], fresh[0]);
}

#[test]
fn gpu_sized_group_weak_mask_stacks_bound_rgba8_rounding_error() {
    // shortcut: RGBA8 rounds coverage per stage; widen storage if deep faint stacks need sub-byte parity.
    for (stages, expected_float, expected_byte) in [(12, 226, 229), (64, 134, 137)] {
        let masks = (0..stages)
            .map(|index| {
                let mut mask =
                    mask_rectangle(&format!("mask-{index}"), 16.0, 12.0, (0.5, 0.5), "subtract");
                mask["strength"] = json!({"base_value": 0.01});
                mask
            })
            .collect::<Vec<_>>();
        let mut owner = group(
            "owner",
            Some((16.0, 12.0)),
            vec![clip(
                "red",
                json!({"type": "solid_color", "colour": "#ff0000"}),
            )],
        );
        owner["masks"] = json!(masks);
        eprintln!("WEAK_MASK_STRESS stages={stages}");
        let output = render_with_tolerance(project(32, 24, vec![owner]), &[0], true, 3).remove(0);
        let red = (255.0_f64 * 0.99_f64.powi(stages)).round() as u8;
        let byte_coverage = (0..stages).fold(255.0_f64, |value, _| (value * 0.99).round()) as u8;
        assert_eq!((red, byte_coverage), (expected_float, expected_byte));
        assert!(red.abs_diff(byte_coverage) <= 3);
        assert_golden(
            &output,
            &golden(32, 24, &[(8, 6, 24, 18, [red, 0, 0, 255])]),
            0,
        );
    }
}

fn animated_feather_stack_project() -> Value {
    let mut value = mixed_mask_operations_project(true, false);
    let owner = &mut value["visual"]["clips"][0];
    owner["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (1.0, 1.0), 0.0);
    for (property, first, last) in [
        (
            "position",
            json!({"x": 0.5, "y": 0.5}),
            json!({"x": 0.4, "y": 0.6}),
        ),
        (
            "scale",
            json!({"x": 1.0, "y": 1.0}),
            json!({"x": 1.3, "y": 0.8}),
        ),
        ("rotation_degrees", json!(0.0), json!(31.0)),
    ] {
        owner["transform"][property]["keyframes"] = json!([
            {"time": 0.0, "value": first, "interpolation": "linear"},
            {"time": 1.0, "value": last, "interpolation": "linear"}
        ]);
    }
    for (index, mask) in owner["masks"]
        .as_array_mut()
        .expect("mask list")
        .iter_mut()
        .enumerate()
    {
        mask["feather"] = json!({"base_value": 0.0, "keyframes": [
            {"time": 0.0, "value": 0.0, "interpolation": "linear"},
            {"time": 1.0, "value": 1.25 + index as f64 * 0.5, "interpolation": "linear"}
        ]});
        mask["transform"]["rotation_degrees"]["keyframes"] = json!([
            {"time": 0.0, "value": 0.0, "interpolation": "linear"},
            {"time": 1.0, "value": -17.0, "interpolation": "linear"}
        ]);
    }
    value
}

#[test]
fn gpu_sized_group_recursive_feathered_mask_stack_is_stable_across_random_access() {
    let fixture = animated_feather_stack_project();
    let frames = render(
        fixture.clone(),
        &[1_000_000_000, 0, 250_000_000, 500_000_000, 0, 1_000_000_000],
        true,
    );
    assert_eq!(frames[0], frames[5]);
    assert_eq!(frames[1], frames[4]);
    let fresh = render(fixture, &[500_000_000], true);
    assert_eq!(frames[3], fresh[0]);
}

#[test]
fn gpu_unsized_mask_stack_preserves_legacy_golden() {
    let mut owner = group(
        "legacy",
        None,
        vec![clip(
            "red",
            json!({"type": "solid_color", "colour": "#ff0000"}),
        )],
    );
    owner["masks"] = json!([
        mask_rectangle("replace", 8.0, 8.0, (0.25, 0.5), "replace"),
        mask_rectangle("union", 8.0, 8.0, (0.5, 0.5), "union"),
        mask_rectangle("subtract", 8.0, 4.0, (0.375, 0.5), "subtract")
    ]);
    let expected = golden(
        32,
        24,
        &[
            (4, 8, 20, 10, [255, 0, 0, 255]),
            (4, 14, 20, 16, [255, 0, 0, 255]),
            (4, 10, 8, 14, [255, 0, 0, 255]),
            (16, 10, 20, 14, [255, 0, 0, 255]),
        ],
    );
    for frame in render(project(32, 24, vec![owner]), &[0, 1_000_000_000, 0], true) {
        assert_golden(&frame, &expected, 0);
    }
}
