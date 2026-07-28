use serde::{Deserialize, Serialize};

use super::optional_non_null;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub background: String,
    pub quality: Quality,
    pub audio: bool,
    pub duration_mode: DurationMode,
    #[serde(default, deserialize_with = "optional_non_null")]
    pub duration: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DurationMode {
    Automatic,
    Explicit,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Preview,
    Balanced,
    High,
}

impl Quality {
    #[must_use]
    pub const fn crf(self) -> u8 {
        match self {
            Self::Preview => 30,
            Self::Balanced => 23,
            Self::High => 18,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FrameRate {
    Decimal(f64),
    Rational(String),
}

impl FrameRate {
    pub fn rational(&self) -> Result<(u64, u64), String> {
        match self {
            Self::Decimal(value) => {
                if !value.is_finite() || *value <= 0.0 {
                    return Err("frame_rate must be positive and finite".to_owned());
                }
                let scaled = (*value * 1_000_000.0).round();
                if scaled > u64::MAX as f64 {
                    return Err("frame_rate is too large".to_owned());
                }
                reduce(scaled as u64, 1_000_000)
            }
            Self::Rational(value) => {
                let Some((numerator, denominator)) = value.split_once('/') else {
                    return Err("frame_rate rational must be N/D".to_owned());
                };
                let numerator = numerator
                    .parse::<u64>()
                    .map_err(|_| "frame_rate numerator must be an integer".to_owned())?;
                let denominator = denominator
                    .parse::<u64>()
                    .map_err(|_| "frame_rate denominator must be an integer".to_owned())?;
                if numerator == 0 || denominator == 0 {
                    return Err("frame_rate rational parts must be positive".to_owned());
                }
                reduce(numerator, denominator)
            }
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Decimal(value) => value.to_string(),
            Self::Rational(value) => value.clone(),
        }
    }
}

fn reduce(numerator: u64, denominator: u64) -> Result<(u64, u64), String> {
    let divisor = gcd(numerator, denominator);
    let reduced = (numerator / divisor, denominator / divisor);
    if reduced.0 > 240_000 || reduced.1 > 1_000_000 {
        Err("frame_rate is outside supported range".to_owned())
    } else {
        Ok(reduced)
    }
}

const fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}
