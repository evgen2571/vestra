//! Human-readable command summaries and diagnostics.

use std::fmt::Write as _;

use vestra::{Diagnostic, InspectionReport, RenderResult, Severity, ValidateResult};

/// Formats one diagnostic as a heading line with its location and hint.
pub fn diagnostic(diagnostic: &Diagnostic) -> String {
    let label = match diagnostic.severity {
        Severity::Fatal => "error",
        Severity::Warning => "warning",
    };
    let mut text = format!("{label}[{}]: {}", diagnostic.code, diagnostic.message);
    if let Some(pointer) = diagnostic
        .pointer
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        let _ = write!(text, "\n  at {pointer}");
    }
    if let Some(id) = diagnostic.related_id.as_deref() {
        let _ = write!(text, "\n  related: {id}");
    }
    if let Some(hint) = diagnostic.hint.as_deref() {
        let _ = write!(text, "\n  hint: {hint}");
    }
    text
}

pub fn validate(result: &ValidateResult) -> String {
    with_warnings(
        format!("{} is valid", result.project.display()),
        &result.warnings,
    )
}

pub fn inspect(result: &InspectionReport) -> String {
    let output = &result.output;
    let mut text = result.project.display().to_string();
    if let Some(name) = &result.name {
        let _ = write!(text, " ({name})");
    }
    let _ = write!(
        text,
        "\n  output   {}\n           {}x{} at {} fps, {} ({} frames, {} duration){}",
        output.path.display(),
        output.width,
        output.height,
        fps(&output.frame_rate),
        seconds(output.duration),
        output.total_frames,
        output.duration_mode,
        if output.preview { ", preview" } else { "" },
    );
    let _ = write!(
        text,
        "\n  visual   {}, {}, {}",
        count(result.visual_clips, "clip"),
        count(result.transitions, "transition"),
        count(result.flashes, "flash"),
    );
    let _ = write!(
        text,
        "\n  assets   {}, {}",
        count(result.assets.images, "image"),
        count(result.assets.audio, "audio file"),
    );
    match &result.audio {
        Some(audio) => {
            let _ = write!(
                text,
                "\n  audio    {}, {}, ends at {}{}",
                count(audio.track_count, "track"),
                count(audio.clip_count, "clip"),
                seconds(audio.end),
                if audio
                    .tracks
                    .iter()
                    .flat_map(|track| &track.clips)
                    .any(|clip| clip.duration_source == vestra::AudioDurationSource::Reported)
                {
                    " (reported source duration)"
                } else {
                    ""
                },
            );
        }
        None => text.push_str("\n  audio    none"),
    }
    with_warnings(text, &result.warnings)
}

pub fn render(result: &RenderResult) -> String {
    let mut text = format!(
        "Rendered {}\n  {}x{} at {} fps, {} ({} frames){}{}",
        result.output.display(),
        result.width,
        result.height,
        fps(&result.frame_rate),
        seconds(result.duration),
        result.total_frames,
        if result.audio_present {
            ", with audio"
        } else {
            ""
        },
        if result.preview { ", preview" } else { "" },
    );
    let _ = write!(text, "\n  {} renderer", result.render_backend);
    if let Some(adapter) = &result.adapter {
        let _ = write!(text, " on {}", adapter.adapter_name);
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "elapsed milliseconds are displayed with two decimals"
    )]
    let elapsed = result.elapsed_ms as f64 / 1000.0;
    let _ = write!(text, ", finished in {elapsed:.2} s");
    with_warnings(text, &result.warnings)
}

fn with_warnings(mut text: String, warnings: &[Diagnostic]) -> String {
    for warning in warnings {
        text.push('\n');
        text.push_str(&diagnostic(warning));
    }
    text
}

/// Shows whole frame rates as `30` and keeps fractional rates such as `30000/1001`.
fn fps(frame_rate: &str) -> &str {
    frame_rate.strip_suffix("/1").unwrap_or(frame_rate)
}

fn seconds(value: f64) -> String {
    format!("{value:.2} s")
}

fn count(value: usize, noun: &str) -> String {
    match (value, noun) {
        (1, _) => format!("1 {noun}"),
        (_, "flash") => format!("{value} flashes"),
        _ => format!("{value} {noun}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vestra::Category;

    #[test]
    fn diagnostics_include_location_and_hint() {
        let mut value = Diagnostic::error(
            "VESTRA-TEST",
            Category::Semantic,
            "keyframe is outside the clip",
            "/visual/clips/0/opacity",
        );
        value.hint = Some("move the keyframe".to_owned());
        assert_eq!(
            diagnostic(&value),
            "error[VESTRA-TEST]: keyframe is outside the clip\n  \
             at /visual/clips/0/opacity\n  hint: move the keyframe"
        );
    }

    #[test]
    fn counts_use_plural_nouns() {
        assert_eq!(count(1, "clip"), "1 clip");
        assert_eq!(count(0, "flash"), "0 flashes");
        assert_eq!(count(3, "audio file"), "3 audio files");
    }
}
