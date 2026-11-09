use std::{collections::BTreeMap, io::Read};

use crate::{MediaError, audio_graph::AudioSourceRequirement};

#[derive(Default)]
pub(crate) struct AudioReadReport {
    pub(crate) stderr: String,
    samples: BTreeMap<usize, Option<u64>>,
    output_samples: BTreeMap<usize, Option<u64>>,
}

impl AudioReadReport {
    pub(crate) fn source_samples(&self, source_index: usize) -> Option<u64> {
        self.samples.get(&source_index).copied().flatten()
    }

    pub(crate) fn verify(&self, requirements: &[AudioSourceRequirement]) -> Result<(), MediaError> {
        for requirement in requirements {
            let available = self
                .samples
                .get(&requirement.source_index)
                .copied()
                .flatten()
                .ok_or_else(|| MediaError::AudioVerification {
                    clip_id: requirement.clip_id.clone(),
                })?;
            if requirement.trim_start >= available
                || requirement.required_end > available.saturating_add(1)
            {
                return Err(MediaError::AudioSourceBounds {
                    clip_id: requirement.clip_id.clone(),
                    required: requirement.required_end,
                    available,
                });
            }
            if requirement.selected_end > available.saturating_add(1)
                && let Some(output) = &requirement.transformed_output
            {
                let produced = self
                    .output_samples
                    .get(&output.clip_index)
                    .copied()
                    .flatten()
                    .ok_or_else(|| MediaError::AudioVerification {
                        clip_id: requirement.clip_id.clone(),
                    })?;
                // At EOF atempo can emit less than its ideal duration transform.
                // Only complete authored selections may use duration-normalizing padding.
                if output.required_samples > produced.saturating_add(1) {
                    return Err(MediaError::AudioSourceBounds {
                        clip_id: requirement.clip_id.clone(),
                        required: requirement.selected_end,
                        available,
                    });
                }
            }
        }
        Ok(())
    }

    fn record_line(&mut self, line: &[u8], source_count: usize) {
        let Ok(line) = std::str::from_utf8(line) else {
            return;
        };
        let (line, samples, index_limit) =
            if let Some(line) = line.strip_prefix("[astats@vestra_source_") {
                (line, &mut self.samples, Some(source_count))
            } else if let Some(line) = line.strip_prefix("[astats@vestra_clip_") {
                (line, &mut self.output_samples, None)
            } else {
                return;
            };
        let Some((source, line)) = line.split_once(" @ ") else {
            return;
        };
        let Ok(source) = source.parse::<usize>() else {
            return;
        };
        let Some((_, count)) = line.split_once("] Number of samples: ") else {
            return;
        };
        if index_limit.is_none_or(|limit| source < limit) {
            let count = count.trim().parse::<u64>().ok();
            // Reinitialized source filters reset their counts; do not accept a partial total.
            samples
                .entry(source)
                .and_modify(|count| *count = None)
                .or_insert(count);
        }
    }
}

