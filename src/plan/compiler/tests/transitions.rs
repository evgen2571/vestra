use super::*;
use std::collections::BTreeMap;

#[test]
fn directional_push_keeps_authored_tracks_and_has_correct_endpoints() {
    let mut outgoing = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
    outgoing.start_nanos = 0;
    transitions::push_layer(
        &mut outgoing,
        1_000_000_000,
        2_000_000_000,
        0.0,
        0.25,
        0.0,
        false,
    );
    let outgoing_contribution = &outgoing.transform_contributions[0];
    assert_eq!(
        outgoing_contribution
            .position_offset
            .evaluate(1_000_000_000)
            .x,
        0.0
    );
    assert_eq!(
        outgoing_contribution
            .position_offset
            .evaluate(2_000_000_000)
            .x,
        0.25
    );
    assert!(outgoing.transform.position.keyframes.is_empty());

    let mut incoming = flashes::compile(&flash(0.0, 0.0), (24, 1), 100).expect("flash");
    incoming.start_nanos = 0;
    transitions::push_layer(
        &mut incoming,
        1_000_000_000,
        2_000_000_000,
        0.0,
        0.25,
        0.0,
        true,
    );
    let incoming_contribution = &incoming.transform_contributions[0];
    assert_eq!(
        incoming_contribution
            .position_offset
            .evaluate(1_000_000_000)
            .x,
        -0.25
    );
    assert_eq!(
        incoming_contribution
            .position_offset
            .evaluate(2_000_000_000)
            .x,
        0.0
    );
    assert!(incoming.transform.position.keyframes.is_empty());
}

#[test]
fn flash_cut_holds_visibility_until_its_peak_then_switches() {
    let mut outgoing = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("out");
    outgoing.id = "out".to_owned();
    outgoing.start_nanos = 0;
    let mut incoming = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("in");
    incoming.id = "in".to_owned();
    incoming.start_nanos = 0;
    let mut layers = vec![outgoing, incoming];
    let indices = BTreeMap::from([("out".to_owned(), 0), ("in".to_owned(), 1)]);
    transitions::compile(
        &[crate::project::Transition::FlashCut {
            id: "cut".to_owned(),
            outgoing: "out".to_owned(),
            incoming: "in".to_owned(),
            start: 1.0,
            duration: 0.2,
            interpolation: crate::project::Interpolation::Named(
                crate::project::InterpolationName::Linear,
            ),
            colour: "#ffffff".to_owned(),
            intensity: 1.0,
        }],
        &indices,
        &mut layers,
        &mut CompilationStats::default(),
    )
    .expect("flash cut compiles");
    let outgoing = &layers[0].opacity_contributions[0];
    let incoming = &layers[1].opacity_contributions[0];
    assert_eq!(outgoing.evaluate(1_099_999_999), 1.0);
    assert_eq!(incoming.evaluate(1_099_999_999), 0.0);
    assert_eq!(outgoing.evaluate(1_100_000_000), 0.0);
    assert_eq!(incoming.evaluate(1_100_000_000), 1.0);
}

#[test]
fn zoom_blur_transition_compiles_to_a_radial_blur_effect() {
    let mut layer = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("layer");
    layer.start_nanos = 0;
    transitions::zoom_layer(
        &mut layer,
        1_000_000_000,
        1_200_000_000,
        1.0,
        1.1,
        Some(4.0),
    );
    assert!(matches!(
        layer.effects[0].effect,
        crate::plan::CompiledEffect::ZoomBlur { .. }
    ));
    assert!(!layer.effects[0].active_at(1_200_000_000));
}
