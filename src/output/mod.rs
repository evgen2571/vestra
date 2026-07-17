//! Output paths, temporary files, and publication.

mod paths;
mod progress;
mod report;
mod result;
mod terminal;

pub use paths::{OutputTarget, effective_parent};
pub use progress::{ProgressFormat, write_progress};
pub use report::{
    write_command_failure_report, write_plan_failure_report, write_render_failure_report,
    write_success_report,
};
pub use terminal::{ResultFormat, print_failure, print_success};
