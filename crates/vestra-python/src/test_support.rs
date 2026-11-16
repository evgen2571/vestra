//! Opt-in synchronization and counters for Python integration tests.
use pyo3::{exceptions::PyRuntimeError, prelude::*, types::PyModule};
use std::sync::{Condvar, Mutex, OnceLock};

pub(crate) mod detached {
    use super::*;
    #[derive(Default)]
    struct DetachBarrier {
        entered: bool,
        released: bool,
    }

    fn detach_barrier() -> &'static (Mutex<DetachBarrier>, Condvar) {
        static BARRIER: OnceLock<(Mutex<DetachBarrier>, Condvar)> = OnceLock::new();
        BARRIER.get_or_init(|| (Mutex::new(DetachBarrier::default()), Condvar::new()))
    }

    /// Private integration-test hook. It exercises the same `Python::detach` path
    /// used by production operations without exposing a supported package API.
    fn synchronization_error(operation: &str) -> PyErr {
        PyRuntimeError::new_err(format!("GIL test synchronization {operation} was poisoned"))
    }

    #[pyfunction]
    fn _test_wait_while_detached(py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let (lock, wake) = detach_barrier();
            let mut state = lock.lock().map_err(|_| synchronization_error("lock"))?;
            state.released = false;
            state.entered = true;
            wake.notify_all();
            while !state.released {
                state = wake
                    .wait(state)
                    .map_err(|_| synchronization_error("condition wait"))?;
            }
            state.entered = false;
            state.released = false;
            Ok(())
        })
    }

    #[pyfunction]
    fn _test_wait_until_detached_entered(py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let (lock, wake) = detach_barrier();
            let state = lock.lock().map_err(|_| synchronization_error("lock"))?;
            let _state = wake
                .wait_while(state, |state| !state.entered)
                .map_err(|_| synchronization_error("condition wait"))?;
            Ok(())
        })
    }

    #[pyfunction]
    fn _test_release_detached_wait() -> PyResult<()> {
        let (lock, wake) = detach_barrier();
        let mut state = lock.lock().map_err(|_| synchronization_error("lock"))?;
        state.released = true;
        wake.notify_all();
        Ok(())
    }

    pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add_function(wrap_pyfunction!(_test_wait_while_detached, module)?)?;
        module.add_function(wrap_pyfunction!(_test_wait_until_detached_entered, module)?)?;
        module.add_function(wrap_pyfunction!(_test_release_detached_wait, module)?)?;
        Ok(())
    }
}

pub(crate) mod prepared {
    use super::*;
    use crate::prepared::SlotError;
    #[derive(Default)]
    struct PreparedTestBarrier {
        armed: bool,
        entered: bool,
        released: bool,
    }
    fn prepared_test_barrier() -> &'static (Mutex<PreparedTestBarrier>, Condvar) {
        static BARRIER: OnceLock<(Mutex<PreparedTestBarrier>, Condvar)> = OnceLock::new();
        BARRIER.get_or_init(|| (Mutex::new(PreparedTestBarrier::default()), Condvar::new()))
    }
    pub(crate) fn wait_for_prepared_test_barrier() -> Result<(), SlotError> {
        let (lock, wake) = prepared_test_barrier();
        let mut state = lock.lock().map_err(|_| SlotError::Poisoned)?;
        if !state.armed {
            return Ok(());
        }
        state.armed = false;
        state.entered = true;
        wake.notify_all();
        while !state.released {
            state = wake.wait(state).map_err(|_| SlotError::Poisoned)?;
        }
        state.entered = false;
        state.released = false;
        Ok(())
    }

    pub(crate) fn wait_for_test_barrier() -> bool {
        wait_for_prepared_test_barrier().is_ok()
    }
    #[pyfunction]
    fn _test_arm_prepared_operation() -> PyResult<()> {
        let (lock, _) = prepared_test_barrier();
        let mut state = lock
            .lock()
            .map_err(|_| PyRuntimeError::new_err("prepared test synchronization was poisoned"))?;
        state.armed = true;
        state.entered = false;
        state.released = false;
        Ok(())
    }
    #[pyfunction]
    fn _test_wait_until_prepared_operation_entered(py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let (lock, wake) = prepared_test_barrier();
            let state = lock.lock().map_err(|_| SlotError::Poisoned)?;
            let _state = wake
                .wait_while(state, |state| !state.entered)
                .map_err(|_| SlotError::Poisoned)?;
            Ok::<(), SlotError>(())
        })
        .map_err(|_| PyRuntimeError::new_err("prepared test synchronization was poisoned"))
    }
    #[pyfunction]
    fn _test_release_prepared_operation() -> PyResult<()> {
        let (lock, wake) = prepared_test_barrier();
        let mut state = lock
            .lock()
            .map_err(|_| PyRuntimeError::new_err("prepared test synchronization was poisoned"))?;
        state.released = true;
        wake.notify_all();
        Ok(())
    }

    pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add_function(wrap_pyfunction!(_test_arm_prepared_operation, module)?)?;
        module.add_function(wrap_pyfunction!(
            _test_wait_until_prepared_operation_entered,
            module
        )?)?;
        module.add_function(wrap_pyfunction!(_test_release_prepared_operation, module)?)?;
        Ok(())
    }
}

