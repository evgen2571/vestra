use crate::gaussian::gaussian_kernel_cache_len;

use super::*;

fn canonical_stylization(value: serde_json::Value) -> EvaluatedEffect {
    let mut project: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/wgpu-small-rgba.json"
    ))
    .expect("canonical fixture");
    let id = value["id"].as_str().expect("test effect id").to_owned();
    project["visual"]["post_effects"] = serde_json::json!([value]);
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/stylization/cpu-tests");
    std::fs::create_dir_all(&directory).expect("test directory");
    let path = directory.join(format!("{id}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec(&project).expect("serialize fixture"),
    )
    .expect("write fixture");
    let input = crate::test_support::load_and_validate(
        &path,
        &crate::test_support::ValidationOptions::default(),
    )
    .expect("stylization canonical model accepted");
    let plan = vestra_core::plan::compile(input, Default::default()).expect("stylization compiles");
    let signals = vestra_core::plan::PreparedScalarSignals::empty();
    vestra_core::plan::evaluate_effect(
        &plan.post_effects[0].effect,
        0,
        1_000_000_000,
        &vestra_core::plan::EvaluationContext::new(&signals),
    )
    .expect("stylization evaluates")
}

#[test]
fn palette_gradient_maps_authored_tones_and_preserves_alpha_and_hidden_rgb() {
    let effect = canonical_stylization(serde_json::json!({
        "type":"palette_map", "id":"gradient",
        "palette":["#100020","#e0ff80"], "mode":"gradient",
        "amount":{"base_value":1.0}, "phase":{"base_value":0.0}
    }));
    let source = RgbaImage::from_raw(
        4,
        1,
        vec![
            0, 0, 0, 255, 128, 128, 128, 128, 255, 255, 255, 255, 128, 0, 255, 0,
        ],
    )
    .expect("source pixels");
    let output = render_effect(&source, effect);
    assert_eq!(
        output.as_raw(),
        &[
            16, 0, 32, 255, 120, 128, 80, 128, 224, 255, 128, 255, 128, 0, 255, 0
        ]
    );
}

#[test]
fn ordered_dither_has_literal_bayer_ranks_and_palette_independent_structure() {
    for (id, palette, dark, light) in [
        (
            "mono",
            ["#000000", "#ffffff"],
            [0, 0, 0, 128],
            [255, 255, 255, 128],
        ),
        (
            "cool",
            ["#101020", "#30e0ff"],
            [16, 16, 32, 128],
            [48, 224, 255, 128],
        ),
        (
            "warm",
            ["#200800", "#ffbc60"],
            [32, 8, 0, 128],
            [255, 188, 96, 128],
        ),
    ] {
        let effect = canonical_stylization(serde_json::json!({
            "type":"ordered_dither", "id":id, "palette":palette,
            "mode":"nearest", "amount":{"base_value":1.0},
            "strength":{"base_value":1.0}, "phase":{"base_value":0.0},
            "matrix":"bayer2", "scale":1
        }));
        let source = RgbaImage::from_pixel(2, 2, Rgba([128, 128, 128, 128]));
        let output = render_effect(&source, effect);
        assert_eq!(
            output.pixels().map(|p| p.0).collect::<Vec<_>>(),
            vec![dark, light, light, dark],
            "{id}"
        );
    }
}

#[test]
fn dither_scale_expands_threshold_cells_without_changing_tone_or_alpha() {
    let effect = canonical_stylization(serde_json::json!({
        "type":"ordered_dither", "id":"scale", "palette":["#000000","#ffffff"],
        "mode":"nearest", "amount":{"base_value":1.0},
        "strength":{"base_value":1.0}, "phase":{"base_value":0.0},
        "matrix":"bayer2", "scale":2
    }));
    let mut source = RgbaImage::from_pixel(4, 4, Rgba([128, 128, 128, 255]));
    source.put_pixel(3, 3, Rgba([99, 11, 240, 0]));
    let output = render_effect(&source, effect);
    let values = output.pixels().map(|p| p[0]).collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![
            0, 0, 255, 255, 0, 0, 255, 255, 255, 255, 0, 0, 255, 255, 0, 99
        ]
    );
    assert_eq!(output.get_pixel(3, 3), source.get_pixel(3, 3));
}

fn render_effect(source: &RgbaImage, effect: EvaluatedEffect) -> RgbaImage {
    let mut surfaces = EffectSurfacePool::new(source.width(), source.height());
    surfaces.begin_from(source);
    let mut timings = CpuHotPathTimings::default();
    apply_chain(&mut surfaces, &[effect], &mut timings, true, &[]);
    let mut output = RgbaImage::new(source.width(), source.height());
    surfaces.copy_to(&mut output);
    output
}

