#![allow(
    clippy::result_large_err,
    reason = "backend diagnostics retain structured user-facing context"
)]

use std::sync::atomic::AtomicBool;

use serde::Serialize;

use crate::{
    Diagnostic,
    plan::EvaluatedFrame,
    render::metrics::{PreparationStats, PreparationTimings},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderBackendKind {
    Cpu,
    Wgpu,
}

impl RenderBackendKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Wgpu => "wgpu",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AdapterMetadata {
    pub adapter_name: String,
    pub device_type: String,
    pub graphics_backend: String,
    pub driver_name: String,
    pub driver_info: String,
    pub vendor_id: u32,
    pub device_id: u32,
}

/// Conservative performance classification for benchmark reporting. An
/// unknown adapter is intentionally not treated as hardware: adapter names
/// vary by driver and an optimistic label would make results misleading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterPerformanceClass {
    Software,
    IntegratedGpu,
    DiscreteGpu,
    VirtualGpu,
    Cpu,
    Unknown,
}

impl AdapterPerformanceClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Software => "software",
            Self::IntegratedGpu => "integrated_gpu",
            Self::DiscreteGpu => "discrete_gpu",
            Self::VirtualGpu => "virtual_gpu",
            Self::Cpu => "cpu",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn is_software(self) -> bool {
        matches!(self, Self::Software | Self::Cpu)
    }

    #[must_use]
    pub const fn is_proven_hardware(self) -> bool {
        matches!(self, Self::DiscreteGpu | Self::IntegratedGpu)
    }
}

impl AdapterMetadata {
    #[must_use]
    pub fn performance_class(&self) -> AdapterPerformanceClass {
        if [
            self.adapter_name.as_str(),
            self.driver_name.as_str(),
            self.driver_info.as_str(),
        ]
        .iter()
        .any(|field| is_known_software_adapter(field))
        {
            return AdapterPerformanceClass::Software;
        }
        if self.graphics_backend.eq_ignore_ascii_case("gl")
            && self.adapter_name.starts_with("D3D12 (")
        {
            let name = self.adapter_name.to_ascii_lowercase();
            if name.contains("nvidia") || name.contains("radeon") || name.contains("amd") {
                return AdapterPerformanceClass::DiscreteGpu;
            }
            if name.contains("intel") {
                return AdapterPerformanceClass::IntegratedGpu;
            }
        }
        if self.device_type.eq_ignore_ascii_case("integratedgpu") {
            return AdapterPerformanceClass::IntegratedGpu;
        }
        if self.device_type.eq_ignore_ascii_case("discretegpu") {
            return AdapterPerformanceClass::DiscreteGpu;
        }
        if self.device_type.eq_ignore_ascii_case("virtualgpu") {
            return AdapterPerformanceClass::VirtualGpu;
        }
        if self.device_type.eq_ignore_ascii_case("cpu") {
            return AdapterPerformanceClass::Cpu;
        }
        AdapterPerformanceClass::Unknown
    }
}

fn is_known_software_adapter(field: &str) -> bool {
    let field = field.to_ascii_lowercase();
    [
        "lavapipe",
        "llvmpipe",
        "softpipe",
        "openswr",
        "swiftshader",
        "software rasterizer",
        "microsoft basic render driver",
        "warp",
    ]
    .iter()
    .any(|marker| field.contains(marker))
}

/// Backend-neutral rendering lifecycle. The engine owns scheduling and encoding;
/// backends consume already evaluated frames and shared decoded source bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollMode {
    NonBlocking,
    WaitForOne,
    Drain,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedFrame {
    pub frame_number: u64,
    pub rgba: Vec<u8>,
}

