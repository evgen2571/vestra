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
fn every_catalog_property_is_compiled_visited_and_validated() {
    use crate::effect_definition::{EffectParameterKind, tests::descriptor_specimen};

    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../python-tests/fixtures/effects/json-v1.json"
    ))
    .unwrap();
    for descriptor in crate::effect_definition::visual_effect_descriptors() {
        let specimen = descriptor_specimen(descriptor);
        let effect: crate::project::Effect = serde_json::from_value(specimen.clone()).unwrap();
        let compiled = compile(&effect, "test", &mut ScalarSignalInterner::default()).unwrap();
        assert_eq!(compiled.kind(), effect.kind(), "{}", descriptor.id);
        let mut scalar_targets = Vec::new();
        compiled.for_each_scalar_property(|target, property| {
            assert_eq!(
                property.constraint,
                target.constraint(),
                "{}",
                descriptor.id
            );
            scalar_targets.push(target);
        });
        assert_eq!(
            scalar_targets,
            descriptor
                .parameters
                .iter()
                .filter_map(|p| p.scalar_target)
                .collect::<Vec<_>>(),
            "{}: scalar descriptor and visitor coverage",
            descriptor.id
        );
        let mut plain_targets = Vec::new();
        compiled.for_each_plain_track(|target, _| plain_targets.push(target));
        assert_eq!(
            plain_targets,
            descriptor
                .parameters
                .iter()
                .filter_map(|p| p.plain_track_target)
                .collect::<Vec<_>>(),
            "{}: plain descriptor and visitor coverage",
            descriptor.id
        );
        let signals = PreparedScalarSignals::empty();
        let evaluated =
            crate::plan::evaluate_effect(&compiled, 0, 0, &EvaluationContext::new(&signals))
                .unwrap();
        assert_eq!(evaluated.kind(), compiled.kind(), "{}", descriptor.id);
        let conservative = crate::plan::compiled_effect_pass_plan(&compiled);
        let evaluated = crate::plan::effect_pass_plan(&evaluated);
        assert!(conservative.len() >= evaluated.len(), "{}", descriptor.id);

        for parameter in descriptor.parameters.iter().filter(|parameter| {
            matches!(
                parameter.kind,
                EffectParameterKind::ScalarProperty | EffectParameterKind::PlainTrack
            )
        }) {
            let mut invalid = specimen.clone();
            invalid[parameter.name]["keyframes"] = json!([
                {"time": -1.0, "value": invalid[parameter.name]["base_value"], "interpolation": "linear"}
            ]);
            let mut project = fixture["project"].clone();
            project["visual"]["clips"][0]["effects"] = json!([invalid]);
            let project: crate::project::Project = serde_json::from_value(project).unwrap();
            let report =
                crate::validation::validate(&project, crate::validation::ResourceLimits::default());
            let pointer = format!(
                "/visual/clips/0/effects/0/{}/keyframes/0/time",
                parameter.name
            );
            assert!(
                report
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.code == "VESTRA-KEYFRAME-TIME"
                        && diagnostic.pointer.as_deref() == Some(pointer.as_str())),
                "{}: missing validation for {}: {:?}",
                descriptor.id,
                parameter.name,
                report.diagnostics()
            );
        }
    }
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

#[test]
fn pre_refactor_track_remapping_and_normalization_are_preserved() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../../../../python-tests/fixtures/effects/json-v1.json"
    ))
    .unwrap();
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
            let identity_before = super::super::optimization::is_static_identity(&compiled);
            let mut normalized = compiled.clone();
            let normalized_count = super::super::optimization::normalize_effect(&mut normalized);
            let identity_after = super::super::optimization::is_static_identity(&normalized);
            let mut remapped = effect.clone();
            super::super::transitions::map_effect_times(&mut remapped, 2.5);
            actual.insert(format!("{name}/{}", if animated { "animated" } else { "static" }), json!({
                "remapped": fingerprint(serde_json::to_value(remapped).unwrap()),
                "normalized": fingerprint((normalized, normalized_count, identity_before, identity_after))
            }));
        }
    }
    let expected: Value = serde_json::from_str(include_str!("traversal-baseline.json")).unwrap();
    assert_eq!(Value::Object(actual), expected);
}
