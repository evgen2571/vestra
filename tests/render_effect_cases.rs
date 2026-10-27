//! Literal evaluated-frame cases shared by backend planning and facade parity tests.
use vestra_core::{
    domain::{Crop, Point},
    plan::{EvaluatedEffect, EvaluatedFrame, EvaluatedSource},
    project,
};

pub fn spectrum_frame(bands: Vec<f32>, bar_gap_ratio: f64) -> EvaluatedFrame {
    EvaluatedFrame {
        time: 0,
        background: [0, 0, 0, 0],
        width: 10,
        height: 4,
        layers: vec![vestra_core::plan::EvaluatedLayer {
            compiled_layer_index: 0,
            visible: true,
            content_dependency: vestra_core::plan::TemporalDependency::Dynamic,
            transform: vestra_core::animation::Transform2D::identity(
                vestra_core::domain::Point { x: 0.5, y: 0.5 },
                vestra_core::domain::Point { x: 0.5, y: 0.5 },
            ),
            source: EvaluatedSource::Spectrum2D {
                bands,
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                bar_gap_ratio,
                min_bar_height_ratio: 0.0,
                layout: project::Spectrum2DLayout::default(),
                gradient: None,
                colour: [20, 30, 40, 128],
            },
            opacity: 1.0,
            effects: Vec::new(),
            masks: Vec::new(),
            matte: None,
            colour_transform: vestra_core::plan::ColourTransform::default(),
            blend_mode: project::BlendMode::Normal,
        }],
        post_effects: Vec::new(),
        evaluated_track_count: 0,
    }
}

pub fn set_spectrum_style(
    frame: &mut EvaluatedFrame,
    layout: project::Spectrum2DLayout,
    min_bar_height_ratio: f64,
    gradient: Option<(project::Spectrum2DGradientDirection, [u8; 4], [u8; 4])>,
) {
    let EvaluatedSource::Spectrum2D {
        layout: current_layout,
        min_bar_height_ratio: current_minimum,
        gradient: current_gradient,
        ..
    } = &mut frame.layers[0].source
    else {
        panic!("expected Spectrum2D source");
    };
    *current_layout = layout;
    *current_minimum = min_bar_height_ratio;
    *current_gradient = gradient;
}

pub fn evaluated_effect_pass_count(frame: &EvaluatedFrame) -> usize {
    frame
        .layers
        .iter()
        .flat_map(|layer| &layer.effects)
        .chain(&frame.post_effects)
        .map(|effect| vestra_core::plan::effect_pass_plan(effect).as_slice().len())
        .sum()
}

