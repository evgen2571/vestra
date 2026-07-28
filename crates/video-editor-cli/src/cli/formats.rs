//! Command-line output and backend format conversions.

use crate::output::{ProgressFormat, ResultFormat};
use clap::ValueEnum;
use video_editor::BackendPreference as RenderBackendPreference;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliResultFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliProgressFormat {
    Human,
    Json,
    None,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliRenderBackend {
    Auto,
    Cpu,
    Wgpu,
}

impl From<CliResultFormat> for ResultFormat {
    fn from(format: CliResultFormat) -> Self {
        match format {
            CliResultFormat::Human => Self::Human,
            CliResultFormat::Json => Self::Json,
        }
    }
}

impl From<CliProgressFormat> for ProgressFormat {
    fn from(format: CliProgressFormat) -> Self {
        match format {
            CliProgressFormat::Human => Self::Human,
            CliProgressFormat::Json => Self::Json,
            CliProgressFormat::None => Self::None,
        }
    }
}

impl From<CliRenderBackend> for RenderBackendPreference {
    fn from(value: CliRenderBackend) -> Self {
        match value {
            CliRenderBackend::Auto => Self::Auto,
            CliRenderBackend::Cpu => Self::Cpu,
            CliRenderBackend::Wgpu => Self::Wgpu,
        }
    }
}
