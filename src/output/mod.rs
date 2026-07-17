//! Output paths, temporary files, and publication.

mod paths;
mod progress;
mod report;
mod result;
mod terminal;

pub use paths::{OutputTarget, effective_parent};
pub use progress::{ProgressFormat, write_progress};
pub use report::write_report;
pub use terminal::{ResultFormat, print_failure, print_success};
