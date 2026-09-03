//! CLI adaptation for the shared application-boundary observability config.

use vestra_observability::{ObservabilityConfig, Verbosity, try_init};

/// Installs the CLI's process-wide tracing subscriber.
pub(crate) fn init(verbosity: u8) -> Result<(), vestra_observability::ObservabilityError> {
    try_init(ObservabilityConfig::with_verbosity(Verbosity::from_count(
        verbosity,
    )))
}
