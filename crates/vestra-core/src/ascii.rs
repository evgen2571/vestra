//! Shared ASCII resource identity and evaluated frame parameters.
use crate::{
    project::{AsciiColorMode, AsciiGlyphStyle, AsciiMode},
    stylization::EvaluatedPalette,
};
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GlyphAtlasSpec {
    pub font: Option<String>,
    pub characters: String,
    pub edge_characters: String,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AsciiParameters {
    pub atlas: usize,
    pub glyph_count: u32,
    pub glyph_style: AsciiGlyphStyle,
    pub mode: AsciiMode,
    pub color_mode: AsciiColorMode,
    pub foreground: [u8; 4],
    pub background: [u8; 4],
    pub palette: EvaluatedPalette,
    pub invert: bool,
    pub cell_width: u32,
    pub cell_height: u32,
    pub edge_threshold: f64,
    pub edge_strength: f64,
    pub source_mix: f64,
    pub amount: f64,
}

#[cfg(test)]
mod tests {
    use crate::{
        project::{Asset, AssetType, Effect, Project},
        validation::{ResourceLimits, validate},
    };
    fn fixture() -> Project {
        let mut project: Project =
            serde_json::from_str(include_str!("../../../examples/effects/ascii.json")).unwrap();
        project
            .visual
            .post_effects
            .push(project.visual.clips[0].effects.remove(0));
        project
    }
    #[test]
    fn ascii_font_reference_and_character_validation_is_portable() {
        let mut project = fixture();
        assert!(
            validate(&project, ResourceLimits::default()).is_valid(),
            "{:?}",
            validate(&project, ResourceLimits::default()).diagnostics()
        );
        let effect = project
            .visual
            .post_effects
            .first_mut()
            .expect("ASCII example global effect");
        let Effect::Ascii {
            font, characters, ..
        } = effect
        else {
            panic!("ASCII example");
        };
        *font = Some("custom".into());
        *characters = " .#".into();
        let invalid = validate(&project, ResourceLimits::default());
        assert!(
            invalid
                .diagnostics()
                .iter()
                .any(|d| d.code == "VESTRA-ASCII-FONT")
        );
        project.assets.push(Asset {
            id: "custom".into(),
            kind: AssetType::Font,
            source: "custom.ttf".into(),
        });
        assert!(
            validate(&project, ResourceLimits::default()).is_valid(),
            "{:?}",
            validate(&project, ResourceLimits::default()).diagnostics()
        );
        let Effect::Ascii { characters, .. } = &mut project.visual.post_effects[0] else {
            unreachable!()
        };
        *characters = "\n".into();
        assert!(
            validate(&project, ResourceLimits::default())
                .diagnostics()
                .iter()
                .any(|d| d.code == "VESTRA-ASCII-CHARACTERS")
        );
    }
    #[test]
    fn custom_ascii_clip_font_is_collected_and_atlas_is_shared() {
        use crate::plan::{CompileOptions, PlanCompileInput, compile};
        use std::{
            collections::BTreeMap,
            path::{Path, PathBuf},
        };
        let mut project: Project =
            serde_json::from_str(include_str!("../../../examples/effects/ascii.json")).unwrap();
        let Effect::Ascii { font, .. } = &mut project.visual.clips[0].effects[0] else {
            panic!("ASCII fixture")
        };
        *font = Some("custom".into());
        project.assets.push(Asset {
            id: "custom".into(),
            kind: AssetType::Font,
            source: "custom.ttf".into(),
        });
        project
            .visual
            .post_effects
            .push(project.visual.clips[0].effects[0].clone());
        let paths = BTreeMap::from([("custom".into(), PathBuf::from("/resolved/custom.ttf"))]);
        let durations = BTreeMap::new();
        let plan = compile(
            PlanCompileInput::new(
                &project,
                ResourceLimits::default(),
                Path::new("/projects"),
                &paths,
                &durations,
                4.0,
                (30, 1),
                120,
                &[],
            ),
            CompileOptions::default(),
        )
        .expect("ASCII custom font compiles");
        assert_eq!(plan.fonts.len(), 1);
        assert_eq!(plan.fonts[0].id, "custom");
        assert_eq!(plan.glyph_atlases.len(), 1);
        assert_eq!(plan.glyph_atlases[0].font.as_deref(), Some("custom"));
    }
    #[test]
    fn ascii_transition_fonts_validate_at_exact_paths_and_compile_in_all_scopes() {
        use crate::plan::{CompileOptions, PlanCompileInput, compile};
        use serde_json::json;
        use std::{
            collections::BTreeMap,
            path::{Path, PathBuf},
        };
        for scope in ["root", "group", "mask"] {
            for endpoint in ["outgoing", "incoming"] {
                let mut value: serde_json::Value = serde_json::from_str(include_str!(
                    "../../../examples/transitions/zoom-crossfade.json"
                ))
                .unwrap();
                let ascii: serde_json::Value =
                    serde_json::from_str(include_str!("../../../examples/effects/ascii.json"))
                        .unwrap();
                let mut effect = ascii["visual"]["clips"][0]["effects"][0].clone();
                effect["font"] = json!("custom");
                value["visual"]["transitions"][0]["definition"][endpoint]["effects"] =
                    json!([effect]);
                let nested = json!({"type":"group", "clips":value["visual"]["clips"], "transitions":value["visual"]["transitions"]});
                let prefix = match scope {
                    "group" => {
                        value["visual"] = json!({"clips":[{"id":"parent", "source":nested, "start":0, "duration":5, "layer":0, "opacity":{"base_value":1}}]});
                        "/visual/clips/0/source"
                    }
                    "mask" => {
                        value["visual"] = json!({"clips":[{"id":"parent", "source":{"type":"solid_color","colour":"#ffffff"}, "start":0, "duration":5, "layer":0, "opacity":{"base_value":1}, "masks":[{"id":"coverage", "input":{"type":"source", "source":nested, "mode":"alpha"}}]}]});
                        "/visual/clips/0/masks/0/input/source"
                    }
                    _ => "/visual",
                };
                let mut project: Project = serde_json::from_value(value).unwrap();
                let expected =
                    format!("{prefix}/transitions/0/definition/{endpoint}/effects/0/font");
                let invalid = validate(&project, ResourceLimits::default());
                assert!(
                    invalid
                        .diagnostics()
                        .iter()
                        .any(|d| d.code == "VESTRA-ASCII-FONT"
                            && d.pointer.as_deref() == Some(&expected)),
                    "{scope}/{endpoint}: {:?}",
                    invalid.diagnostics()
                );
                project.assets.push(Asset {
                    id: "custom".into(),
                    kind: AssetType::Image,
                    source: "custom.ttf".into(),
                });
                assert!(
                    validate(&project, ResourceLimits::default())
                        .diagnostics()
                        .iter()
                        .any(|d| d.code == "VESTRA-ASCII-FONT"
                            && d.pointer.as_deref() == Some(&expected))
                );
                project.assets.last_mut().unwrap().kind = AssetType::Font;
                let valid = validate(&project, ResourceLimits::default());
                assert!(
                    valid.is_valid(),
                    "{scope}/{endpoint}: {:?}",
                    valid.diagnostics()
                );
                let paths = project
                    .assets
                    .iter()
                    .map(|asset| {
                        (
                            asset.id.clone(),
                            PathBuf::from(format!("/resolved/{}", asset.id)),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let durations = BTreeMap::new();
                let plan = compile(
                    PlanCompileInput::new(
                        &project,
                        ResourceLimits::default(),
                        Path::new("/projects"),
                        &paths,
                        &durations,
                        5.0,
                        (30, 1),
                        150,
                        &[],
                    ),
                    CompileOptions::default(),
                )
                .expect("custom transition font compiles");
                assert_eq!(plan.fonts.len(), 1);
                assert_eq!(plan.glyph_atlases.len(), 1);
                assert_eq!(plan.glyph_atlases[0].font.as_deref(), Some("custom"));
            }
        }
    }
}