#[test]
fn gaussian_spreads_symmetrically_without_transparent_colour_halos() {
    let mut source = RgbaImage::from_pixel(7, 7, Rgba([255, 0, 0, 0]));
    source.put_pixel(3, 3, Rgba([255, 255, 255, 255]));
    let target = render_effect(&source, EvaluatedEffect::GaussianBlur { radius: 2.0 });

    assert_eq!(target.get_pixel(2, 3), target.get_pixel(4, 3));
    assert_eq!(target.get_pixel(3, 2), target.get_pixel(3, 4));
    assert!(target.get_pixel(2, 3)[3] > 0);
    assert_eq!(target.get_pixel(0, 0)[0], 0);
}

#[test]
fn transparent_glow_matches_the_pixel_golden_fixture_through_pass_execution() {
    let mut source = RgbaImage::new(3, 3);
    source.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
    let output = render_effect(
        &source,
        EvaluatedEffect::Glow {
            threshold: 0.0,
            radius: 1.0,
            intensity: 1.0,
            colour: [255, 64, 0, 255],
        },
    );

    assert_eq!(
        output.as_raw(),
        &[
            255, 64, 0, 3, 255, 64, 0, 21, 255, 64, 0, 3, 255, 64, 0, 21, 255, 255, 255, 255, 255,
            64, 0, 21, 255, 64, 0, 3, 255, 64, 0, 21, 255, 64, 0, 3,
        ]
    );
}

#[test]
fn sharpen_matches_the_pixel_golden_fixture_through_pass_execution() {
    let mut source = RgbaImage::new(3, 1);
    source.put_pixel(0, 0, Rgba([20, 80, 160, 255]));
    source.put_pixel(1, 0, Rgba([180, 120, 60, 192]));
    source.put_pixel(2, 0, Rgba([40, 200, 100, 128]));
    let output = render_effect(
        &source,
        EvaluatedEffect::Sharpen {
            amount: 0.8,
            radius: 1.0,
        },
    );

    assert_eq!(
        output.as_raw(),
        &[10, 78, 166, 255, 206, 120, 46, 192, 23, 210, 105, 128]
    );
}

#[test]
fn glow_spreads_visible_premultiplied_alpha_outside_the_source() {
    let mut source = RgbaImage::new(7, 7);
    source.put_pixel(3, 3, Rgba([255, 255, 255, 255]));
    let target = render_effect(
        &source,
        EvaluatedEffect::Glow {
            threshold: 0.5,
            radius: 2.0,
            intensity: 1.0,
            colour: [255, 0, 0, 255],
        },
    );

    assert_eq!(target.get_pixel(3, 3)[0], 255);
    let horizontal = target.get_pixel(2, 3);
    let vertical = target.get_pixel(3, 2);
    assert!(horizontal[0] > 0 && horizontal[3] > 0);
    assert!(vertical[0] > 0 && vertical[3] > 0);
    assert_eq!(horizontal, vertical);
    assert!(target.get_pixel(2, 3)[3] > target.get_pixel(1, 3)[3]);
    assert_eq!(target.get_pixel(2, 3)[1], 0);
    assert_eq!(target.get_pixel(2, 3)[2], 0);
}

#[test]
fn gaussian_kernels_are_reused_and_bounded_through_pass_execution() {
    let source = RgbaImage::from_pixel(3, 3, Rgba([255, 255, 255, 255]));
    let _ = render_effect(&source, EvaluatedEffect::GaussianBlur { radius: 2.0 });
    let after_first = gaussian_kernel_cache_len();
    let _ = render_effect(&source, EvaluatedEffect::GaussianBlur { radius: 2.0 });
    assert_eq!(gaussian_kernel_cache_len(), after_first);
    for radius in 1..24 {
        let _ = render_effect(
            &source,
            EvaluatedEffect::GaussianBlur {
                radius: f64::from(radius),
            },
        );
    }
    assert!(gaussian_kernel_cache_len() <= 16);
}

