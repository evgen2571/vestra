//! Read-only topology of one compiled render plan.
//!
//! The WGPU preparation path needs a conservative view of every compiled
//! layer, including layers materialized by nested groups and owned mask
//! sources.  Keeping that walk here prevents resource estimates and working
//! texture allocation from growing subtly different notions of the plan.

use vestra_core::plan::{CompiledLayer, CompiledMaskInput, CompiledVisualSource, RenderPlan};

use crate::render::effects::compiled_effect_pass_requirements;

/// Structural facts shared by WGPU preparation and frame-plan construction.
///
/// This value borrows no plan data.  It is cheap to copy after being derived
/// once during backend preparation, while its fields remain private so
/// consumers cannot manufacture inconsistent topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PlanTopology {
    compiled_layer_count: usize,
    compiled_group_count: usize,
    compiled_mask_count: usize,
    compiled_matte_count: usize,
    required_group_depth: usize,
    requires_auxiliary: bool,
    has_masks: bool,
    has_mask_feather: bool,
}

impl PlanTopology {
    /// Derives the complete compiled-plan topology in one structural walk.
    #[must_use]
    pub(super) fn from_plan(plan: &RenderPlan) -> Self {
        let summary = summarize_layers(&plan.layers);
        Self {
            compiled_layer_count: summary.compiled_layer_count,
            compiled_group_count: summary.compiled_group_count,
            compiled_mask_count: summary.compiled_mask_count,
            compiled_matte_count: summary.compiled_matte_count,
            required_group_depth: summary.required_group_depth,
            requires_auxiliary: summary.requires_auxiliary
                || plan.post_effects.iter().any(|timed| {
                    compiled_effect_pass_requirements(&timed.effect).retains_original()
                }),
            has_masks: summary.has_masks,
            has_mask_feather: summary.has_mask_feather,
        }
    }

    pub(super) const fn compiled_layer_count(self) -> usize {
        self.compiled_layer_count
    }

    pub(super) const fn compiled_group_count(self) -> usize {
        self.compiled_group_count
    }

    pub(super) const fn compiled_mask_count(self) -> usize {
        self.compiled_mask_count
    }

    pub(super) const fn compiled_matte_count(self) -> usize {
        self.compiled_matte_count
    }

    pub(super) const fn required_group_depth(self) -> usize {
        self.required_group_depth
    }

    pub(super) const fn requires_auxiliary(self) -> bool {
        self.requires_auxiliary
    }

    pub(super) const fn has_masks(self) -> bool {
        self.has_masks
    }

