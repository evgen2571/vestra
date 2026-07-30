"""Immutable Python bindings for the video-editor Rust SDK."""

from ._native import (
    BackendPreference,
    BackendKind,
    BackendFallback,
    Category,
    Diagnostic,
    Editor,
    Frame,
    FrameRate,
    FrameRenderError,
    GraphicsBackend,
    InspectAssets,
    InspectAudio,
    InspectionReport,
    InspectOutput,
    PreflightOptions,
    PreflightReport,
    Project,
    ProjectError,
    PreparationError,
    PreparationReport,
    PreparationTimings,
    PrepareOptions,
    PreparedProject,
    PreparedProjectBusyError,
    AdapterDeviceType,
    AdapterInfo,
    PixelFormat,
    Severity,
    ValidationReport,
    VideoEditorError,
    native_version,
)

__version__ = native_version()

__all__ = [
    "__version__", "native_version", "BackendPreference", "BackendKind", "BackendFallback",
    "AdapterDeviceType", "AdapterInfo", "GraphicsBackend", "PixelFormat", "Category", "Severity",
    "Diagnostic", "Editor", "Project", "PreflightOptions", "ValidationReport",
    "PreflightReport", "InspectionReport", "InspectOutput", "InspectAssets", "InspectAudio",
    "PrepareOptions", "PreparedProject", "PreparationReport", "PreparationTimings", "FrameRate", "Frame",
    "VideoEditorError", "ProjectError", "PreparationError", "FrameRenderError", "PreparedProjectBusyError",
]