#[test]
fn optimized_gaussian_convolution_is_byte_identical_to_reference() {
    for (width, height) in [(1, 1), (1, 257), (257, 1), (17, 19)] {
        let mut source = RgbaImage::new(width, height);
        for (index, pixel) in source.pixels_mut().enumerate() {
            *pixel = Rgba([
                (index * 17) as u8,
                (index * 31) as u8,
                (index * 47) as u8,
                (index * 29) as u8,
            ]);
        }

        for radius in [1.0, 2.5, 4.0, 16.0, 32.0] {
            let kernel = GaussianKernel::new(radius);
            for horizontal in [true, false] {
                let mut optimized = RgbaImage::new(source.width(), source.height());
                let mut reference = RgbaImage::new(source.width(), source.height());
                convolve(&source, &mut optimized, &kernel, horizontal);
                convolve_reference(&source, &mut reference, &kernel, horizontal);
                assert_eq!(
                    optimized, reference,
                    "size={width}x{height}, radius={radius}, horizontal={horizontal}"
                );
            }
        }
    }
}

#[test]
fn sparse_gaussian_matches_full_frame_reference_with_reused_targets() {
    let mut actual = RgbaImage::from_pixel(19, 13, Rgba([255; 4]));
    let mut expected = actual.clone();
    let mut actual_second = actual.clone();
    let mut expected_second = actual.clone();
    for (left, top, right, bottom) in [
        (3, 2, 15, 10),
        (8, 6, 9, 7),
        (0, 0, 1, 1),
        (18, 12, 19, 13),
        (0, 5, 19, 6),
        (5, 0, 6, 13),
        (1, 1, 18, 12),
        (0, 0, 0, 0),
    ] {
        let source = RgbaImage::from_fn(19, 13, |x, y| {
            let alpha = if (left..right).contains(&x)
                && (top..bottom).contains(&y)
                && (left != 1 || !(3..=16).contains(&x))
            {
                ((x * 17 + y * 23) % 255 + 1) as u8
            } else {
                0
            };
            Rgba([211, (x * 13) as u8, (y * 19) as u8, alpha])
        });
        for radius in [1.0, 2.5, 8.0, 32.0] {
            let kernel = GaussianKernel::new(radius);
            for horizontal in [true, false] {
                convolve(&source, &mut actual, &kernel, horizontal);
                convolve_reference(&source, &mut expected, &kernel, horizontal);
                assert_eq!(
                    actual, expected,
                    "region {left},{top},{right},{bottom}, radius {radius}, horizontal {horizontal}"
                );
            }
            convolve(&source, &mut actual, &kernel, true);
            convolve(&actual, &mut actual_second, &kernel, false);
            convolve_reference(&source, &mut expected, &kernel, true);
            convolve_reference(&expected, &mut expected_second, &kernel, false);
            assert_eq!(actual_second, expected_second, "two-pass radius {radius}");
        }
    }
}

#[test]
#[ignore = "manual release Gaussian bounds benchmark"]
fn gaussian_bounds_benchmark() {
    use std::{hint::black_box, time::Instant};
    let (width, height) = (1280, 720);
    for pattern in ["opaque", "near_full", "islands", "sparse"] {
        let source = RgbaImage::from_fn(width, height, |x, y| {
            let visible = match pattern {
                "opaque" => true,
                "near_full" => x > 0 && y > 0 && x + 1 < width && y + 1 < height,
                "islands" => {
                    ((1..5).contains(&x) && (1..5).contains(&y))
                        || ((width - 5..width - 1).contains(&x)
                            && (height - 5..height - 1).contains(&y))
                }
                _ => {
                    (width / 3..width * 2 / 3).contains(&x)
                        && (height / 3..height * 2 / 3).contains(&y)
                }
            };
            Rgba([211, x as u8, y as u8, if visible { 255 } else { 0 }])
        });
        let mut intermediate = RgbaImage::new(width, height);
        let mut target = RgbaImage::new(width, height);
        for radius in [1.0, 8.0] {
            let kernel = GaussianKernel::new(radius);
            convolve(&source, &mut intermediate, &kernel, true);
            convolve(&intermediate, &mut target, &kernel, false);
            for sample in 0..5 {
                let started = Instant::now();
                convolve(&source, &mut intermediate, &kernel, true);
                convolve(&intermediate, &mut target, &kernel, false);
                black_box(target.as_raw());
                println!(
                    "gaussian_bounds pattern={pattern} width={width} height={height} radius={radius} sample={sample} wall_ms={:.3} scope=prepared_two_pass",
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
    }
}

#[test]
fn small_sampling_radius_remains_a_cpu_effect_for_directional_and_motion_blur() {
    let mut source = RgbaImage::new(3, 1);
    source.put_pixel(1, 0, Rgba([255, 255, 255, 255]));
    let mut directional = RgbaImage::new(3, 1);
    let mut motion = RgbaImage::new(3, 1);
    blur(&source, &mut directional, 0.12, Some(0.0), None);
    blur(&source, &mut motion, 0.12, Some(0.0), Some(9));
    assert_ne!(directional, source);
    assert_ne!(motion, source);
}

#[test]
fn arbitrary_five_pass_sequence_executes_each_pass_in_order() {
    let passes = [
        EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform {
                    matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    offset: [1.0, 0.0, 0.0],
                },
            },
            EffectResource::Current,
            EffectResource::Current,
        ),
        EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform {
                    matrix: [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
                    offset: [0.0; 3],
                },
            },
            EffectResource::Current,
            EffectResource::Current,
        ),
        EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform {
                    matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 2.0]],
                    offset: [0.0; 3],
                },
            },
            EffectResource::Current,
            EffectResource::Current,
        ),
        EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform {
                    matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    offset: [5.0, 6.0, 7.0],
                },
            },
            EffectResource::Current,
            EffectResource::Current,
        ),
        EffectPass::new(
            EffectOperation::ApplyColourTransform {
                transform: ColourTransform {
                    matrix: [[1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                    offset: [0.0; 3],
                },
            },
            EffectResource::Current,
            EffectResource::Current,
        ),
    ];
    let source = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
    let mut surfaces = EffectSurfacePool::new(1, 1);
    surfaces.begin_from(&source);

    execute_effect_pass_sequence(&mut surfaces, &passes, None, false, false, &[]);

    assert_eq!(surfaces.current().get_pixel(0, 0), &Rgba([42, 17, 67, 255]));
    assert_eq!(surfaces.stats().reuses, 4);
}

