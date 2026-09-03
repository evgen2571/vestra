//! Output paths, temporary files, and publication.

pub(crate) mod progress;
mod report;
mod result;
mod terminal;

pub use progress::ProgressFormat;
pub use report::{
    write_command_failure_report, write_plan_failure_report, write_render_failure_report,
    write_success_report,
};
pub use terminal::{ResultFormat, print_failure, print_success};
pub(crate) use terminal::{print_failure_stderr, print_success_stderr};
