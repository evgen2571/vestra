use serde::{Deserialize, Serialize};

/// Coverage extracted from an existing layer presentation.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatteMode {
    Alpha,
    Luma,
}

/// A reference to another layer in the same immediate composition.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TrackMatte {
    pub source_layer: String,
    pub mode: MatteMode,
    #[serde(default)]
    pub invert: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_matte_round_trips_with_alpha_luma_and_invert() {
        for mode in [MatteMode::Alpha, MatteMode::Luma] {
            let value = TrackMatte {
                source_layer: "matte".to_owned(),
                mode,
                invert: true,
            };
            let json = serde_json::to_value(&value).expect("matte serializes");
            assert_eq!(json["source_layer"], "matte");
            assert_eq!(
                serde_json::from_value::<TrackMatte>(json.clone()).unwrap(),
                value
            );
        }
    }
}