    pub(super) const fn has_mask_feather(self) -> bool {
        self.has_mask_feather
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct TopologySummary {
    compiled_layer_count: usize,
    compiled_group_count: usize,
    compiled_mask_count: usize,
    compiled_matte_count: usize,
    required_group_depth: usize,
    requires_auxiliary: bool,
    has_masks: bool,
    has_mask_feather: bool,
}

fn summarize_layers(layers: &[CompiledLayer]) -> TopologySummary {
    let mut summary = TopologySummary::default();
    let mut has_matte = false;

    for layer in layers {
        let source = summarize_source(&layer.source);
        let mut owned_mask_layers = 0;
        let mut owned_mask_groups = 0;
        let mut owned_mask_masks = 0;
        let mut owned_mask_depth = 0;
        // A Source mask is lowered as one additional layer, followed by any
        // descendants of a Group source. Its descendants contribute masks
        // exactly as the compiled requirement walk did, but mattes remain
        // excluded because owned mask source compilation strips the source
        // layer's own matte metadata.
        for mask in &layer.masks {
            let CompiledMaskInput::Source { source, .. } = &mask.input else {
                continue;
            };
            let source = summarize_source(source);
            owned_mask_layers = add(owned_mask_layers, add(1, source.compiled_layer_count));
            owned_mask_groups = add(owned_mask_groups, source.compiled_group_count);
            owned_mask_masks = add(owned_mask_masks, source.compiled_mask_count);
            owned_mask_depth = owned_mask_depth.max(add(2, source.required_group_depth));
        }
        summary.compiled_layer_count = add(
            summary.compiled_layer_count,
            add(1, add(source.compiled_layer_count, owned_mask_layers)),
        );
        summary.compiled_group_count = add(
            summary.compiled_group_count,
            add(source.compiled_group_count, owned_mask_groups),
        );
        summary.compiled_mask_count = add(
            summary.compiled_mask_count,
            add(
                layer.masks.len(),
                add(source.compiled_mask_count, owned_mask_masks),
            ),
        );
        summary.compiled_matte_count = add(
            summary.compiled_matte_count,
            add(
                usize::from(layer.matte.is_some()),
                source.compiled_matte_count,
            ),
        );

        let layer_depth = source.required_group_depth.max(owned_mask_depth);
        summary.required_group_depth = summary.required_group_depth.max(layer_depth);

        let layer_has_masks = !layer.masks.is_empty() || layer.matte.is_some();
        summary.has_masks |= layer_has_masks || source.has_masks;
        summary.has_mask_feather |= source.has_mask_feather
            || layer.masks.iter().any(|mask| {
                mask.feather.authored_track.base_value > 0.0
                    || !mask.feather.authored_track.keyframes.is_empty()
                    || mask.feather.has_modifiers()
            });
        summary.requires_auxiliary |= layer_has_masks
            || layer
                .effects
                .iter()
                .any(|timed| compiled_effect_pass_requirements(&timed.effect).retains_original())
            || source.requires_auxiliary;
        has_matte |= layer.matte.is_some();
    }

    let matte_scratch = if has_matte {
        add(4, layers.len().saturating_mul(2))
    } else {
        0
    };
    summary.required_group_depth = summary.required_group_depth.max(matte_scratch);
    summary
}

fn summarize_source(source: &CompiledVisualSource) -> TopologySummary {
    let CompiledVisualSource::Group(composition) = source else {
        return TopologySummary::default();
    };

    let mut summary = summarize_layers(&composition.layers);
    summary.compiled_group_count = add(summary.compiled_group_count, 1);
    summary.required_group_depth = add(1, summary.required_group_depth);
    summary
}

fn add(left: usize, right: usize) -> usize {
    left.saturating_add(right)
}

#[cfg(test)]
mod tests {
    use super::PlanTopology;
    use crate::test_support::{ValidationOptions, load_and_validate};
    use vestra_core::plan::{CompileOptions, RenderPlan, compile};

    fn fixture(path: &str) -> RenderPlan {
        let validated = load_and_validate(
            std::path::Path::new(path),
            &ValidationOptions {
                check_backend: false,
            },
        )
        .expect("fixture validates");
        compile(validated, CompileOptions::default()).expect("fixture compiles")
    }

    #[test]
    fn empty_plan_has_no_compiled_structure() {
        let project = vestra_core::project::Project::from_json(
            r##"{
                "schema_version": 3,
                "output": {
                    "path": "empty.mp4", "width": 2, "height": 2,
                    "frame_rate": "1/1", "background": "#00000000",
                    "quality": "preview", "audio": false, "duration_mode": "explicit",
                    "duration": 1
                },
                "assets": [], "visual": {"clips": []}
            }"##,
        )
        .expect("empty project parses");
        let report = vestra_core::validation::validate(
            &project,
            vestra_core::validation::ResourceLimits::default(),
        );
        assert!(report.is_valid(), "{:?}", report.diagnostics());
        let assets = std::collections::BTreeMap::new();
        let durations = std::collections::BTreeMap::new();
        let input = vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &assets,
            &durations,
            1.0,
            (1, 1),
            1,
            &[],
        );
        let plan = compile(input, CompileOptions::default()).expect("empty project compiles");
        let topology = PlanTopology::from_plan(&plan);
        assert_eq!(topology.compiled_layer_count(), 0);
        assert_eq!(topology.compiled_group_count(), 0);
        assert_eq!(topology.compiled_mask_count(), 0);
        assert_eq!(topology.compiled_matte_count(), 0);
        assert_eq!(topology.required_group_depth(), 0);
        assert!(!topology.requires_auxiliary());
        assert!(!topology.has_masks());
        assert!(!topology.has_mask_feather());
    }

    #[test]
    fn nested_group_topology_tracks_layers_groups_and_depth() {
        let plan = fixture("examples/projects/animation-effects.json");
        let topology = PlanTopology::from_plan(&plan);
        assert!(topology.compiled_layer_count() >= plan.layers.len());
        assert!(topology.compiled_group_count() <= topology.compiled_layer_count());
        assert!(topology.required_group_depth() <= topology.compiled_group_count());
    }

