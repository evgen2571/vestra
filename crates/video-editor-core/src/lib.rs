//! Deterministic, backend-independent editor model and timeline primitives.
//!
//! This internal workspace crate owns schema value types, diagnostics,
//! animation and timeline semantics. It must not depend on rendering, media,
//! application orchestration, or command-line code. Its public surface is
//! intentionally provisional until the future SDK crate defines the supported
//! external API.

extern crate self as video_editor_core;

pub mod animation;
pub mod diagnostic;
pub mod domain;
mod effect_definition;
pub mod effects;
pub mod output;
/// Deterministic output configuration, never media process behavior.
pub mod media {
    pub use crate::output::EncoderSettings;
}
/// Deterministic camera-shake transform evaluation.
pub mod camera_shake;
pub mod motion_bounds;
pub mod plan;
pub mod plan_audio;
pub mod plan_schedule;
pub mod plan_sizing;
/// Deterministic frame/time and preview-dimension conversion used by plan
/// compilation. No project loading or rendering dependency is involved.
pub mod plan_time;
/// Project-track normalization used by plan compilation.
pub mod plan_tracks;
pub mod timeline;
pub mod validation;

/// Canonical serializable project value objects.
///
/// The SDK crate owns project source context, validation, and environment
/// preflight. This crate remains deterministic and backend-independent.
#[path = "project/model/mod.rs"]
#[allow(
    clippy::module_inception,
    reason = "the compatibility module preserves the existing project model path"
)]
pub mod project;

pub use diagnostic::{Category, Diagnostic, Severity};

#[cfg(test)]
mod camera_shake_tests {
    use crate::{animation::Transform2D, camera_shake};

    fn transform() -> Transform2D {
        Transform2D {
            position: crate::domain::Point { x: 0.5, y: 0.5 },
            anchor: crate::domain::Point { x: 0.5, y: 0.5 },
            scale: crate::domain::Point { x: 1.0, y: 1.0 },
            rotation_radians: 0.0,
        }
    }

    #[test]
    fn camera_shake_is_seeded_and_deterministic() {
        let mut first = transform();
        let mut repeated = transform();
        let mut different_seed = transform();
        for (target, seed) in [
            (&mut first, 7),
            (&mut repeated, 7),
            (&mut different_seed, 8),
        ] {
            camera_shake::apply(target, 100_000_000, 0.02, 0.1, 0.01, 14.0, seed, 0.03, 0.22);
        }
        assert_eq!(first.position.x, repeated.position.x);
        assert_ne!(first.position.x, different_seed.position.x);
    }
}
