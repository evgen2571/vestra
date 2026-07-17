//! Output paths, temporary files, and publication.

mod paths;
mod report;

pub use paths::{OutputTarget, effective_parent};
pub use report::write_report;
