//! Compilation from validated v1 projects to renderer-only data.

mod compiler;
mod evaluated;
mod model;
mod schedule;

pub use compiler::{CompileOptions, compile};
pub(crate) use evaluated::evaluate;
pub use evaluated::{EvaluatedEffect, EvaluatedFrame, EvaluatedLayer, EvaluatedSource};
pub use model::*;
pub(crate) use schedule::{ActiveSchedule, ScheduleAction};