pub fn catalogue_cases(
    base: &EvaluatedFrame,
    mut check: impl FnMut(&EvaluatedFrame, &str, u8) -> bool,
) {
    let cases = [
        (
            "brightness",
            EvaluatedEffect::Brightness { amount: 0.12 },
            2,
        ),
        ("contrast", EvaluatedEffect::Contrast { amount: 1.18 }, 2),
        (
            "saturation",
            EvaluatedEffect::Saturation { amount: 0.63 },
            2,
        ),
        (
            "tint",
            EvaluatedEffect::Tint {
                colour: [20, 170, 255, 255],
                amount: 0.32,
            },
            2,
        ),
        (
            "gaussian",
            EvaluatedEffect::GaussianBlur { radius: 2.25 },
            4,
        ),
        (
            "glow",
            EvaluatedEffect::Glow {
                threshold: 0.4,
                radius: 2.25,
                intensity: 0.8,
                colour: [255, 170, 60, 255],
            },
            5,
        ),
        (
            "sharpen",
            EvaluatedEffect::Sharpen {
                amount: 0.65,
                radius: 2.25,
            },
            5,
        ),
        (
            "directional blur",
            EvaluatedEffect::DirectionalBlur {
                radius: 4.0,
                angle_degrees: 31.0,
            },
            4,
        ),
        (
            "small directional blur",
            EvaluatedEffect::DirectionalBlur {
                radius: 0.12,
                angle_degrees: 31.0,
            },
            2,
        ),
        (
            "zoom blur",
            EvaluatedEffect::ZoomBlur {
                radius: 8.0,
                samples: 9,
                anchor: Point { x: 0.37, y: 0.61 },
                direction: project::ZoomBlurDirection::Centered,
            },
            5,
        ),
        (
            "small zoom blur",
            EvaluatedEffect::ZoomBlur {
                radius: 0.12,
                samples: 9,
                anchor: Point { x: 0.37, y: 0.61 },
                direction: project::ZoomBlurDirection::Centered,
            },
            2,
        ),
        (
            "radial blur",
            EvaluatedEffect::RadialBlur {
                amount: 8.0,
                center: Point { x: 0.37, y: 0.61 },
            },
            4,
        ),
        (
            "motion tile",
            EvaluatedEffect::MotionTile {
                output_width_percent: 220.0,
                output_height_percent: 180.0,
                tile_center: Point { x: 0.37, y: 0.61 },
                mirror_edges: true,
            },
            2,
        ),
        (
            "motion tile repeat",
            EvaluatedEffect::MotionTile {
                output_width_percent: 220.0,
                output_height_percent: 180.0,
                tile_center: Point { x: 0.37, y: 0.61 },
                mirror_edges: false,
            },
            2,
        ),
        (
            "motion blur",
            EvaluatedEffect::MotionBlur {
                radius: 4.0,
                angle_degrees: 31.0,
                intensity: 1.0,
                shutter_angle: 180.0,
                max_radius: 8.0,
                samples: 9,
            },
            4,
        ),
        (
            "small motion blur",
            EvaluatedEffect::MotionBlur {
                radius: 0.12,
                angle_degrees: 31.0,
                intensity: 1.0,
                shutter_angle: 180.0,
                max_radius: 8.0,
                samples: 9,
            },
            2,
        ),
        (
            "chromatic aberration",
            EvaluatedEffect::ChromaticAberration {
                amount: 2.0,
                angle_degrees: 20.0,
            },
            4,
        ),
        (
            "vignette",
            EvaluatedEffect::Vignette {
                amount: 0.7,
                radius: 0.45,
                softness: 0.25,
                colour: [10, 20, 50, 255],
            },
            2,
        ),
        (
            "color adjustment",
            EvaluatedEffect::ColorAdjust {
                exposure: 0.2,
                gamma: 1.3,
                black_point: 0.05,
                white_point: 0.92,
            },
            3,
        ),
    ];
    for (name, effect, tolerance) in cases {
        let mut frame = base.clone();
        frame.layers[0].effects = vec![effect];
        if !check(&frame, name, tolerance) {
            return;
        }
    }

    let mut cropped = base.clone();
    let EvaluatedSource::Image {
        crop,
        cacheable_crop,
        ..
    } = &mut cropped.layers[0].source
    else {
        panic!("RGBA parity fixture must use an image source");
    };
    *crop = Crop {
        x: 0.2,
        y: 0.15,
        width: 0.55,
        height: 0.65,
    };
    *cacheable_crop = false;
    cropped.layers[0].effects = vec![EvaluatedEffect::MotionTile {
        output_width_percent: 220.0,
        output_height_percent: 180.0,
        tile_center: Point { x: 0.37, y: 0.61 },
        mirror_edges: true,
    }];
    assert!(check(&cropped, "motion tile with a nonzero crop", 2,));

    let chains = [
        (
            "basic colour plus gaussian",
            vec![
                EvaluatedEffect::Brightness { amount: 0.12 },
                EvaluatedEffect::Contrast { amount: 1.18 },
                EvaluatedEffect::GaussianBlur { radius: 2.25 },
            ],
            vec![],
            project::BlendMode::Normal,
            5,
        ),
        (
            "gaussian plus glow",
            vec![
                EvaluatedEffect::GaussianBlur { radius: 2.25 },
                EvaluatedEffect::Glow {
                    threshold: 0.4,
                    radius: 2.25,
                    intensity: 0.8,
                    colour: [255, 170, 60, 255],
                },
            ],
            vec![],
            project::BlendMode::Normal,
            6,
        ),
        (
            "glow plus sharpen",
            vec![
                EvaluatedEffect::Glow {
                    threshold: 0.4,
                    radius: 2.25,
                    intensity: 0.8,
                    colour: [255, 170, 60, 255],
                },
                EvaluatedEffect::Sharpen {
                    amount: 0.65,
                    radius: 2.25,
                },
            ],
            vec![],
            project::BlendMode::Overlay,
            7,
        ),
        (
            "sharpen plus glow and global sharpen",
            vec![
                EvaluatedEffect::Sharpen {
                    amount: 0.65,
                    radius: 2.25,
                },
                EvaluatedEffect::Glow {
                    threshold: 0.4,
                    radius: 2.25,
                    intensity: 0.8,
                    colour: [255, 170, 60, 255],
                },
            ],
            vec![EvaluatedEffect::Sharpen {
                amount: 0.45,
                radius: 2.25,
            }],
            project::BlendMode::Screen,
            7,
        ),
    ];
    for (name, effects, post_effects, blend_mode, tolerance) in chains {
        let mut frame = base.clone();
        frame.layers[0].effects = effects;
        frame.layers[0].blend_mode = blend_mode;
        frame.post_effects = post_effects;
        if name == "sharpen plus glow and global sharpen" {
            assert_eq!(evaluated_effect_pass_count(&frame), 10);
            assert!(
                evaluated_effect_pass_count(&frame) > 4,
                "the old four-pass allocation must not cover this chain"
            );
        }
        if !check(&frame, name, tolerance) {
            return;
        }
    }
}

