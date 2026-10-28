//! Frozen specimens captured at 8fb4847, independent of the new declarations.

use super::*;
use crate::plan::{EvaluationContext, PreparedScalarSignals};
use serde_json::{Value, json};

fn fingerprint(value: impl std::fmt::Debug) -> String {
    let hash = format!("{value:?}")
        .bytes()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    format!("{hash:016x}")
}

#[test]
fn pre_refactor_effect_specimens_preserve_serde_compilation_evaluation_and_topology() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../../../../python-tests/fixtures/effects/json-v1.json"
    ))
    .unwrap();
    let signals = PreparedScalarSignals::empty();
    let context = EvaluationContext::new(&signals);
    let mut actual = serde_json::Map::new();
    for (name, original) in fixtures["effects"].as_object().unwrap() {
        for animated in [false, true] {
            let mut specimen = original.clone();
            if animated {
                for field in specimen.as_object_mut().unwrap().values_mut() {
                    if let Some(base) = field.get("base_value").and_then(Value::as_f64) {
                        field["keyframes"] = json!([
                            {"time": 0.25, "value": base, "interpolation": "linear"},
                            {"time": 0.75, "value": base + 0.125, "interpolation": "linear"}
                        ]);
                    }
                }
            }
            let effect: crate::project::Effect = serde_json::from_value(specimen).unwrap();
            let compiled =
                compile(&effect, "baseline", &mut ScalarSignalInterner::default()).unwrap();
            let mut authored_targets = Vec::new();
            effect.for_each_scalar_property(|target, property| {
                authored_targets.push(target);
                assert_eq!(
                    serde_json::to_value(property).unwrap(),
                    serde_json::to_value(&effect).unwrap()[target.name()],
                    "{name}: authored field association"
                );
            });
            assert_eq!(authored_targets, effect.definition().scalar_properties);
            let mut properties = Vec::new();
            compiled.for_each_scalar_property(|target, property| {
                properties.push(format!("{target:?}:{property:?}"));
            });
            compiled.for_each_plain_track(|target, track| {
                properties.push(format!("{target:?}:{track:?}"));
            });
            let frames: Vec<_> = [0, 500_000_000, 2_000_000_000]
                .into_iter()
                .map(|time| {
                    let evaluated =
                        crate::plan::evaluate_effect(&compiled, time, time + 123, &context)
                            .unwrap();
                    fingerprint((
                        evaluated.clone(),
                        evaluated.is_identity(),
                        evaluated.is_pre_transform(),
                        evaluated.is_basic_colour_effect(),
                        crate::plan::effect_pass_plan(&evaluated),
                    ))
                })
                .collect();
            actual.insert(format!("{name}/{}", if animated { "animated" } else { "static" }), json!({
                "serde": fingerprint(serde_json::to_value(&effect).unwrap()),
                "compiled": fingerprint(&compiled),
                "properties": fingerprint(properties),
                "topology": fingerprint((crate::plan::compiled_effect_pass_plan(&compiled),
                    crate::plan::compiled_effect_pass_requirements(&compiled))),
                "classification": fingerprint((compiled.class(), compiled.estimated_pass_count(), compiled.keyframe_count(),
                    super::super::dependency::effect_dependency(&compiled))),
                "frames": frames
            }));
        }
    }
    let expected: Value =
        serde_json::from_str(include_str!("compatibility-baseline.json")).unwrap();
    assert_eq!(Value::Object(actual), expected);
}
