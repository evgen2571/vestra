//! Output paths, temporary files, and publication.

mod progress;
mod report;
mod result;
mod terminal;

pub use progress::{ProgressFormat, write_progress};
pub use report::{
    write_command_failure_report, write_plan_failure_report, write_render_failure_report,
    write_success_report,
};
pub use terminal::{ResultFormat, print_failure, print_success};
pub use video_editor_media::{OutputTarget, effective_parent};
