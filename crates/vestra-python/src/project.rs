use super::*;
#[pyclass(
    name = "Project",
    frozen,
    skip_from_py_object,
    module = "vestra._native"
)]
#[derive(Clone)]
pub(crate) struct PyProject {
    pub(crate) inner: NativeProject,
}

#[pymethods]
impl PyProject {
    #[classmethod]
    fn load(_cls: &Bound<'_, PyType>, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<Self> {
        let path = conversion::path_from_python(path)?;
        match py.detach(|| NativeProject::load(path)) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    #[classmethod]
    #[pyo3(signature = (text, *, base_directory = None))]
    fn from_json(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        text: String,
        base_directory: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let base_directory = base_directory
            .map(conversion::path_from_python)
            .transpose()?
            .unwrap_or_else(|| PathBuf::from("."));
        match py.detach(|| NativeProject::from_json(&text, base_directory)) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    #[classmethod]
    #[pyo3(signature = (data, *, base_directory = None))]
    fn from_dict(
        _cls: &Bound<'_, PyType>,
        py: Python<'_>,
        data: &Bound<'_, PyAny>,
        base_directory: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let value = conversion::mapping_value(data)?;
        let base_directory = base_directory
            .map(conversion::path_from_python)
            .transpose()?
            .unwrap_or_else(|| PathBuf::from("."));
        match py.detach(|| NativeProject::from_value(value, base_directory)) {
            Ok(inner) => Ok(Self { inner }),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        py.detach(|| self.inner.to_json())
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = py
            .detach(|| self.inner.to_value())
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let object = pythonize::pythonize(py, &value)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        Ok(object.unbind())
    }

    fn save(&self, py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<()> {
        let path = conversion::path_from_python(path)?;
        match py.detach(|| self.inner.save(path)) {
            Ok(()) => Ok(()),
            Err(error) => Err(project_error(py, error)?),
        }
    }

    #[getter]
    fn base_directory(&self) -> PathBuf {
        self.inner.base_directory().to_path_buf()
    }
    #[getter]
    fn source_path(&self) -> Option<PathBuf> {
        self.inner.source_path().map(ToOwned::to_owned)
    }
    fn __repr__(&self) -> String {
        format!(
            "Project(source_path={:?}, base_directory={:?})",
            self.inner.source_path(),
            self.inner.base_directory()
        )
    }
}
