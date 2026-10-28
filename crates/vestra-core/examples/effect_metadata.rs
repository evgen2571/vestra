//! Export development metadata without loading renderer or native Python dependencies.

use std::{env, error::Error, fs};

use serde::Serialize;
use serde_json::{Value, json};
use vestra_core::effect_definition::{
    EffectParameterDescriptor, EffectParameterKind, VisualEffectDescriptor,
    visual_effect_descriptors,
};

#[path = "../../../scripts/effects/schema.rs"]
mod visual_schema;

use visual_schema::visual_effect_schema_definitions;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Stage {
    PreTransform,
    PostTransform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum TemporalPolicy {
    FromProperties,
    AlwaysDynamic,
}

#[derive(Serialize)]
struct CatalogExecution {
    #[serde(skip_serializing)]
    id: &'static str,
    stage: Stage,
    temporal_policy: TemporalPolicy,
    estimated_pass_count: usize,
    retains_original: bool,
}

// Consume the same catalog input without compiling private runtime parameter expressions.
macro_rules! execution {
    ({id: $id:literal, class: $class:ident, scope: $scope:ident, stage: $stage:ident,
      passes: $passes:literal, temporal: $temporal:ident, retains_original: $retains:literal,
      parameters: $parameters:tt}) => {
        CatalogExecution {
            id: $id,
            stage: Stage::$stage,
            temporal_policy: TemporalPolicy::$temporal,
            estimated_pass_count: $passes,
            retains_original: $retains,
        }
    };
}
macro_rules! visual_effect_catalog {
    ($internal:ident => {
        definition: $internal_definition:tt,
        compiled: $internal_compiled:tt,
        evaluated: $internal_evaluated:tt
    }; $( $kind:ident => {
        module: $module:ident,
        definition: $definition:tt,
        authored: $authored:tt,
        compiled: $compiled:tt,
        evaluated: $evaluated:tt,
        scalars: [$($field:ident => $target:ident = $index:literal: $constraint:tt),* $(,)?],
        tracks: [$($track:ident => $plain_target:ident = $plain_index:literal: $validation:tt),* $(,)?]
    }),+ $(,)?) => {
        const CATALOG_EXECUTION: &[CatalogExecution] = &[
            execution!($internal_definition), $(execution!($definition)),+
        ];
    };
}

include!("../src/effect_definition/catalog_entries.rs");

#[derive(Serialize)]
struct GenerationDescriptor {
    #[serde(flatten)]
    descriptor: VisualEffectDescriptor,
    #[serde(flatten)]
    execution: &'static CatalogExecution,
}

fn generation_descriptors() -> Result<Vec<GenerationDescriptor>, Box<dyn Error>> {
    visual_effect_descriptors()
        .map(|descriptor| {
            let execution = CATALOG_EXECUTION
                .iter()
                .find(|entry| entry.id == descriptor.id)
                .ok_or_else(|| {
                    format!("effect {} has no catalog execution metadata", descriptor.id)
                })?;
            Ok(GenerationDescriptor {
                descriptor,
                execution,
            })
        })
        .collect()
}

fn synchronized_schema(template: &str) -> Result<String, Box<dyn Error>> {
    let mut schema: Value = serde_json::from_str(template)?;
    let defs = schema["$defs"]
        .as_object_mut()
        .ok_or("project schema has no $defs object")?;
    let branches = defs
        .get("effect")
        .and_then(|effect| effect.get("oneOf"))
        .and_then(Value::as_array)
        .ok_or("project schema has no effect oneOf array")?;
    let previous = branches
        .iter()
        .map(|branch| {
            branch
                .get("$ref")
                .and_then(Value::as_str)
                .and_then(|reference| reference.strip_prefix("#/$defs/"))
                .map(str::to_owned)
                .ok_or("project effect schema branch has no local definition reference")
        })
        .collect::<Result<Vec<_>, _>>()?;
    for name in previous {
        defs.remove(&name);
    }
    defs.extend(visual_effect_schema_definitions());
    Ok(serde_json::to_string_pretty(&schema)? + "\n")
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let template_path = arguments
        .next()
        .unwrap_or_else(|| "schemas/project.schema.json".into());
    if arguments.next().is_some() {
        return Err("usage: effect_metadata [project-schema-template]".into());
    }
    let schema = synchronized_schema(&fs::read_to_string(template_path)?)?;
    let effects = generation_descriptors()?;
    println!(
        "{}",
        serde_json::to_string(&json!({"effects": effects, "schema": schema}))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        CATALOG_EXECUTION, Stage, TemporalPolicy, generation_descriptors, synchronized_schema,
    };

    #[test]
    fn development_export_covers_every_canonical_effect_without_expanding_native_payloads() {
        let catalog_ids: BTreeSet<_> = CATALOG_EXECUTION.iter().map(|entry| entry.id).collect();
        assert_eq!(catalog_ids.len(), CATALOG_EXECUTION.len());
        assert_eq!(catalog_ids.len(), 24);
        let effects = generation_descriptors().unwrap();
        let authored_ids: BTreeSet<_> = effects.iter().map(|effect| effect.descriptor.id).collect();
        assert_eq!(authored_ids.len(), effects.len());
        assert_eq!(authored_ids.len(), 23);
        assert_eq!(
            catalog_ids
                .difference(&authored_ids)
                .copied()
                .collect::<Vec<_>>(),
            vec!["colour_transform"]
        );
        for effect in effects {
            let mut exported = serde_json::to_value(&effect).unwrap();
            let object = exported.as_object_mut().unwrap();
            for name in [
                "stage",
                "temporal_policy",
                "estimated_pass_count",
                "retains_original",
            ] {
                assert!(object.remove(name).is_some());
            }
            assert_eq!(exported, serde_json::to_value(effect.descriptor).unwrap());
        }
    }

    #[test]
    fn shared_execution_metadata_preserves_distinct_effect_requirements() {
        let effects = generation_descriptors().unwrap();
        let glow = effects
            .iter()
            .find(|effect| effect.descriptor.id == "glow")
            .unwrap();
        assert_eq!(glow.execution.stage, Stage::PostTransform);
        assert_eq!(glow.execution.estimated_pass_count, 4);
        assert!(glow.execution.retains_original);
        assert_eq!(
            glow.execution.temporal_policy,
            TemporalPolicy::FromProperties
        );
        let shake = effects
            .iter()
            .find(|effect| effect.descriptor.id == "camera_shake")
            .unwrap();
        assert_eq!(shake.execution.estimated_pass_count, 0);
        assert_eq!(
            shake.execution.temporal_policy,
            TemporalPolicy::AlwaysDynamic
        );
        let tile = effects
            .iter()
            .find(|effect| effect.descriptor.id == "motion_tile")
            .unwrap();
        assert_eq!(tile.execution.stage, Stage::PreTransform);
    }

    #[test]
    fn synchronization_preserves_schema_bytes_and_unrelated_definitions() {
        let template = include_str!("../../../schemas/project.schema.json");
        assert_eq!(synchronized_schema(template).unwrap(), template);
    }

    #[test]
    fn synchronization_replaces_stale_visual_branches() {
        let template = include_str!("../../../schemas/project.schema.json");
        let mut stale: serde_json::Value = serde_json::from_str(template).unwrap();
        stale["$defs"]["obsolete_effect"] = serde_json::json!({"type": "object"});
        stale["$defs"]["effect"]["oneOf"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"$ref": "#/$defs/obsolete_effect"}));
        stale["$defs"]["vignette_effect"]["properties"]
            .as_object_mut()
            .unwrap()
            .remove("colour");
        assert_eq!(synchronized_schema(&stale.to_string()).unwrap(), template);
    }

    #[test]
    fn synchronization_rejects_missing_template_structure() {
        assert!(
            synchronized_schema("{}")
                .unwrap_err()
                .to_string()
                .contains("$defs")
        );
        assert!(
            synchronized_schema(r#"{"$defs":{"effect":{"oneOf":[{}]}}}"#)
                .unwrap_err()
                .to_string()
                .contains("local definition reference")
        );
    }
}