pub trait RenderBackend: Send {
    fn kind(&self) -> RenderBackendKind;
    fn capacity(&self) -> usize;
    fn in_flight(&self) -> usize;
    fn submit_frame(&mut self, frame_number: u64, frame: &EvaluatedFrame)
    -> Result<(), Diagnostic>;
    fn poll_completed(&mut self, mode: PollMode) -> Result<Option<CompletedFrame>, Diagnostic>;
    /// Returns the exact frame associated with an asynchronous backend failure,
    /// when the backend can identify it.
    fn failed_frame_number(&self) -> Option<u64> {
        None
    }
    /// Cancellation-aware backends may avoid an uninterruptible driver wait.
    /// The default preserves the synchronous CPU contract.
    fn poll_completed_cancellable(
        &mut self,
        mode: PollMode,
        _cancelled: &AtomicBool,
    ) -> Result<Option<CompletedFrame>, Diagnostic> {
        self.poll_completed(mode)
    }
    fn flush(&mut self) -> Result<Vec<CompletedFrame>, Diagnostic>;
    fn abort(&mut self);
    /// Confirms that a completed operation left no backend-owned work or
    /// readback resource outstanding. The runner calls this before an output
    /// can become visible to users.
    fn verify_idle(&self) -> Result<(), Diagnostic> {
        if self.in_flight() == 0 {
            Ok(())
        } else {
            Err(Diagnostic::error(
                "MVP-BACKEND-NOT-IDLE",
                crate::Category::Backend,
                "backend retained in-flight work after flush",
                "",
            ))
        }
    }
    fn stats(&mut self) -> PreparationStats;
    fn timings(&self) -> PreparationTimings;
    fn staged_metrics(&self) -> crate::metrics::StagedMetrics;
    /// Starts a new video-operation metric window without rebuilding prepared
    /// renderer resources. Callers must only invoke this while the backend is
    /// idle, after all prior completions have been consumed.
    fn reset_operation_metrics(&mut self) {}
    fn record_written(&mut self, frame_number: u64);
    fn record_ready_queue(&mut self, length: usize, out_of_order: bool);
    fn adapter(&self) -> Option<AdapterMetadata>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(device_type: &str, adapter_name: &str) -> AdapterMetadata {
        AdapterMetadata {
            adapter_name: adapter_name.to_owned(),
            device_type: device_type.to_owned(),
            graphics_backend: "vulkan".to_owned(),
            driver_name: String::new(),
            driver_info: String::new(),
            vendor_id: 0,
            device_id: 0,
        }
    }

    #[test]
    fn classifies_known_software_adapter_markers_without_adapter_discovery() {
        for (device_type, name) in [
            ("other", "lavapipe (Mesa 24.0.0)"),
            ("other", "llvmpipe (LLVM 18.1.0)"),
            ("other", "SwiftShader Device (Subzero)"),
            ("other", "Microsoft Basic Render Driver"),
        ] {
            assert_eq!(
                metadata(device_type, name).performance_class(),
                AdapterPerformanceClass::Software,
                "{name}"
            );
        }
    }

    #[test]
    fn classifies_cpu_adapters_as_software_without_adapter_discovery() {
        let class = metadata("cpu", "Unknown CPU Adapter").performance_class();
        assert_eq!(class, AdapterPerformanceClass::Cpu);
        assert!(class.is_software());
    }

    #[test]
    fn classifies_hardware_device_types_without_name_assumptions() {
        for (device_type, expected) in [
            ("discretegpu", AdapterPerformanceClass::DiscreteGpu),
            ("integratedgpu", AdapterPerformanceClass::IntegratedGpu),
            ("virtualgpu", AdapterPerformanceClass::VirtualGpu),
        ] {
            assert_eq!(
                metadata(device_type, "adapter name need not reveal its driver")
                    .performance_class(),
                expected
            );
        }
    }

    #[test]
    fn classifies_common_vendor_metadata_without_adapter_discovery() {
        for (name, device_type, expected) in [
            (
                "NVIDIA GeForce RTX 4080",
                "discretegpu",
                AdapterPerformanceClass::DiscreteGpu,
            ),
            (
                "AMD Radeon RX 7900 XT",
                "discretegpu",
                AdapterPerformanceClass::DiscreteGpu,
            ),
            (
                "Intel(R) Iris(R) Xe",
                "integratedgpu",
                AdapterPerformanceClass::IntegratedGpu,
            ),
        ] {
            assert_eq!(
                metadata(device_type, name).performance_class(),
                expected,
                "{name}"
            );
        }
    }

    #[test]
    fn leaves_unrecognized_virtual_or_other_adapters_unclassified() {
        assert_eq!(
            metadata("other", "opaque remote GPU").performance_class(),
            AdapterPerformanceClass::Unknown
        );
    }

    #[test]
    fn classifies_wsl_d3d12_nvidia_as_deliberate_hardware() {
        let metadata = AdapterMetadata {
            adapter_name: "D3D12 (NVIDIA GeForce GTX 1650 SUPER)".to_owned(),
            device_type: "other".to_owned(),
            graphics_backend: "gl".to_owned(),
            driver_name: String::new(),
            driver_info: "Mesa".to_owned(),
            vendor_id: 0,
            device_id: 0,
        };
        assert_eq!(
            metadata.performance_class(),
            AdapterPerformanceClass::DiscreteGpu
        );
    }
}
