//! Compilation from validated projects to renderer-only data.

mod compiler;
mod evaluation;
mod model;
mod schedule;

pub use compiler::{CompileOptions, compile};
pub(crate) use evaluation::evaluate;
pub use evaluation::{
    ColourTransform, EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource,
};
pub use model::*;
pub(crate) use schedule::{ActiveSchedule, ScheduleAction};
