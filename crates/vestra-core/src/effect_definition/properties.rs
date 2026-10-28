//! Scalar property identities, authored bounds and runtime constraints.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarPropertyConstraint {
    /// Accept any finite scalar value.
    Finite,
    /// Constrain the final property value to an inclusive interval.
    ClosedRange { min: f64, max: f64 },
    /// Zero is a valid value, but negative values are not.
    NonNegative,
    /// Values at or below the floor are promoted to the floor.
    PositiveFloor { minimum: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ScalarAuthoredValidation {
    Finite,
    Range {
        min: Option<f64>,
        max: Option<f64>,
        min_exclusive: bool,
        max_exclusive: bool,
    },
}

impl ScalarAuthoredValidation {
    #[must_use]
    pub(crate) const fn accepts(self, value: &f64) -> bool {
        let value = *value;
        match self {
            Self::Finite => value.is_finite(),
            Self::Range {
                min,
                max,
                min_exclusive,
                max_exclusive,
            } => {
                value.is_finite()
                    && match min {
                        None => true,
                        Some(min) if min_exclusive => value > min,
                        Some(min) => value >= min,
                    }
                    && match max {
                        None => true,
                        Some(max) if max_exclusive => value < max,
                        Some(max) => value <= max,
                    }
            }
        }
    }

    #[must_use]
    pub(super) const fn bounds(self) -> (Option<f64>, Option<f64>, bool, bool) {
        match self {
            Self::Finite => (None, None, false, false),
            Self::Range {
                min,
                max,
                min_exclusive,
                max_exclusive,
            } => (min, max, min_exclusive, max_exclusive),
        }
    }
}

/// The smallest renderer-meaningful strictly-positive visual scalar.
pub const MIN_POSITIVE_PROPERTY_VALUE: f64 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScalarPropertyDefinition {
    pub runtime_constraint: ScalarPropertyConstraint,
    pub authored_validation: ScalarAuthoredValidation,
}

macro_rules! visual_effect_catalog {
    ($internal:ident => {
        definition: $internal_definition:tt,
        compiled: $internal_compiled:tt,
        evaluated: $internal_evaluated:tt
    }; $( $kind:ident => {
        module: $module:ident,
        definition: $definition:tt,
        authored: $authored:tt,
        compiled: $compiled:tt,
        evaluated: $evaluated:tt,
        scalars: [$($field:ident => $target:ident = $index:literal: $constraint:tt),* $(,)?],
        tracks: [$($track:ident => $plain_target:ident = $plain_index:literal: $validation:tt),* $(,)?]
    }),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum ScalarPropertyTarget { $($($target = $index,)*)+ RotationDegrees = 72 }
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
        #[serde(rename_all = "snake_case")]
        pub enum PlainTrackTarget { $($($plain_target = $plain_index,)*)+ }
        impl ScalarPropertyTarget {
            pub(crate) const fn definition(self) -> ScalarPropertyDefinition {
                use ScalarAuthoredValidation::{Finite, Range};
                use ScalarPropertyConstraint::{ClosedRange as RuntimeRange, Finite as RuntimeFinite,
                    NonNegative as RuntimeNonNegative, PositiveFloor};
                match self {
                    $($(Self::$target => $constraint,)*)+
                    Self::RotationDegrees => ScalarPropertyDefinition {
                        runtime_constraint: RuntimeFinite, authored_validation: Finite
                    }
                }
            }
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { $($(Self::$target => stringify!($field),)*)+ Self::RotationDegrees => "rotation_degrees" }
            }
        }
        impl PlainTrackTarget {
            pub(crate) const fn authored_validation(self) -> ScalarAuthoredValidation {
                use ScalarAuthoredValidation::Range;
                match self { $($(Self::$plain_target => $validation,)*)+ }
            }
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self { $($(Self::$plain_target => stringify!($track),)*)+ }
            }
        }
    };
}

include!("catalog_entries.rs");

impl ScalarPropertyTarget {
    #[must_use]
    pub(crate) const fn constraint(self) -> ScalarPropertyConstraint {
        self.definition().runtime_constraint
    }
    #[must_use]
    pub(crate) const fn authored_validation(self) -> super::ScalarAuthoredValidation {
        self.definition().authored_validation
    }
}
