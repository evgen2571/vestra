//! Process-local identities for externally initiated runtime operations.

use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

static NEXT_OPERATION_ID: AtomicU64 = AtomicU64::new(1);

/// A process-local identity shared by all runtime events for one operation.
///
/// IDs are inexpensive to generate, copy, display, and serialize. They are
/// unique for the lifetime of a process; they are not globally unique.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct OperationId(u64);

impl OperationId {
    /// Generates the next process-local operation identity.
    #[must_use]
    pub fn new() -> Self {
        Self(NEXT_OPERATION_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Returns the stable numeric representation used by serialized events.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl Default for OperationId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OperationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
