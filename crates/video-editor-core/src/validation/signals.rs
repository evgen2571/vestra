//! Semantic validation for canonical scalar signal descriptions.

use crate::{
    Category, Diagnostic,
    plan::{AudioFrequencyBand, ClampTransform, CubicResponseCurve, GainTransform, RemapTransform},
};

pub(super) fn validate_modifiers(
    modifiers: &[crate::project::ScalarModifier],
    path: &str,
    has_authored_audio: bool,
    errors: &mut Vec<Diagnostic>,
) {
    for (index, modifier) in modifiers.iter().enumerate() {
        let path = format!("{path}/modifiers/{index}/signal");
        match &modifier.signal.source {
            crate::project::ScalarSignalSource::Audio { feature, .. } => {
                if !has_authored_audio {
                    errors.push(Diagnostic::error(
                        "MVP-SIGNAL-MASTER-AUDIO",
                        Category::Semantic,
                        "Master audio signal requires authored audio material",
                        format!("{path}/source"),
                    ));
                }
                if let crate::project::AudioScalarFeature::BandEnergy { min_hz, max_hz } = feature
                    && let Err(error) = AudioFrequencyBand::new(*min_hz, *max_hz)
                {
                    let field = if !min_hz.is_finite() || *min_hz < 0.0 || min_hz >= max_hz {
                        "min_hz"
                    } else {
                        "max_hz"
                    };
                    errors.push(Diagnostic::error(
                        "MVP-SIGNAL-BAND",
                        Category::Semantic,
                        error.to_string(),
                        format!("{path}/source/feature/{field}"),
                    ));
                }
            }
        }
        for (transform_index, transform) in modifier.signal.transforms.iter().enumerate() {
            let transform_path = format!("{path}/transforms/{transform_index}");
            let result = match transform {
                crate::project::SignalTransform::Gain { gain } => {
                    GainTransform::new(*gain).map(|_| ())
                }
                crate::project::SignalTransform::Remap {
                    input_min,
                    input_max,
                    output_start,
                    output_end,
                } => RemapTransform::new(*input_min, *input_max, *output_start, *output_end)
                    .map(|_| ()),
                crate::project::SignalTransform::Clamp { min, max } => {
                    ClampTransform::new(*min, *max).map(|_| ())
                }
                crate::project::SignalTransform::Envelope { attack, release } => {
                    if attack.is_finite()
                        && release.is_finite()
                        && *attack >= 0.0
                        && *release >= 0.0
                    {
                        Ok(())
                    } else {
                        Err(crate::plan::SignalTransformContractError::NonFiniteParameter)
                    }
                }
                crate::project::SignalTransform::ResponseCurve { x1, y1, x2, y2 } => {
                    CubicResponseCurve::new(*x1, *y1, *x2, *y2).map(|_| ())
                }
            };
            if let Err(error) = result {
                let (field, message) = invalid_transform_field(transform, &error);
                errors.push(Diagnostic::error(
                    "MVP-SIGNAL-TRANSFORM",
                    Category::Semantic,
                    message,
                    format!("{transform_path}/{field}"),
                ));
            }
        }
    }
}

fn invalid_transform_field(
    transform: &crate::project::SignalTransform,
    error: &crate::plan::SignalTransformContractError,
) -> (&'static str, String) {
    match transform {
        crate::project::SignalTransform::Gain { .. } => ("gain", error.to_string()),
        crate::project::SignalTransform::Remap {
            input_min,
            input_max,
            output_start,
            output_end,
        } => (
            first_non_finite(&[
                ("input_min", *input_min),
                ("input_max", *input_max),
                ("output_start", *output_start),
                ("output_end", *output_end),
            ])
            .unwrap_or("input_min"),
            error.to_string(),
        ),
        crate::project::SignalTransform::Clamp { min, max } => (
            first_non_finite(&[("min", *min), ("max", *max)]).unwrap_or("min"),
            error.to_string(),
        ),
        crate::project::SignalTransform::Envelope { attack, release: _ } => (
            if !attack.is_finite() || *attack < 0.0 {
                "attack"
            } else {
                "release"
            },
            "envelope attack and release must be finite and non-negative".to_owned(),
        ),
        crate::project::SignalTransform::ResponseCurve { x1, y1, x2, y2 } => (
            first_non_finite(&[("x1", *x1), ("y1", *y1), ("x2", *x2), ("y2", *y2)]).unwrap_or_else(
                || {
                    if !(0.0..=1.0).contains(x1) {
                        "x1"
                    } else {
                        "x2"
                    }
                },
            ),
            error.to_string(),
        ),
    }
}

fn first_non_finite(values: &[(&'static str, f64)]) -> Option<&'static str> {
    values
        .iter()
        .find_map(|(field, value)| (!value.is_finite()).then_some(*field))
}
