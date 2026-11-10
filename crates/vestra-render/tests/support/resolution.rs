//! Runtime output size preserves the authored layout before rasterization.

use super::*;

fn output_at(project: Value, resolution: (u32, u32), times: &[u128], gpu: bool) -> Vec<RgbaImage> {
    render_with_options(
        project,
        times,
        gpu,
        3,
        CompileOptions {
            resolution: Some(resolution),
            ..CompileOptions::default()
        },
    )
}

fn assert_half(full: &RgbaImage, small: &RgbaImage, maximum_error: u8) {
    assert_eq!((full.width() / 2, full.height() / 2), small.dimensions());
    let mut worst = (0, 0, 0, 0, 0, 0);
    for (x, y, pixel) in small.enumerate_pixels() {
        for channel in 0..4 {
            let expected = ((u32::from(full.get_pixel(x * 2, y * 2)[channel])
                + u32::from(full.get_pixel(x * 2 + 1, y * 2)[channel])
                + u32::from(full.get_pixel(x * 2, y * 2 + 1)[channel])
                + u32::from(full.get_pixel(x * 2 + 1, y * 2 + 1)[channel])
                + 2)
                / 4) as u8;
            let error = pixel[channel].abs_diff(expected);
            if error > worst.0 {
                worst = (error, x, y, channel, pixel[channel], expected);
            }
        }
    }
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/render-resolution");
    std::fs::create_dir_all(&artifacts).expect("artifact directory");
    let label = std::thread::current()
        .name()
        .expect("named test")
        .replace("::", "-");
    full.save(artifacts.join(format!("{label}-full.png")))
        .expect("full frame");
    small
        .save(artifacts.join(format!("{label}-half.png")))
        .expect("half frame");
    assert!(
        worst.0 <= maximum_error,
        "half-size worst pixel (error, x, y, channel, actual, expected): {worst:?}"
    );
}

