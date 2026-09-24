use std::path::PathBuf;

use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyType};
use vestra::{
    BackendPreference as NativeBackendPreference, Editor as NativeEditor,
    PreflightOptions as NativePreflightOptions,
};

use crate::{
    PyInspectionReport, PyPreflightReport, PyProject, PyValidationReport, conversion, editor_error,
    prepared, render,
};
#[pyclass(
    name = "Editor",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyEditor {
    inner: NativeEditor,
}

#[pymethods]
impl PyEditor {
    #[new]
    fn new() -> Self {
        Self {
            inner: NativeEditor::new(),
        }
    }
    fn validate(&self, py: Python<'_>, project: &PyProject) -> PyValidationReport {
        PyValidationReport::from(py.detach(|| self.inner.validate(&project.inner)))
    }
    fn preflight(
        &self,
        py: Python<'_>,
        project: &PyProject,
        options: &PyPreflightOptions,
    ) -> PyPreflightReport {
        PyPreflightReport::from(
            py.detach(|| self.inner.preflight(&project.inner, options.inner.clone())),
        )
    }
    #[pyo3(signature = (project, *, preview = false))]
    #[expect(
        clippy::result_large_err,
        reason = "the Python bridge preserves native editor diagnostics"
    )]
    fn inspect(
        &self,
        py: Python<'_>,
        project: &PyProject,
        preview: bool,
    ) -> PyResult<PyInspectionReport> {
        match py.detach(|| self.inner.inspect(&project.inner, preview)) {
            Ok(report) => Ok(PyInspectionReport::from(report)),
            Err(error) => Err(editor_error(py, error)?),
        }
    }

    #[pyo3(signature = (project, options = None))]
    #[expect(
        clippy::result_large_err,
        reason = "the Python bridge preserves native preparation diagnostics"
    )]
    fn prepare(
        &self,
        py: Python<'_>,
        project: &PyProject,
        options: Option<&prepared::PyPrepareOptions>,
    ) -> PyResult<prepared::PyPreparedProject> {
        let options = options.map_or_else(vestra::PrepareOptions::default, |value| value.inner);
        match py.detach(|| {
            if !prepared::wait_for_test_barrier() {
                return Err(None);
            }
            self.inner.prepare(&project.inner, options).map_err(Some)
        }) {
            Ok(value) => Ok(prepared::PyPreparedProject::new(value)),
            Err(Some(error)) => prepared::preparation_error(py, error),
            Err(None) => Err(PyRuntimeError::new_err(
                "prepared test synchronization failed",
            )),
        }
    }

    #[pyo3(signature = (project, request, *, show_progress = true, on_progress = None, cancellation = None))]
    fn render(
        &self,
        py: Python<'_>,
        project: &PyProject,
        request: &render::PyRenderRequest,
        show_progress: bool,
        on_progress: Option<Py<PyAny>>,
        cancellation: Option<&render::PyCancellationToken>,
    ) -> PyResult<render::PyRenderResult> {
        render::render_one_shot(
            py,
            &self.inner,
            &project.inner,
            request,
            show_progress,
            on_progress,
            cancellation,
        )
    }
}

#[pyclass(
    name = "PreflightOptions",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyPreflightOptions {
    inner: NativePreflightOptions,
    kind: String,
    backend: Option<PyBackendPreference>,
    output: Option<PathBuf>,
    overwrite: bool,
}

#[pymethods]
impl PyPreflightOptions {
    #[classmethod]
    fn for_validation(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: NativePreflightOptions::for_validation(),
            kind: "validation".to_owned(),
            backend: None,
            output: None,
            overwrite: false,
        }
    }
    #[classmethod]
    fn for_inspection(_cls: &Bound<'_, PyType>) -> Self {
        Self {
            inner: NativePreflightOptions::for_inspection(),
            kind: "inspection".to_owned(),
            backend: None,
            output: None,
            overwrite: false,
        }
    }
    #[classmethod]
    fn for_preparation(_cls: &Bound<'_, PyType>, backend: &PyBackendPreference) -> Self {
        Self {
            inner: NativePreflightOptions::for_preparation(backend.native()),
            kind: "preparation".to_owned(),
            backend: Some(*backend),
            output: None,
            overwrite: false,
        }
    }
    #[classmethod]
    #[pyo3(signature = (backend, output, *, overwrite = false))]
    fn for_render(
        _cls: &Bound<'_, PyType>,
        backend: &PyBackendPreference,
        output: &Bound<'_, PyAny>,
        overwrite: bool,
    ) -> PyResult<Self> {
        let output = conversion::path_from_python(output)?;
        Ok(Self {
            inner: NativePreflightOptions::for_render(
                backend.native(),
                Some(output.clone()),
                overwrite,
            ),
            kind: "render".to_owned(),
            backend: Some(*backend),
            output: Some(output),
            overwrite,
        })
    }
    #[getter]
    fn kind(&self) -> String {
        self.kind.clone()
    }
    #[getter]
    fn backend(&self) -> Option<PyBackendPreference> {
        self.backend
    }
    #[getter]
    fn output(&self) -> Option<PathBuf> {
        self.output.clone()
    }
    #[getter]
    fn overwrite(&self) -> bool {
        self.overwrite
    }
}

#[pyclass(
    name = "BackendPreference",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) enum PyBackendPreference {
    #[pyo3(name = "AUTO")]
    Auto,
    #[pyo3(name = "CPU")]
    Cpu,
    #[pyo3(name = "WGPU")]
    Wgpu,
}
impl PyBackendPreference {
    pub(crate) const fn native(self) -> NativeBackendPreference {
        match self {
            Self::Auto => NativeBackendPreference::Auto,
            Self::Cpu => NativeBackendPreference::Cpu,
            Self::Wgpu => NativeBackendPreference::Wgpu,
        }
    }
    const fn as_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Wgpu => "wgpu",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PyBackendPreference;
    use vestra::BackendPreference;

    #[test]
    fn backend_preference_values_match_the_python_contract() {
        assert_eq!(PyBackendPreference::Auto.native(), BackendPreference::Auto);
        assert_eq!(PyBackendPreference::Wgpu.as_value(), "wgpu");
    }
}
#[pymethods]
impl PyBackendPreference {
    #[getter]
    fn value(&self) -> &'static str {
        (*self).as_value()
    }
    fn __str__(&self) -> &'static str {
        (*self).as_value()
    }
}