pub(crate) mod render {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NATIVE_RENDER_INVOCATIONS: AtomicUsize = AtomicUsize::new(0);
    static CALLBACK_PYTHON_ATTACHMENTS: AtomicUsize = AtomicUsize::new(0);
    pub(crate) fn record_render_invocation() {
        NATIVE_RENDER_INVOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
    pub(crate) fn record_callback_attachment() {
        CALLBACK_PYTHON_ATTACHMENTS.fetch_add(1, Ordering::Relaxed);
    }
    #[derive(Default)]
    struct VideoRenderTestBarrier {
        armed: bool,
        entered: bool,
        released: bool,
    }

    fn video_render_test_barrier() -> &'static (Mutex<VideoRenderTestBarrier>, Condvar) {
        static BARRIER: OnceLock<(Mutex<VideoRenderTestBarrier>, Condvar)> = OnceLock::new();
        BARRIER.get_or_init(|| {
            (
                Mutex::new(VideoRenderTestBarrier::default()),
                Condvar::new(),
            )
        })
    }

    pub(crate) fn wait_for_video_render_test_barrier() -> Result<(), crate::prepared::SlotError> {
        let (lock, wake) = video_render_test_barrier();
        let mut state = lock
            .lock()
            .map_err(|_| crate::prepared::SlotError::Poisoned)?;
        if !state.armed {
            return Ok(());
        }
        state.armed = false;
        state.entered = true;
        wake.notify_all();
        while !state.released {
            state = wake
                .wait(state)
                .map_err(|_| crate::prepared::SlotError::Poisoned)?;
        }
        state.entered = false;
        state.released = false;
        Ok(())
    }

    fn video_render_synchronization_error() -> PyErr {
        PyRuntimeError::new_err("video render test synchronization was poisoned")
    }

    #[pyfunction]
    fn _test_arm_video_render() -> PyResult<()> {
        let (lock, _) = video_render_test_barrier();
        let mut state = lock
            .lock()
            .map_err(|_| video_render_synchronization_error())?;
        state.armed = true;
        state.entered = false;
        state.released = false;
        Ok(())
    }

    #[pyfunction]
    fn _test_wait_until_video_render_entered(py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let (lock, wake) = video_render_test_barrier();
            let state = lock
                .lock()
                .map_err(|_| video_render_synchronization_error())?;
            let _state = wake
                .wait_while(state, |state| !state.entered)
                .map_err(|_| video_render_synchronization_error())?;
            Ok(())
        })
    }

    #[pyfunction]
    fn _test_release_video_render() -> PyResult<()> {
        let (lock, wake) = video_render_test_barrier();
        let mut state = lock
            .lock()
            .map_err(|_| video_render_synchronization_error())?;
        state.released = true;
        wake.notify_all();
        Ok(())
    }

    #[pyfunction]
    fn _test_reset_native_render_invocation_count() -> PyResult<()> {
        NATIVE_RENDER_INVOCATIONS.store(0, Ordering::Relaxed);
        Ok(())
    }

    #[pyfunction]
    fn _test_native_render_invocation_count() -> PyResult<usize> {
        Ok(NATIVE_RENDER_INVOCATIONS.load(Ordering::Relaxed))
    }

    #[pyfunction]
    fn _test_reset_callback_attach_count() -> PyResult<()> {
        CALLBACK_PYTHON_ATTACHMENTS.store(0, Ordering::Relaxed);
        Ok(())
    }

    #[pyfunction]
    fn _test_callback_attach_count() -> PyResult<usize> {
        Ok(CALLBACK_PYTHON_ATTACHMENTS.load(Ordering::Relaxed))
    }

    pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add_function(wrap_pyfunction!(_test_arm_video_render, module)?)?;
        module.add_function(wrap_pyfunction!(
            _test_wait_until_video_render_entered,
            module
        )?)?;
        module.add_function(wrap_pyfunction!(_test_release_video_render, module)?)?;
        module.add_function(wrap_pyfunction!(
            _test_reset_native_render_invocation_count,
            module
        )?)?;
        module.add_function(wrap_pyfunction!(
            _test_native_render_invocation_count,
            module
        )?)?;
        module.add_function(wrap_pyfunction!(_test_reset_callback_attach_count, module)?)?;
        module.add_function(wrap_pyfunction!(_test_callback_attach_count, module)?)?;
        Ok(())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    detached::register(module)?;
    prepared::register(module)?;
    render::register(module)
}
