//! Explicit clip-local text highlight timing.

use serde::{Deserialize, Serialize};

/// An alternate fill for the independently timed text ranges in a cue.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WordHighlight {
    pub colour: String,
    pub words: Vec<WordTiming>,
}

/// A half-open time interval (seconds) and half-open Unicode scalar range.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WordTiming {
    pub start: f64,
    pub end: f64,
    pub text_start: usize,
    pub text_end: usize,
}
