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
}
