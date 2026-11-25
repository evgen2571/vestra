//! Compile text animation to the shared clip-local nanosecond time domain.

#![allow(
    clippy::result_large_err,
    reason = "compiler diagnostics remain structured and machine-readable"
)]

use crate::{Category, Diagnostic, animation::Track, project::TextSource};

use super::{time, tracks};

#[derive(Clone, Debug)]
pub struct CompiledTextAnimation {
    pub(crate) reveal: Option<Track<f64>>,
    pub(crate) words: Vec<CompiledWordTiming>,
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledWordTiming {
    start: u128,
    end: u128,
    byte_range: (usize, usize),
}

/// One frame's text paint state. The range indexes UTF-8 bytes in the full cue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextAnimationFrame {
    pub reveal: f64,
    pub active_range: Option<(usize, usize)>,
}

impl Default for TextAnimationFrame {
    fn default() -> Self {
        Self {
            reveal: 1.0,
            active_range: None,
        }
    }
}

impl CompiledTextAnimation {
    pub(crate) fn compile(text: &TextSource, id: &str) -> Result<Option<Box<Self>>, Diagnostic> {
        let reveal = text
            .reveal
            .as_ref()
            .map(|track| tracks::compile(track, id))
            .transpose()?;
        let mut words = Vec::new();
        if let Some(highlight) = &text.word_highlight {
            let mut byte_offsets = text
                .text
                .char_indices()
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            byte_offsets.push(text.text.len());
            for word in &highlight.words {
                let start = byte_offsets.get(word.text_start);
                let end = byte_offsets.get(word.text_end);
                let (Some(&start), Some(&end)) = (start, end) else {
                    return Err(Diagnostic::error(
                        "VESTRA-PLAN-TEXT-HIGHLIGHT-RANGE",
                        Category::Internal,
                        format!("validated text clip '{id}' has a word range outside its text"),
                        "",
                    ));
                };
                words.push(CompiledWordTiming {
                    start: time::to_nanos(word.start, id)?,
                    end: time::to_nanos(word.end, id)?,
                    byte_range: (start, end),
                });
            }
        }
        Ok((reveal.is_some() || !words.is_empty()).then(|| Box::new(Self { reveal, words })))
    }

    pub(crate) fn evaluate(&self, relative: u128) -> TextAnimationFrame {
        let next = self.words.partition_point(|word| word.end <= relative);
        let active_range = self
            .words
            .get(next)
            .filter(|word| word.start <= relative)
            .map(|word| word.byte_range);
        TextAnimationFrame {
            reveal: self
                .reveal
                .as_ref()
                .map_or(1.0, |track| track.evaluate(relative).clamp(0.0, 1.0)),
            active_range,
        }
    }
}
