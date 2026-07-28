//! Thread-safe capture of asynchronous WGPU failures.

use std::sync::{Arc, Mutex};

use crate::{Category, Diagnostic};

#[derive(Clone, Default)]
pub(super) struct RuntimeErrorState(Arc<Mutex<Option<Diagnostic>>>);

impl RuntimeErrorState {
    pub(super) fn install(device: &wgpu::Device) -> Self {
        let state = Self::default();
        let uncaptured = state.clone();
        device.on_uncaptured_error(Box::new(move |error| {
            uncaptured.record("WGPU-RUNTIME", format!("uncaptured WGPU error: {error}"));
        }));
        let lost = state.clone();
        device.set_device_lost_callback(move |reason, message| {
            lost.record(
                "WGPU-DEVICE-LOST",
                format!("WGPU device lost ({reason:?}): {message}"),
            );
        });
        state
    }

    pub(super) fn check(&self) -> Result<(), Diagnostic> {
        self.0
            .lock()
            .map_err(|_| {
                Diagnostic::error(
                    "WGPU-RUNTIME",
                    Category::Backend,
                    "WGPU runtime error state was poisoned",
                    "",
                )
            })?
            .clone()
            .map_or(Ok(()), Err)
    }

    fn record(&self, code: &str, message: String) {
        if let Ok(mut fatal) = self.0.lock()
            && fatal.is_none()
        {
            *fatal = Some(Diagnostic::error(code, Category::Backend, message, ""));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_fatal_error_is_preserved() {
        let state = RuntimeErrorState::default();
        state.record("FIRST", "first".into());
        state.record("SECOND", "second".into());
        assert_eq!(state.check().expect_err("fatal state").code, "FIRST");
    }

    #[test]
    fn empty_state_allows_submit_poll_and_flush_checks() {
        let state = RuntimeErrorState::default();
        state.check().expect("no runtime failure");
        state.check().expect("repeated check remains clear");
    }

    #[test]
    fn device_loss_root_cause_is_not_overwritten_by_later_callbacks() {
        let state = RuntimeErrorState::default();
        state.record("WGPU-DEVICE-LOST", "device reset".into());
        state.record("WGPU-RUNTIME", "later validation error".into());
        let error = state.check().expect_err("device loss is fatal");
        assert_eq!(error.code, "WGPU-DEVICE-LOST");
        assert_eq!(error.message, "device reset");
    }
}
