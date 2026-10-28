// Ascii semantics; registration is derived from its authoritative declaration.

use crate::Category;
use crate::Diagnostic;
use crate::effect_definition::ScalarPropertyTarget;
use crate::effects::effect_amount_is_identity;
use crate::plan::CompiledEffect;
use crate::plan::EffectPassPlan;
use crate::plan::EvaluatedEffect;
use crate::plan::EvaluationContext;
use crate::plan::EvaluationError;
use crate::plan::ScalarSignalInterner;
use crate::plan::effect_passes::evaluated::ascii_pass_plan;
use crate::project::Effect;
use crate::project::parse_colour;
use crate::validation::effects::properties::invalid_effect;
use crate::validation::effects::properties::validate_palette;
use crate::validation::tracks;

pub(crate) fn compile(
    effect: &Effect,
    id: &str,
    interner: &mut ScalarSignalInterner,
) -> Result<CompiledEffect, Diagnostic> {
    let crate::project::Effect::Ascii {
        characters,
        edge_characters,
        font,
        glyph_style,
        mode,
        color_mode,
        foreground,
        background,
        palette,
        invert,
        period,
        amount,
        phase,
        cell_width,
        cell_height,
        edge_threshold,
        edge_strength,
        source_mix,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    macro_rules! scalar {
        ($property:expr, $target:expr) => {
            crate::plan::compiler::signals::compile_property(
                $property,
                id,
                $target.constraint(),
                interner,
            )?
        };
    }
    Ok(crate::plan::CompiledEffect::Ascii {
        glyphs: crate::ascii::GlyphAtlasSpec {
            font: font.clone(),
            characters: characters.clone(),
            edge_characters: edge_characters.clone(),
        },
        parameters: crate::ascii::AsciiParameters {
            atlas: usize::MAX,
            glyph_count: characters.chars().count() as u32,
            glyph_style: *glyph_style,
            mode: *mode,
            color_mode: *color_mode,
            foreground: parse_colour(foreground).expect("validated ASCII foreground"),
            background: parse_colour(background).expect("validated ASCII background"),
            palette: crate::stylization::compile_palette(palette).expect("validated ASCII palette"),
            invert: *invert,
            cell_width: 8,
            cell_height: 12,
            edge_threshold: 0.15,
            edge_strength: 1.0,
            source_mix: 0.0,
            amount: 1.0,
        },
        period: *period,
        amount: scalar!(amount, ScalarPropertyTarget::AsciiAmount),
        phase: scalar!(phase, ScalarPropertyTarget::AsciiPhase),
        cell_width: scalar!(cell_width, ScalarPropertyTarget::AsciiCellWidth),
        cell_height: scalar!(cell_height, ScalarPropertyTarget::AsciiCellHeight),
        edge_threshold: scalar!(edge_threshold, ScalarPropertyTarget::AsciiEdgeThreshold),
        edge_strength: scalar!(edge_strength, ScalarPropertyTarget::AsciiEdgeStrength),
        source_mix: scalar!(source_mix, ScalarPropertyTarget::AsciiSourceMix),
    })
}

pub(crate) fn evaluate(
    effect: &CompiledEffect,
    authored_time: u128,
    project_time: u128,
    context: &EvaluationContext<'_>,
) -> Result<EvaluatedEffect, EvaluationError> {
    let CompiledEffect::Ascii {
        parameters,
        period,
        amount,
        phase,
        cell_width,
        cell_height,
        edge_threshold,
        edge_strength,
        source_mix,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    Ok({
        let mut parameters = *parameters;
        parameters.amount = amount.evaluate(authored_time, project_time, context)?;
        parameters.cell_width = cell_width
            .evaluate(authored_time, project_time, context)?
            .round() as u32;
        parameters.cell_height = cell_height
            .evaluate(authored_time, project_time, context)?
            .round() as u32;
        parameters.edge_threshold =
            edge_threshold.evaluate(authored_time, project_time, context)?;
        parameters.edge_strength = edge_strength.evaluate(authored_time, project_time, context)?;
        parameters.source_mix = source_mix.evaluate(authored_time, project_time, context)?;

        let palette_mode = if parameters.color_mode == crate::project::AsciiColorMode::Rainbow {
            crate::project::PaletteMode::Rainbow
        } else {
            crate::project::PaletteMode::Gradient
        };
        parameters.palette = crate::stylization::evaluate_palette(
            &parameters.palette,
            palette_mode,
            phase.evaluate(authored_time, project_time, context)?,
            *period,
            authored_time,
            crate::project::PaletteInterpolation::Rgb,
        );
        EvaluatedEffect::Ascii { parameters }
    })
}

pub(crate) fn validate(
    effect: &Effect,
    active_duration: f64,
    path: &str,
    maximum_keyframes: usize,
    errors: &mut Vec<Diagnostic>,
    has_authored_audio: bool,
) {
    let crate::project::Effect::Ascii {
        characters,
        edge_characters,
        foreground,
        background,
        palette,
        period,
        amount,
        phase,
        cell_width,
        cell_height,
        edge_threshold,
        edge_strength,
        source_mix,
        ..
    } = effect
    else {
        panic!("effect dispatch matches its declaration")
    };
    let track = |track: &crate::project::ScalarProperty,
                 field,
                 target: ScalarPropertyTarget,
                 errors: &mut Vec<Diagnostic>| {
        tracks::validate_scalar_property(
            track,
            active_duration,
            &format!("{path}/{field}"),
            maximum_keyframes,
            errors,
            |value| target.authored_validation().accepts(value),
            has_authored_audio,
        );
    };
    {
        validate_palette(palette, *period, path, errors);
        if !(1..=256).contains(&characters.chars().count())
            || characters
                .chars()
                .chain(edge_characters.chars())
                .any(char::is_control)
            || edge_characters.chars().count() != 4
        {
            invalid_effect(
                errors,
                "VESTRA-ASCII-CHARACTERS",
                "ASCII requires 1–256 non-control fill characters and exactly four edge characters",
                path,
                "characters",
            );
        }
        for (name, colour) in [("foreground", foreground), ("background", background)] {
            if parse_colour(colour).is_none() {
                invalid_effect(
                    errors,
                    "VESTRA-ASCII-COLOUR",
                    "ASCII colors must be valid RGBA colors",
                    path,
                    name,
                );
            }
        }
        track(amount, "amount", ScalarPropertyTarget::AsciiAmount, errors);
        track(phase, "phase", ScalarPropertyTarget::AsciiPhase, errors);
        track(
            cell_width,
            "cell_width",
            ScalarPropertyTarget::AsciiCellWidth,
            errors,
        );
        track(
            cell_height,
            "cell_height",
            ScalarPropertyTarget::AsciiCellHeight,
            errors,
        );
        track(
            edge_threshold,
            "edge_threshold",
            ScalarPropertyTarget::AsciiEdgeThreshold,
            errors,
        );
        track(
            edge_strength,
            "edge_strength",
            ScalarPropertyTarget::AsciiEdgeStrength,
            errors,
        );
        track(
            source_mix,
            "source_mix",
            ScalarPropertyTarget::AsciiSourceMix,
            errors,
        );
    }
}

pub(crate) fn compiled_passes(effect: &CompiledEffect) -> EffectPassPlan {
    let CompiledEffect::Ascii { parameters, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    ascii_pass_plan(*parameters)
}

pub(crate) fn evaluated_passes(effect: &EvaluatedEffect) -> EffectPassPlan {
    let EvaluatedEffect::Ascii { parameters } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    ascii_pass_plan(*parameters)
}

pub(crate) fn evaluated_identity(effect: &EvaluatedEffect) -> bool {
    let EvaluatedEffect::Ascii { parameters } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    effect_amount_is_identity(parameters.amount) || parameters.source_mix == 1.0
}

pub(crate) fn static_identity(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Ascii { amount, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    amount.keyframes.is_empty() && effect_amount_is_identity(amount.base_value)
}

pub(crate) fn extra_dynamic(effect: &CompiledEffect) -> bool {
    let CompiledEffect::Ascii { period, .. } = effect else {
        panic!("effect dispatch matches its declaration")
    };
    period.is_some()
}

pub(crate) fn extra_modifiers(_effect: &CompiledEffect) -> bool {
    false
}

pub(crate) const fn pass_count(effect: &CompiledEffect) -> usize {
    effect.definition().estimated_pass_count
}

pub(crate) const fn extra_requirements(_effect: &CompiledEffect) -> bool {
    false
}

pub(crate) fn validate_assets(
    effect: &Effect,
    assets: &std::collections::BTreeMap<String, crate::project::AssetType>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) {
    if let crate::project::Effect::Ascii {
        font: Some(font), ..
    } = effect
        && assets.get(font) != Some(&crate::project::AssetType::Font)
    {
        errors.push(Diagnostic::error(
            "VESTRA-ASCII-FONT",
            Category::Semantic,
            format!("ASCII font '{font}' must refer to a Font asset"),
            format!("{path}/font"),
        ));
    }
}
