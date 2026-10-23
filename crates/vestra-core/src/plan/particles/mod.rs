//! Compiled, random-access particle-system semantics.

#![allow(
    clippy::result_large_err,
    reason = "particle compilation preserves the repository's structured diagnostics"
)]

mod compiler;
mod evaluation;
mod model;

pub(super) use compiler::compile_with_signals;
pub use evaluation::StyledParticleIterator;
pub use model::{
    CompiledColourLifetimeCurve, CompiledParticleBurst, CompiledParticleSystem,
    CompiledScalarLifetimeCurve, EvaluatedParticleAppearance, EvaluatedParticleInstance,
    ParticleIdentity, RATE_SCALE,
};

#[cfg(test)]
mod tests;