/// Keep counters independently of the diagnostic tail, including large graphs.
pub(crate) fn collect_audio_stderr(mut stderr: impl Read, source_count: usize) -> AudioReadReport {
    const LIMIT: usize = 64 * 1024;
    const LINE_LIMIT: usize = 4096;
    let mut report = AudioReadReport::default();
    let mut tail = Vec::new();
    let mut line = Vec::new();
    let mut oversized = false;
    let mut chunk = [0_u8; 8192];
    loop {
        let read = match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        tail.extend_from_slice(&chunk[..read]);
        if tail.len() > LIMIT {
            tail.drain(..tail.len() - LIMIT);
        }
        for &byte in &chunk[..read] {
            if byte == b'\n' || byte == b'\r' {
                if !oversized {
                    report.record_line(&line, source_count);
                }
                line.clear();
                oversized = false;
            } else if line.len() < LINE_LIMIT {
                line.push(byte);
            } else {
                oversized = true;
            }
        }
    }
    if !oversized {
        report.record_line(&line, source_count);
    }
    report.stderr = String::from_utf8_lossy(&tail).trim().to_owned();
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requirement(start: u64, end: u64) -> AudioSourceRequirement {
        AudioSourceRequirement {
            source_index: 0,
            clip_id: "clip".into(),
            trim_start: start,
            required_end: end,
            selected_end: end,
            transformed_output: None,
        }
    }

    #[test]
    fn counters_survive_truncation_and_allow_only_end_rounding() {
        let mut bytes = b"[astats@vestra_source_0 @ 0x123] Number of samples: 18000\r\n".to_vec();
        bytes.extend(vec![b'x'; 70 * 1024]);
        let report = collect_audio_stderr(&bytes[..], 1);
        assert!(report.stderr.len() <= 64 * 1024);
        report
            .verify(&[requirement(0, 18001)])
            .expect("end rounding");
        assert!(matches!(
            report.verify(&[requirement(0, 18002)]),
            Err(MediaError::AudioSourceBounds { .. })
        ));
        assert!(matches!(
            report.verify(&[requirement(18000, 18001)]),
            Err(MediaError::AudioSourceBounds { .. })
        ));
    }

    #[test]
    fn absent_malformed_and_unexpected_counters_fail_closed() {
        for count in ["", "NaN", "-1", "18446744073709551616"] {
            let bytes = format!("[astats@vestra_source_0 @ 0x123] Number of samples: {count}\n");
            assert!(matches!(
                collect_audio_stderr(bytes.as_bytes(), 1).verify(&[requirement(0, 1)]),
                Err(MediaError::AudioVerification { .. })
            ));
        }
        let bytes = b"[astats@vestra_source_999 @ 0x123] Number of samples: 1000\n";
        assert!(collect_audio_stderr(&bytes[..], 1).samples.is_empty());
        let bytes = b"[astats@vestra_source_0 @ 0x123] Number of samples: 1000\n[astats@vestra_source_0 @ 0x456] Number of samples: 2000\n";
        assert!(matches!(
            collect_audio_stderr(&bytes[..], 1).verify(&[requirement(0, 1)]),
            Err(MediaError::AudioVerification { .. })
        ));
    }

    #[test]
    fn partial_speed_sources_require_real_output_but_complete_selections_allow_padding() {
        let mut requirement = requirement(0, 100);
        requirement.selected_end = 1_000;
        requirement.transformed_output = Some(crate::audio_graph::AudioOutputRequirement {
            clip_index: 2,
            required_samples: 200,
        });
        for (count, valid) in [("199", true), ("198", false), ("NaN", false)] {
            let bytes = format!(
                "[astats@vestra_clip_2 @ 0x1] Number of samples: {count}\n[astats@vestra_source_0 @ 0x2] Number of samples: 150\n"
            );
            assert_eq!(
                collect_audio_stderr(bytes.as_bytes(), 1)
                    .verify(std::slice::from_ref(&requirement))
                    .is_ok(),
                valid
            );
        }
        for (source_count, valid) in [(150, false), (999, true), (1_000, true)] {
            let bytes =
                format!("[astats@vestra_source_0 @ 0x2] Number of samples: {source_count}\n");
            assert_eq!(
                collect_audio_stderr(bytes.as_bytes(), 1)
                    .verify(std::slice::from_ref(&requirement))
                    .is_ok(),
                valid
            );
        }
        let bytes = b"[astats@vestra_source_0 @ 0x2] Number of samples: 150\n[astats@vestra_clip_2 @ 0x1] Number of samples: 200\n[astats@vestra_clip_2 @ 0x3] Number of samples: 200\n";
        assert!(matches!(
            collect_audio_stderr(&bytes[..], 1).verify(&[requirement]),
            Err(MediaError::AudioVerification { .. })
        ));
    }
}
