use super::*;

#[test]
fn compiles_transitions_and_flashes_to_normal_layers() {
    let validated = load_and_validate(
        std::path::Path::new("examples/projects/animation-effects.json"),
        &ValidationOptions {
            check_backend: false,
            ..ValidationOptions::default()
        },
    )
    .expect("valid project");
    let plan = compile(&validated, CompileOptions::default()).expect("plan");
    assert_eq!(plan.frame_count, 144);
    assert_eq!(plan.images.len(), 2);
    assert!(
        plan.layers
            .iter()
            .any(|layer| matches!(layer.source, CompiledVisualSource::SolidColor { .. }))
    );
    assert!(
        plan.layers
            .iter()
            .any(|layer| !layer.opacity_contributions.is_empty())
    );
    assert_eq!(plan.compilation.keyframe_count, 12);
    assert_eq!(plan.compilation.compiled_transition_association_count, 2);
}

#[test]
fn frame_intervals_are_half_open() {
    assert_eq!(first_frame_at_or_after(0, (24, 1)).expect("frame"), 0);
    assert_eq!(
        first_frame_at_or_after(1_000_000_000, (24, 1)).expect("frame"),
        24
    );
    assert_eq!(
        first_frame_at_or_after(1_000_000_001, (24, 1)).expect("frame"),
        25
    );
}

#[test]
fn flash_without_fade_out_keeps_constant_opacity_until_its_end() {
    let layer = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash compiles");
    assert_eq!(layer.opacity.base_value, 0.7);
    assert!(layer.opacity.keyframes.is_empty());
}

#[test]
fn flash_fade_out_holds_then_reaches_zero_at_end() {
    let layer = flashes::compile(&flash(0.0, 0.5), (24, 1), 100).expect("flash compiles");
    assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
    assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
}

#[test]
fn flash_fade_in_only_reaches_and_holds_configured_opacity() {
    let layer = flashes::compile(&flash(0.5, 0.0), (24, 1), 100).expect("flash");
    assert_eq!(layer.opacity.evaluate(0), 0.0);
    assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
    assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
    assert_eq!(layer.opacity.evaluate(1_999_999_999), 0.7);
}

#[test]
fn flash_with_both_fades_holds_between_their_boundaries() {
    let layer = flashes::compile(&flash(0.5, 0.5), (24, 1), 100).expect("flash");
    assert_eq!(layer.opacity.evaluate(250_000_000), 0.35);
    assert_eq!(layer.opacity.evaluate(500_000_000), 0.7);
    assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.7);
    assert_eq!(layer.opacity.evaluate(1_750_000_000), 0.35);
    assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
}

#[test]
fn flash_fades_can_fill_the_entire_interval() {
    let layer = flashes::compile(&flash(1.0, 1.0), (24, 1), 100).expect("flash");
    assert_eq!(layer.opacity.evaluate(1_000_000_000), 0.7);
    assert_eq!(layer.opacity.evaluate(1_500_000_000), 0.35);
    assert_eq!(layer.opacity.evaluate(2_000_000_000), 0.0);
    assert_eq!(layer.end_frame, 72);
}

#[test]
fn zero_frame_layers_do_not_count_toward_active_layer_limit() {
    let mut layer = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
    layer.start_frame = 12;
    layer.end_frame = 12;
    limits::enforce_active_layer_limit(&[layer], 0).expect("zero-frame layer is never active");
}