pub fn spectrum_style_cases(mut check: impl FnMut(&EvaluatedFrame, &str, u8) -> bool) {
    let linear = |anchor, band_mapping| {
        project::Spectrum2DLayout::Linear(project::Spectrum2DLinearLayout {
            anchor,
            band_mapping,
        })
    };
    let radial = |start_angle_degrees, sweep_angle_degrees, direction, band_mapping| {
        project::Spectrum2DLayout::Radial(project::Spectrum2DRadialLayout {
            inner_radius_ratio: 0.35,
            start_angle_degrees,
            sweep_angle_degrees,
            direction,
            band_mapping,
        })
    };
    let cases = [
        (
            "linear top forward",
            linear(
                project::Spectrum2DLinearAnchor::Top,
                project::Spectrum2DBandMapping::Forward,
            ),
            0.0,
            None,
        ),
        (
            "linear center forward",
            linear(
                project::Spectrum2DLinearAnchor::Center,
                project::Spectrum2DBandMapping::Forward,
            ),
            0.0,
            None,
        ),
        (
            "linear top reverse",
            linear(
                project::Spectrum2DLinearAnchor::Top,
                project::Spectrum2DBandMapping::Reverse,
            ),
            0.0,
            None,
        ),
        (
            "linear center reverse",
            linear(
                project::Spectrum2DLinearAnchor::Center,
                project::Spectrum2DBandMapping::Reverse,
            ),
            0.0,
            None,
        ),
        (
            "linear bottom center out",
            linear(
                project::Spectrum2DLinearAnchor::Bottom,
                project::Spectrum2DBandMapping::CenterOut,
            ),
            0.10,
            None,
        ),
        (
            "linear top center out",
            linear(
                project::Spectrum2DLinearAnchor::Top,
                project::Spectrum2DBandMapping::CenterOut,
            ),
            0.0,
            Some((
                project::Spectrum2DGradientDirection::AcrossBands,
                [255, 0, 0, 0],
                [0, 0, 255, 255],
            )),
        ),
        (
            "linear center center out",
            linear(
                project::Spectrum2DLinearAnchor::Center,
                project::Spectrum2DBandMapping::CenterOut,
            ),
            0.0,
            Some((
                project::Spectrum2DGradientDirection::AlongBar,
                [0, 255, 0, 32],
                [255, 255, 0, 224],
            )),
        ),
        (
            "linear center out",
            linear(
                project::Spectrum2DLinearAnchor::Center,
                project::Spectrum2DBandMapping::CenterOut,
            ),
            0.25,
            Some((
                project::Spectrum2DGradientDirection::AcrossBands,
                [255, 0, 0, 80],
                [0, 0, 255, 220],
            )),
        ),
        (
            "radial outward full circle seam",
            radial(
                0.0,
                360.0,
                project::Spectrum2DRadialDirection::Outward,
                project::Spectrum2DBandMapping::Forward,
            ),
            0.0,
            Some((
                project::Spectrum2DGradientDirection::AcrossBands,
                [255, 0, 0, 255],
                [0, 0, 255, 255],
            )),
        ),
        (
            "radial reverse rotated",
            radial(
                450.0,
                180.0,
                project::Spectrum2DRadialDirection::Inward,
                project::Spectrum2DBandMapping::Reverse,
            ),
            0.0,
            Some((
                project::Spectrum2DGradientDirection::AlongBar,
                [0, 255, 0, 90],
                [255, 255, 0, 210],
            )),
        ),
        (
            "radial both rotated arc minimum",
            radial(
                -450.0,
                180.0,
                project::Spectrum2DRadialDirection::Both,
                project::Spectrum2DBandMapping::Forward,
            ),
            0.10,
            Some((
                project::Spectrum2DGradientDirection::AlongBar,
                [0, 255, 0, 0],
                [255, 255, 0, 255],
            )),
        ),
    ];
    for (name, layout, minimum, gradient) in cases {
        let mut frame = spectrum_frame(vec![0.1, 0.35, 0.7, 1.0], 0.08);
        frame.width = 32;
        frame.height = 32;
        set_spectrum_style(&mut frame, layout, minimum, gradient);
        if !check(&frame, name, 1) {
            return;
        }
    }
}
