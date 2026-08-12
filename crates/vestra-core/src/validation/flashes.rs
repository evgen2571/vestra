//! Flash overlay validation.

use std::collections::BTreeSet;

use crate::{Category, Diagnostic, project::parse_colour};

pub(super) fn validate(flashes: &[crate::project::Flash], errors: &mut Vec<Diagnostic>) {
    let mut ids = BTreeSet::new();
    for (index, flash) in flashes.iter().enumerate() {
        let path = format!("/visual/flashes/{index}");
        if flash.id.trim().is_empty() || !ids.insert(&flash.id) {
            errors.push(Diagnostic::error(
                "MVP-FLASH-ID",
                Category::Semantic,
                "flash ids must be non-empty and unique",
                format!("{path}/id"),
            ));
        }
        if !super::nonnegative(flash.start) || !super::positive(flash.duration) {
            errors.push(Diagnostic::error(
                "MVP-FLASH-TIME",
                Category::Semantic,
                "flash start and duration must be finite with positive duration",
                path.clone(),
            ));
        }
        if !super::unit(flash.opacity) || parse_colour(&flash.colour).is_none() {
            errors.push(Diagnostic::error(
                "MVP-FLASH-PROPERTIES",
                Category::Semantic,
                "flash opacity or colour is invalid",
                path.clone(),
            ));
        }
        if !super::nonnegative(flash.fade_in)
            || !super::nonnegative(flash.fade_out)
            || flash.fade_in + flash.fade_out > flash.duration
        {
            errors.push(Diagnostic::error(
                "MVP-FLASH-FADES",
                Category::Semantic,
                "flash fades must be finite, non-negative, and fit within duration",
                path,
            ));
        }
    }
}