fn hierarchy(sized: bool, source_mask: bool, feather: f64) -> Value {
    let mut card = group(
        "card",
        sized.then_some((96.0, 64.0)),
        vec![rectangle("leaf", 64.0, 32.0, "#ff0000", (0.5, 0.5))],
    );
    card["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (1.0, 1.0), 90.0);
    let input = if source_mask {
        json!({"type": "source", "mode": "alpha", "source": {
            "type": "group", "size": {"width": 32.0, "height": 32.0}, "clips": [
                clip("white", json!({"type": "solid_color", "colour": "#ffffff"}))
            ]
        }})
    } else {
        json!({"type": "shape", "geometry": {"type": "rectangle", "width": 32, "height": 32}, "fill": "#ffffff"})
    };
    card["masks"] =
        json!([{"id": "window", "input": input, "operation": "replace", "feather": feather}]);
    project(128, 96, vec![group("outer", None, vec![card])])
}

#[test]
fn runtime_resolution_preserves_sized_unsized_groups_and_mask_owner_coordinates() {
    for sized in [false, true] {
        for source_mask in [false, true] {
            let fixture = hierarchy(sized, source_mask, 0.0);
            let full = render(fixture.clone(), &[0], false).remove(0);
            let half = output_at(fixture, (64, 48), &[0], false).remove(0);
            assert_half(&full, &half, 0);
            assert_golden(
                &half,
                &golden(64, 48, &[(24, 16, 40, 32, [255, 0, 0, 255])]),
                0,
            );
        }
    }
}

#[test]
fn runtime_resolution_scales_feather_and_post_transform_blur() {
    let mut fixture = hierarchy(true, true, 8.0);
    fixture["visual"]["clips"][0]["effects"] = json!([
        {"id": "blur", "type": "gaussian_blur", "radius": {"base_value": 8.0}}
    ]);
    let full = render(fixture.clone(), &[0], false).remove(0);
    let half = output_at(fixture, (64, 48), &[0], false).remove(0);
    // Fractional box widths and pixel integration differ at the reduced edge.
    assert_half(&full, &half, 12);
    assert!(half.get_pixel(32, 24)[0] > 240);
    assert!(half.get_pixel(16, 24)[0] < 10);
}

#[test]
fn runtime_resolution_preserves_nonuniform_scale_and_rotation() {
    let mut fixture = hierarchy(true, false, 0.0);
    let card = &mut fixture["visual"]["clips"][0]["source"]["clips"][0];
    card.as_object_mut().unwrap().remove("masks");
    card["source"]["clips"][0]["transform"] =
        presentation((0.5, 0.5), (0.5, 0.5), (0.75, 1.25), 0.0);
    let full = render(fixture.clone(), &[0], false).remove(0);
    let half = output_at(fixture, (64, 48), &[0], false).remove(0);
    // Direct output sampling and averaged full-resolution samples differ at filtered corners.
    assert_half(&full, &half, 16);
    assert_golden(
        &half,
        &golden(64, 48, &[(22, 12, 42, 36, [255, 0, 0, 255])]),
        0,
    );
}

#[test]
fn runtime_resolution_upscale_preserves_authored_shape_proportions() {
    let fixture = project(
        64,
        48,
        vec![rectangle("red", 32.0, 16.0, "#ff0000", (0.5, 0.5))],
    );
    let enlarged = output_at(fixture, (128, 96), &[0], false).remove(0);
    let mut expected = golden(128, 96, &[(32, 32, 96, 64, [255, 0, 0, 255])]);
    // The doubled source raster has 3/4 edge coverage and (3/4)^2 corner coverage.
    for y in 32..64 {
        for x in 32..96 {
            let edge_x = x == 32 || x == 95;
            let edge_y = y == 32 || y == 63;
            let red = match (edge_x, edge_y) {
                (true, true) => 143,
                (true, false) | (false, true) => 191,
                (false, false) => 255,
            };
            expected.put_pixel(x, y, Rgba([red, 0, 0, 255]));
        }
    }
    assert_golden(&enlarged, &expected, 1);
}

#[test]
fn runtime_resolution_keeps_mattes_in_parent_coordinates() {
    let mut owner = rectangle("owner", 64.0, 32.0, "#ff0000", (0.5, 0.5));
    owner["matte"] = json!({"source_layer": "matte", "mode": "alpha"});
    let mut matte = rectangle("matte", 32.0, 16.0, "#ffffff", (0.5, 0.5));
    matte["visible"] = json!(false);
    let fixture = project(128, 96, vec![owner, matte]);
    let full = render(fixture.clone(), &[0], false).remove(0);
    let half = output_at(fixture, (64, 48), &[0], false).remove(0);
    assert_half(&full, &half, 0);
    assert_golden(
        &half,
        &golden(64, 48, &[(24, 20, 40, 28, [255, 0, 0, 255])]),
        0,
    );
}

fn raster_mask() -> Value {
    let mut owner = rectangle("owner", 128.0, 96.0, "#ff0000", (0.75, 0.5));
    owner["transform"] = presentation((0.75, 0.5), (0.5, 0.5), (0.5, 0.5), 0.0);
    owner["masks"] = json!([mask_rectangle("window", 32.0, 16.0, (0.25, 0.5), "replace")]);
    project(128, 96, vec![owner])
}

#[test]
fn runtime_resolution_raster_masks_scale_once_in_the_owner_canvas() {
    let fixture = raster_mask();
    let full = render(fixture.clone(), &[0], false).remove(0);
    let half = output_at(fixture, (64, 48), &[0], false).remove(0);
    assert_half(&full, &half, 0);
    assert_golden(
        &half,
        &golden(64, 48, &[(36, 22, 44, 26, [255, 0, 0, 255])]),
        0,
    );
}

#[test]
fn runtime_resolution_fits_letterbox_and_preserves_background_blending() {
    let mut shape = rectangle("red", 64.0, 32.0, "#ff0000", (0.5, 0.5));
    shape["blend_mode"] = json!("multiply");
    let mut fixture = project(128, 64, vec![shape]);
    fixture["output"]["background"] = json!("#204060");
    let output = output_at(fixture, (64, 64), &[0], false).remove(0);
    let mut expected = RgbaImage::from_pixel(64, 64, Rgba([32, 64, 96, 255]));
    for y in 24..40 {
        for x in 16..48 {
            expected.put_pixel(x, y, Rgba([32, 0, 0, 255]));
        }
    }
    assert_golden(&output, &expected, 0);
}

#[test]
fn runtime_resolution_letterbox_preserves_partially_transparent_background_alpha() {
    let mut fixture = project(
        128,
        64,
        vec![rectangle("red", 64.0, 32.0, "#ff000080", (0.5, 0.5))],
    );
    fixture["output"]["background"] = json!("#20406080");
    let native = render(fixture.clone(), &[0], false).remove(0);
    let fitted = output_at(fixture, (64, 64), &[0], false).remove(0);
    assert_eq!(fitted.get_pixel(0, 0).0, [32, 64, 96, 128]);
    assert_eq!(fitted.get_pixel(32, 32), native.get_pixel(64, 32));
}

fn cropped_tile() -> Value {
    let artifacts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/render-resolution");
    std::fs::create_dir_all(&artifacts).expect("fixture directory");
    let path = artifacts.join("crop-pattern.png");
    RgbaImage::from_fn(64, 32, |x, y| {
        Rgba(if (x / 16 + y / 16) % 2 == 0 {
            [255, 0, 0, 255]
        } else {
            [0, 255, 0, 255]
        })
    })
    .save(&path)
    .expect("synthetic crop fixture");
    let mut image = clip("pattern", json!({"type": "image", "asset": "pattern"}));
    image["crop"] = json!({"base_value": {"x": 0.25, "y": 0.0, "width": 0.5, "height": 1.0}});
    image["sizing"] = json!({"mode": "stretch", "width": 64, "height": 32});
    image["transform"] = presentation((0.5, 0.5), (0.5, 0.5), (0.75, 0.75), 90.0);
    image["effects"] = json!([{"id": "tile", "type": "motion_tile",
        "output_width_percent": {"base_value": 200.0}, "output_height_percent": {"base_value": 200.0},
        "tile_center": {"base_value": {"x": 0.5, "y": 0.5}}, "mirror_edges": true}]);
    let mut value = project(128, 96, vec![image]);
    value["assets"] = json!([{"id": "pattern", "type": "image", "source": path}]);
    value
}

#[test]
fn runtime_resolution_preserves_crops_sizing_and_pre_transform_motion_tile() {
    let mut fit = image_sizing_project("fit");
    let mut cover = image_sizing_project("cover");
    for fixture in [&mut fit, &mut cover] {
        fixture["output"]["width"] = json!(128);
        fixture["output"]["height"] = json!(96);
        fixture["visual"]["clips"][0]["source"]["size"] = json!({"width": 48.0, "height": 48.0});
    }
    for fixture in [cropped_tile(), fit, cover] {
        let width = fixture["output"]["width"].as_u64().unwrap() as u32;
        let height = fixture["output"]["height"].as_u64().unwrap() as u32;
        let full = render(fixture.clone(), &[0], false).remove(0);
        let half = output_at(fixture, (width / 2, height / 2), &[0], false).remove(0);
        assert_half(&full, &half, 0);
        assert!(half.pixels().any(|pixel| pixel[0] != 0 || pixel[2] != 0));
    }
}

fn animated() -> Value {
    let mut value = hierarchy(true, true, 0.0);
    let card = &mut value["visual"]["clips"][0]["source"]["clips"][0];
    card["transform"]["position"]["keyframes"] = json!([
        {"time": 0.0, "value": {"x": 0.25, "y": 0.5}, "interpolation": "linear"},
        {"time": 1.0, "value": {"x": 0.75, "y": 0.5}, "interpolation": "linear"}
    ]);
    value
}

#[test]
fn runtime_resolution_frames_match_fresh_preparation_in_random_access_order() {
    let fixture = animated();
    let times = [1_000_000_000, 0, 500_000_000, 0, 1_000_000_000];
    let full = render(fixture.clone(), &times, false);
    let half = output_at(fixture.clone(), (64, 48), &times, false);
    for (full, half) in full.iter().zip(&half) {
        assert_half(full, half, 0);
    }
    assert_eq!(half[0], half[4]);
    assert_eq!(half[1], half[3]);
    assert_ne!(half[0], half[1]);
    let fresh = output_at(fixture, (64, 48), &[500_000_000], false);
    assert_eq!(half[2], fresh[0]);
}

#[test]
fn runtime_resolution_preserves_normalized_particle_proportions() {
    let mut fixture = super::super::fixture_tests::particle_upload_project(1);
    fixture["output"]["width"] = json!(128);
    fixture["output"]["height"] = json!(96);
    let source = &mut fixture["visual"]["clips"][0]["source"];
    source["emission"]["bursts"][0]["count"] = json!(1);
    source["particle"]["size"] = json!(0.5);
    source["particle"]["colour"] = json!("#ff0000");
    let full = render(fixture.clone(), &[0], false).remove(0);
    let half = output_at(fixture, (64, 48), &[0], false).remove(0);
    assert_half(&full, &half, 0);
    assert_eq!(
        half.pixels().filter(|pixel| pixel[0] == 255).count(),
        24 * 24
    );
}

#[test]
fn gpu_runtime_resolution_matches_cpu_for_geometry_effects_and_letterbox() {
    for fixture in [
        hierarchy(false, false, 0.0),
        raster_mask(),
        hierarchy(true, true, 8.0),
        cropped_tile(),
        animated(),
    ] {
        output_at(fixture, (64, 48), &[1_000_000_000, 0, 500_000_000, 0], true);
    }
    output_at(
        project(
            64,
            48,
            vec![rectangle("upscaled", 32.0, 16.0, "#ff0000", (0.5, 0.5))],
        ),
        (128, 96),
        &[0],
        true,
    );
    output_at(
        project(
            128,
            64,
            vec![rectangle("red", 64.0, 32.0, "#ff0000", (0.5, 0.5))],
        ),
        (64, 64),
        &[0],
        true,
    );
}
