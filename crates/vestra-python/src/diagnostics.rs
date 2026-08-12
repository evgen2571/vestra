use pyo3::{prelude::*, types::PyTuple};
use vestra::{
    Category as NativeCategory, Diagnostic as NativeDiagnostic, PreflightReport as NativePreflight,
    Severity as NativeSeverity, ValidationReport as NativeValidation,
};

#[pyclass(
    name = "Category",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) enum PyCategory {
    #[pyo3(name = "USAGE")]
    Usage,
    #[pyo3(name = "PROJECT")]
    Project,
    #[pyo3(name = "SEMANTIC")]
    Semantic,
    #[pyo3(name = "ASSET")]
    Asset,
    #[pyo3(name = "MEDIA")]
    Media,
    #[pyo3(name = "BACKEND")]
    Backend,
    #[pyo3(name = "RENDER")]
    Render,
    #[pyo3(name = "OUTPUT")]
    Output,
    #[pyo3(name = "CANCELLATION")]
    Cancellation,
    #[pyo3(name = "INTERNAL")]
    Internal,
}
impl From<NativeCategory> for PyCategory {
    fn from(value: NativeCategory) -> Self {
        match value {
            NativeCategory::Usage => Self::Usage,
            NativeCategory::Project => Self::Project,
            NativeCategory::Semantic => Self::Semantic,
            NativeCategory::Asset => Self::Asset,
            NativeCategory::Media => Self::Media,
            NativeCategory::Backend => Self::Backend,
            NativeCategory::Render => Self::Render,
            NativeCategory::Output => Self::Output,
            NativeCategory::Cancellation => Self::Cancellation,
            NativeCategory::Internal => Self::Internal,
        }
    }
}
#[pymethods]
impl PyCategory {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::Usage => "usage",
            Self::Project => "project",
            Self::Semantic => "semantic",
            Self::Asset => "asset",
            Self::Media => "media",
            Self::Backend => "backend",
            Self::Render => "render",
            Self::Output => "output",
            Self::Cancellation => "cancellation",
            Self::Internal => "internal",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[pyclass(
    name = "Severity",
    frozen,
    eq,
    hash,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) enum PySeverity {
    #[pyo3(name = "FATAL")]
    Fatal,
    #[pyo3(name = "WARNING")]
    Warning,
}
impl From<NativeSeverity> for PySeverity {
    fn from(value: NativeSeverity) -> Self {
        match value {
            NativeSeverity::Fatal => Self::Fatal,
            NativeSeverity::Warning => Self::Warning,
        }
    }
}
#[pymethods]
impl PySeverity {
    #[getter]
    fn value(&self) -> &'static str {
        match self {
            Self::Fatal => "fatal",
            Self::Warning => "warning",
        }
    }
    fn __str__(&self) -> &'static str {
        self.value()
    }
}

#[cfg(test)]
mod tests {
    use super::{PyCategory, PySeverity};
    use vestra::{Category, Severity};

    #[test]
    fn diagnostic_enum_values_match_the_python_contract() {
        assert_eq!(
            PyCategory::from(Category::Cancellation).value(),
            "cancellation"
        );
        assert_eq!(PySeverity::from(Severity::Warning).value(), "warning");
    }
}

#[pyclass(
    name = "Diagnostic",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyDiagnostic {
    #[pyo3(get)]
    pub(crate) code: String,
    #[pyo3(get)]
    pub(crate) category: PyCategory,
    #[pyo3(get)]
    pub(crate) severity: PySeverity,
    #[pyo3(get)]
    pub(crate) message: String,
    #[pyo3(get)]
    pub(crate) pointer: Option<String>,
    #[pyo3(get)]
    pub(crate) related_id: Option<String>,
    #[pyo3(get)]
    pub(crate) hint: Option<String>,
}
impl From<NativeDiagnostic> for PyDiagnostic {
    fn from(value: NativeDiagnostic) -> Self {
        Self {
            code: value.code,
            category: value.category.into(),
            severity: value.severity.into(),
            message: value.message,
            pointer: value.pointer,
            related_id: value.related_id,
            hint: value.hint,
        }
    }
}
#[pymethods]
impl PyDiagnostic {
    fn __str__(&self) -> String {
        format!("{}: {}", self.code, self.message)
    }
    fn __repr__(&self) -> String {
        format!(
            "Diagnostic(code={:?}, severity={})",
            self.code,
            self.severity.value()
        )
    }
}

pub(crate) fn diagnostics(values: &[NativeDiagnostic]) -> Vec<PyDiagnostic> {
    values.iter().cloned().map(PyDiagnostic::from).collect()
}

pub(crate) fn diagnostic_tuple(py: Python<'_>, values: &[PyDiagnostic]) -> PyResult<Py<PyAny>> {
    let values = values
        .iter()
        .cloned()
        .map(|value| Py::new(py, value))
        .collect::<PyResult<Vec<_>>>()?;
    Ok(PyTuple::new(py, values)?.into_any().unbind())
}

#[pyclass(
    name = "ValidationReport",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
pub(crate) struct PyValidationReport {
    #[pyo3(get)]
    is_valid: bool,
    diagnostics: Vec<PyDiagnostic>,
    errors: Vec<PyDiagnostic>,
    warnings: Vec<PyDiagnostic>,
}
#[pymethods]
impl PyValidationReport {
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.diagnostics)
    }
    #[getter]
    fn errors(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.errors)
    }
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}
impl From<NativeValidation> for PyValidationReport {
    fn from(value: NativeValidation) -> Self {
        Self {
            is_valid: value.is_valid(),
            diagnostics: diagnostics(value.diagnostics()),
            errors: value.errors().cloned().map(PyDiagnostic::from).collect(),
            warnings: value.warnings().cloned().map(PyDiagnostic::from).collect(),
        }
    }
}

#[pyclass(
    name = "PreflightReport",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
pub(crate) struct PyPreflightReport {
    #[pyo3(get)]
    is_ready: bool,
    #[pyo3(get)]
    is_valid: bool,
    diagnostics: Vec<PyDiagnostic>,
    errors: Vec<PyDiagnostic>,
    warnings: Vec<PyDiagnostic>,
}
#[pymethods]
impl PyPreflightReport {
    #[getter]
    fn diagnostics(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.diagnostics)
    }
    #[getter]
    fn errors(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.errors)
    }
    #[getter]
    fn warnings(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        diagnostic_tuple(py, &self.warnings)
    }
}
impl From<NativePreflight> for PyPreflightReport {
    fn from(value: NativePreflight) -> Self {
        Self {
            is_ready: value.is_ready(),
            is_valid: value.is_valid(),
            diagnostics: diagnostics(value.diagnostics()),
            errors: value.errors().cloned().map(PyDiagnostic::from).collect(),
            warnings: value.warnings().cloned().map(PyDiagnostic::from).collect(),
        }
    }
}
