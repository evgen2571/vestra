"""Immutable Python bindings for the video-editor Rust SDK."""

from ._native import (
    BackendPreference,
    Category,
    Diagnostic,
    Editor,
    InspectAssets,
    InspectAudio,
    InspectionReport,
    InspectOutput,
    PreflightOptions,
    PreflightReport,
    Project,
    ProjectError,
    Severity,
    ValidationReport,
    VideoEditorError,
    native_version,
)

__version__ = native_version()

__all__ = [
    "__version__", "native_version", "BackendPreference", "Category", "Severity",
    "Diagnostic", "Editor", "Project", "PreflightOptions", "ValidationReport",
    "PreflightReport", "InspectionReport", "InspectOutput", "InspectAssets", "InspectAudio",
    "VideoEditorError", "ProjectError",
]
