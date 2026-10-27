//! Cross-backend tests use only the facade and existing backend lifecycle APIs.
#![cfg(all(feature = "cpu", feature = "wgpu"))]
#![allow(
    clippy::result_large_err,
    reason = "renderer tests inspect structured diagnostics"
)]

pub use vestra_core::timeline;
pub use vestra_render::*;
#[path = "support/crops_gpu.rs"]
mod crop_gpu_tests;
#[path = "../../../tests/render_effect_cases.rs"]
mod effect_cases;
#[path = "support/particle_fixture.rs"]
mod fixture_tests;
#[path = "support/gpu.rs"]
mod gpu;
#[path = "support/lifecycle.rs"]
mod lifecycle;
#[path = "support/parity_gpu.rs"]
mod parity_gpu_tests;
#[path = "support/resources_gpu.rs"]
mod resource_gpu_tests;
#[path = "support/stylization.rs"]
mod stylization_tests;
#[path = "support/fixture_loader.rs"]
mod test_support;
