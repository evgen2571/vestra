//! Command-line output and backend format conversions.

use crate::output::{ProgressFormat, ResultFormat};
use clap::ValueEnum;
use vestra::BackendPreference as RenderBackendPreference;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliResultFormat {
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum CliProgressFormat {
    Auto,
    Terminal,
    #[value(name = "human", hide = true)]
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
            CliProgressFormat::Auto => Self::Auto,
            CliProgressFormat::Terminal | CliProgressFormat::Human => Self::Terminal,
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
