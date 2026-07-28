use serde::{Deserialize, Serialize};

use super::ActiveInterval;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Preset {
    SlowDrift {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
    },
    ZoomPunch {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
    },
    Impact {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
        seed: u64,
    },
    HeavyImpact {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
        seed: u64,
    },
    FocusReveal {
        #[serde(flatten)]
        timing: ActiveInterval,
        intensity: f64,
    },
}

impl Preset {
    #[must_use]
    pub fn timing(&self) -> ActiveInterval {
        match self {
            Self::SlowDrift { timing, .. }
            | Self::ZoomPunch { timing, .. }
            | Self::Impact { timing, .. }
            | Self::HeavyImpact { timing, .. }
            | Self::FocusReveal { timing, .. } => *timing,
        }
    }
}
