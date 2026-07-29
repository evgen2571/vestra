//! Deterministic compilation and per-frame evaluation for renderer backends.

mod compiler;
mod effect_passes;
mod evaluation;
mod input;
mod model;
mod schedule;

pub use compiler::{CompileOptions, compile};
pub use effect_passes::{EffectPass, EffectPassPlan, effect_pass_plan};
pub use evaluation::evaluate;
pub use evaluation::{
    ColourTransform, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource,
};
pub use input::PlanCompileInput;
pub use model::*;
pub use schedule::{ActiveSchedule, ScheduleAction, ScheduleCursor, ScheduleEvent};

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::{ActiveSchedule, CompileOptions, PlanCompileInput, compile, evaluate};
    use crate::{project::Project, validation::ResourceLimits};

    fn canonical_input() -> PlanCompileInput<'static> {
        let project = Box::leak(Box::new(
            serde_json::from_str::<Project>(include_str!(
                "../../../../examples/projects/animation-effects.json"
            ))
            .expect("fixture project"),
        ));
        let assets = Box::leak(Box::new(BTreeMap::from([
            ("red".to_owned(), PathBuf::from("/resolved/red.png")),
            ("blue".to_owned(), PathBuf::from("/resolved/blue.png")),
        ])));
        let durations = Box::leak(Box::new(BTreeMap::new()));
        PlanCompileInput::new(
            project,
            ResourceLimits::default(),
            std::path::Path::new("/projects"),
            assets,
            durations,
            6.0,
            (24, 1),
            144,
            &[],
        )
    }

    #[test]
    fn canonical_fixture_compiles_from_supplied_preflight_data() {
        let plan = compile(canonical_input(), CompileOptions::default()).expect("plan");
        assert_eq!((plan.canvas.width, plan.canvas.height), (320, 180));
        assert_eq!(plan.images.len(), 2);
        assert_eq!(plan.layers.len(), 3);
        assert_eq!(
            plan.configured_output,
            PathBuf::from("/projects/../output/animation-effects.mp4")
        );
    }

    #[test]
    fn schedule_and_evaluation_share_the_compiled_plan() {
        let plan = compile(canonical_input(), CompileOptions::default()).expect("plan");
        let schedule = ActiveSchedule::compile(&plan);
        let mut cursor = schedule.cursor();
        let active = cursor
            .events_at(0)
            .iter()
            .filter_map(|event| {
                (event.action == super::ScheduleAction::Activate).then_some(event.item)
            })
            .collect::<Vec<_>>();
        let frame = evaluate(&plan, &active, 0);
        assert_eq!(frame.layers.len(), 1);
    }
}