    #[test]
    fn mask_and_matte_counts_preserve_their_distinct_compiled_roles() {
        let geometric = PlanTopology::from_plan(&fixture("examples/projects/geometric-masks.json"));
        assert_eq!(geometric.compiled_layer_count(), 1);
        assert_eq!(geometric.compiled_group_count(), 0);
        assert_eq!(geometric.compiled_mask_count(), 2);
        assert_eq!(geometric.compiled_matte_count(), 0);
        assert_eq!(geometric.required_group_depth(), 0);
        assert!(geometric.has_masks());
        assert!(geometric.requires_auxiliary());
        assert!(!geometric.has_mask_feather());

        let image = PlanTopology::from_plan(&fixture("examples/projects/image-masks.json"));
        assert_eq!(image.compiled_layer_count(), 1);
        assert_eq!(image.compiled_group_count(), 0);
        assert_eq!(image.compiled_mask_count(), 1);
        assert_eq!(image.compiled_matte_count(), 0);
        assert_eq!(image.required_group_depth(), 0);
        assert!(image.has_masks());
        assert!(image.has_mask_feather());

        let matte = PlanTopology::from_plan(&fixture("examples/projects/track-matte.json"));
        assert_eq!(matte.compiled_layer_count(), 2);
        assert_eq!(matte.compiled_group_count(), 0);
        assert_eq!(matte.compiled_mask_count(), 0);
        assert_eq!(matte.compiled_matte_count(), 1);
        assert_eq!(matte.required_group_depth(), 8);
        assert!(matte.has_masks());
        assert!(matte.requires_auxiliary());
        assert!(!matte.has_mask_feather());
    }

    #[test]
    fn owned_source_masks_include_nested_group_layers_and_groups() {
        let project: vestra_core::project::Project = serde_json::from_value(serde_json::json!({
            "schema_version": 4,
            "output": {
                "path": "owned-mask-topology.mp4", "width": 2, "height": 2,
                "frame_rate": "1/1", "background": "#00000000",
                "quality": "preview", "audio": false,
                "duration_mode": "explicit", "duration": 1
            },
            "assets": [],
            "visual": {"clips": [{
                "id": "owner",
                "source": {"type": "solid_color", "colour": "#ffffff"},
                "start": 0, "duration": 1, "layer": 0,
                "opacity": {"base_value": 1},
                "masks": [{
                    "id": "owned-group",
                    "input": {"type": "source", "mode": "alpha", "source": {
                        "type": "group", "clips": [{
                            "id": "nested-group",
                            "source": {"type": "group", "clips": [{
                                "id": "leaf",
                                "source": {"type": "solid_color", "colour": "#ffffff"},
                                "start": 0, "duration": 1, "layer": 0,
                                "opacity": {"base_value": 1}
                            }]},
                            "start": 0, "duration": 1, "layer": 0,
                            "opacity": {"base_value": 1}
                        }]
                    }},
                    "operation": "replace"
                }]
            }]}
        }))
        .expect("owned source mask project parses");
        let report = vestra_core::validation::validate(
            &project,
            vestra_core::validation::ResourceLimits::default(),
        );
        assert!(report.is_valid(), "{:?}", report.diagnostics());
        let assets = std::collections::BTreeMap::new();
        let durations = std::collections::BTreeMap::new();
        let input = vestra_core::plan::PlanCompileInput::new(
            &project,
            vestra_core::validation::ResourceLimits::default(),
            std::path::Path::new("."),
            &assets,
            &durations,
            1.0,
            (1, 1),
            1,
            &[],
        );
        let plan = compile(input, CompileOptions::default()).expect("owned source mask compiles");
        let topology = PlanTopology::from_plan(&plan);
        assert_eq!(topology.compiled_layer_count(), 4);
        assert_eq!(topology.compiled_group_count(), 2);
        assert_eq!(topology.compiled_mask_count(), 1);
        assert_eq!(topology.compiled_matte_count(), 0);
        assert_eq!(topology.required_group_depth(), 4);
        assert!(topology.has_masks());
        assert!(topology.requires_auxiliary());
    }
}
