//! Compilation from validated v1 projects to renderer-only data.

mod compiler;
mod model;
mod schedule;

pub use compiler::{CompileOptions, compile};
pub use model::*;
pub use schedule::{ActiveSchedule, ScheduleAction};
