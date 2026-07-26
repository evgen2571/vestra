use super::*;

#[test]
fn transient_preset_uses_its_own_interval_on_a_long_clip() {
    let mut layer = flashes::compile(&flash(0.0, 0.0), (30, 1), 300).expect("layer");
    layer.id = "clip".to_owned();
    let preset = crate::project::Preset::Impact {
        timing: crate::project::ActiveInterval {
            start: 1.25,
            duration: Some(0.28),
        },
        intensity: 1.0,
        seed: 7,
    };
    let mut compilation = CompilationStats::default();
    presets::apply(&mut layer, &preset, 4.0, &mut compilation).expect("preset compiles");
    assert_eq!(compilation.generated_local_effect_count, 3);
    assert_eq!(layer.transform_contributions.len(), 1);
    assert_eq!(layer.effects.len(), 3);
    assert!(matches!(
        layer.effects[0].effect,
        crate::plan::CompiledEffect::CameraShake { .. }
    ));
    assert!(matches!(
        layer.effects[1].effect,
        crate::plan::CompiledEffect::ChromaticAberration { .. }
    ));
    assert!(matches!(
        layer.effects[2].effect,
        crate::plan::CompiledEffect::Tint { .. }
    ));
    assert!(
        layer
            .transform_contributions
            .iter()
            .all(|contribution| contribution.start >= 1_250_000_000)
    );
    assert!(
        layer
            .effects
            .iter()
            .all(|effect| { effect.start == 1_250_000_000 && effect.end == 1_530_000_000 })
    );
    assert!(
        layer
            .effects
            .iter()
            .all(|effect| !effect.active_at(1_000_000_000))
    );
    assert!(
        layer
            .effects
            .iter()
            .all(|effect| !effect.active_at(1_530_000_000))
    );
}
