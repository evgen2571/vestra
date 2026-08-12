//! Scalar signal contracts, preparation, and random-access evaluation.

mod model;
mod prepare;
mod prepared;

pub use model::*;
pub use prepare::{
    SignalPreparationError, prepare_scalar_signals, prepare_transformed_scalar_signal,
};
pub use prepared::PreparedScalarSignalError;
pub use prepared::{
    EvaluationContext, EvaluationError, PreparedScalarSignal, PreparedScalarSignals,
};

#[cfg(test)]
mod tests;