#[test]
fn original_dependent_passes_pin_retained_storage_without_extra_frame_copy() {
    let mut source = RgbaImage::new(3, 3);
    source.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
    let mut surfaces = EffectSurfacePool::new(3, 3);
    surfaces.begin_from(&source);
    let copied_before_effect = surfaces.stats().copy_bytes;

    apply_chain(
        &mut surfaces,
        &[EvaluatedEffect::Glow {
            threshold: 0.0,
            radius: 1.0,
            intensity: 1.0,
            colour: [255, 64, 0, 255],
        }],
        &mut CpuHotPathTimings::default(),
        true,
        &[],
    );

    assert_eq!(surfaces.stats().copy_bytes, copied_before_effect);
    assert_eq!(surfaces.stats().reuses, 2);
    assert_eq!(surfaces.stats().allocations, 3);
    assert_eq!(surfaces.stats().retained_buffers, 3);
}

fn convolve_reference(
    source: &RgbaImage,
    target: &mut RgbaImage,
    kernel: &GaussianKernel,
    horizontal: bool,
) {
    let width = source.width() as i32;
    let height = source.height() as i32;
    for y in 0..height {
        for x in 0..width {
            let mut premultiplied = [0.0; 3];
            let mut alpha = 0.0;
            for (index, offset) in (-kernel.radius..=kernel.radius).enumerate() {
                let (sample_x, sample_y) = if horizontal {
                    ((x + offset).clamp(0, width - 1), y)
                } else {
                    (x, (y + offset).clamp(0, height - 1))
                };
                let pixel = source.get_pixel(sample_x as u32, sample_y as u32);
                let weight = kernel.weights[index];
                let sample_alpha = f64::from(pixel[3]) / 255.0;
                alpha += sample_alpha * weight;
                for channel in 0..3 {
                    premultiplied[channel] +=
                        f64::from(pixel[channel]) / 255.0 * sample_alpha * weight;
                }
            }
            let rgb = if alpha <= 0.000_000_1 {
                [0; 3]
            } else {
                premultiplied.map(|value| (value / alpha * 255.0).round().clamp(0.0, 255.0) as u8)
            };
            target.put_pixel(
                x as u32,
                y as u32,
                Rgba([rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8]),
            );
        }
    }
}

mod integer_gaussian_tests {
    use super::*;
    #[test]
    fn integer_profile_preserves_constant_partial_alpha_at_every_radius() {
        for quarter in 0..=128 {
            let kernel = GaussianKernel::new(f64::from(quarter) / 4.0);
            for colour in [[255; 4], [17, 99, 201, 128], [255, 0, 0, 0]] {
                let source = RgbaImage::from_pixel(2, 2, Rgba(colour));
                let expected = if colour[3] == 0 {
                    RgbaImage::new(2, 2)
                } else {
                    source.clone()
                };
                for horizontal in [false, true] {
                    let mut target = RgbaImage::from_pixel(2, 2, Rgba([1, 2, 3, 4]));
                    convolve_integer(&source, &mut target, &kernel, horizontal);
                    assert_eq!(target, expected);
                }
            }
        }
    }
}
