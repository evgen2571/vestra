//! Compilation from validated projects to renderer-only data.

mod colour_transform;
mod compiler;
mod evaluation;
mod model;
mod schedule;

pub use colour_transform::ColourTransform;
pub use compiler::{CompileOptions, compile};
pub(crate) use evaluation::evaluate;
pub use evaluation::{EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource};
pub use model::*;
pub(crate) use schedule::{ActiveSchedule, ScheduleAction};
