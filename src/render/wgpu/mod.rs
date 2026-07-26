//! Headless WGPU resource ownership for the render backend.

#![allow(
    clippy::result_large_err,
    reason = "WGPU preparation retains structured user-facing diagnostics"
)]

mod backend;
mod context;
mod diagnostics;
mod executor;
mod frame_plan;
mod parameters;
mod parity;
mod pipeline;
mod readback;
mod requirements;
mod resources;
pub(crate) mod support;
mod texture_pool;

#[cfg(test)]
#[path = "tests/parity.rs"]
mod parity_tests;
#[cfg(test)]
#[path = "tests/readback.rs"]
mod readback_tests;
#[cfg(test)]
#[path = "tests/requirements.rs"]
mod requirements_tests;
#[cfg(test)]
#[path = "tests/shader.rs"]
mod shader_tests;

pub use backend::WgpuBackend;
pub use parity::{FrameDifference, PixelMismatch, compare_rgba};

#[cfg(test)]
#[path = "tests/crops_gpu.rs"]
mod crop_gpu_tests;
#[cfg(test)]
#[path = "tests/gpu.rs"]
mod gpu;
#[cfg(test)]
#[path = "tests/parity_gpu.rs"]
mod parity_gpu_tests;
#[cfg(test)]
#[path = "tests/resources_gpu.rs"]
mod resource_gpu_tests;
