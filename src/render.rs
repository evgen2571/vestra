use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use image::{Rgba, RgbaImage, imageops::FilterType};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    diagnostic::{Category, Diagnostic},
    project::{
        Animation, AnimationTarget, Clip, Crop, Flash, Point, Sizing, Transition, ValidatedProject,
        parse_colour,
    },
    timeline::{
        active, animation_progress, eased, frame_time_nanos, interpolate_crop, interpolate_point,
    },
};

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub output_override: Option<PathBuf>,
    pub overwrite: bool,
    pub preview: bool,
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderSummary {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub frame_count: u64,
    pub audio_present: bool,
    pub preview: bool,
    pub elapsed_ms: u128,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderEvent {
    pub event_schema_version: u8,
    #[serde(rename = "type")]
    pub kind: String,
    pub frame: u64,
    pub total_frames: u64,
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct RenderError {
    pub diagnostic: Diagnostic,
    pub temporary_removed: bool,
}

#[allow(
    clippy::result_large_err,
    reason = "render errors carry the required machine-readable diagnostic and cleanup result"
)]
pub fn render(
    validated: &ValidatedProject,
    options: &RenderOptions,
    emit: &mut dyn FnMut(RenderEvent),
) -> Result<RenderSummary, RenderError> {
    let started = Instant::now();
    let output_path = resolve_output_path(validated, options);
    if output_path.exists() && !options.overwrite {
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-OUTPUT-EXISTS",
                Category::Output,
                format!(
                    "output '{}' already exists; pass --overwrite to replace it",
                    output_path.display()
                ),
                "/output/path",
            ),
            temporary_removed: false,
        });
    }
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-OUTPUT-PARENT",
                Category::Output,
                format!("output directory '{}' does not exist", parent.display()),
                "/output/path",
            ),
            temporary_removed: false,
        });
    }
    let (width, height) = effective_dimensions(
        validated.project.output.width,
        validated.project.output.height,
        options.preview,
    );
    let temporary = temporary_output_path(&output_path);
    let images = load_images(validated)?;
    emit(RenderEvent {
        event_schema_version: 1,
        kind: "started".to_owned(),
        frame: 0,
        total_frames: validated.frame_count,
        progress: 0.0,
        output_path: Some(output_path.clone()),
    });
    let mut child = start_ffmpeg(validated, &temporary, width, height)?;
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-RENDER-PIPE",
                Category::Render,
                "FFmpeg did not expose a frame input pipe",
                "",
            ),
            temporary_removed: remove_temporary(&temporary),
        });
    };
    for frame in 0..validated.frame_count {
        if options.cancelled.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RenderError {
                diagnostic: Diagnostic::error(
                    "MVP-CANCELLED",
                    Category::Cancellation,
                    "render cancelled",
                    "",
                ),
                temporary_removed: remove_temporary(&temporary),
            });
        }
        let time_nanos = frame_time_nanos(frame, validated.frame_rate.0, validated.frame_rate.1);
        let image = compose_frame(validated, &images, width, height, time_nanos);
        if let Err(error) = stdin.write_all(image.as_raw()) {
            drop(stdin);
            let _ = child.kill();
            let _ = child.wait();
            return Err(RenderError {
                diagnostic: Diagnostic::error(
                    "MVP-RENDER-WRITE",
                    Category::Render,
                    format!("cannot stream frame {frame}: {error}"),
                    "",
                ),
                temporary_removed: remove_temporary(&temporary),
            });
        }
        let completed = frame + 1;
        emit(RenderEvent {
            event_schema_version: 1,
            kind: "progress".to_owned(),
            frame: completed,
            total_frames: validated.frame_count,
            progress: (completed as f64 / validated.frame_count as f64 * 0.99).min(0.99),
            output_path: None,
        });
    }
    drop(stdin);
    let output = child.wait_with_output().map_err(|error| RenderError {
        diagnostic: Diagnostic::error(
            "MVP-RENDER-WAIT",
            Category::Render,
            format!("cannot wait for FFmpeg: {error}"),
            "",
        ),
        temporary_removed: remove_temporary(&temporary),
    })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-ENCODE",
                Category::Render,
                format!("FFmpeg failed with status {}: {detail}", output.status),
                "",
            ),
            temporary_removed: remove_temporary(&temporary),
        });
    }
    if let Err(error) = fs::rename(&temporary, &output_path) {
        return Err(RenderError {
            diagnostic: Diagnostic::error(
                "MVP-OUTPUT-PUBLISH",
                Category::Output,
                format!("cannot publish output: {error}"),
                "/output/path",
            ),
            temporary_removed: remove_temporary(&temporary),
        });
    }
    emit(RenderEvent {
        event_schema_version: 1,
        kind: "completed".to_owned(),
        frame: validated.frame_count,
        total_frames: validated.frame_count,
        progress: 1.0,
        output_path: Some(output_path.clone()),
    });
    Ok(RenderSummary {
        output_path,
        width,
        height,
        duration: validated.duration,
        frame_count: validated.frame_count,
        audio_present: include_audio(validated),
        preview: options.preview,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn resolve_output_path(validated: &ValidatedProject, options: &RenderOptions) -> PathBuf {
    options.output_override.clone().unwrap_or_else(|| {
        let configured = PathBuf::from(&validated.project.output.path);
        if configured.is_absolute() {
            configured
        } else {
            validated
                .project_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(configured)
        }
    })
}

fn temporary_output_path(output: &Path) -> PathBuf {
    let stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    output.with_file_name(format!(".{stem}.video-editor-{}.tmp.mp4", Uuid::new_v4()))
}

fn remove_temporary(path: &Path) -> bool {
    fs::remove_file(path).is_ok() || !path.exists()
}

fn effective_dimensions(width: u32, height: u32, preview: bool) -> (u32, u32) {
    if !preview || width.max(height) <= 640 {
        return (width, height);
    }
    let scale = 640.0 / f64::from(width.max(height));
    let scaled_width = ((f64::from(width) * scale).round() as u32).max(2) / 2 * 2;
    let scaled_height = ((f64::from(height) * scale).round() as u32).max(2) / 2 * 2;
    (scaled_width, scaled_height)
}

#[allow(
    clippy::result_large_err,
    reason = "the caller must receive the complete render diagnostic"
)]
fn load_images(validated: &ValidatedProject) -> Result<BTreeMap<String, RgbaImage>, RenderError> {
    let mut images = BTreeMap::new();
    for asset in &validated.project.assets {
        if asset.kind == crate::project::AssetType::Image {
            let Some(path) = validated.asset_paths.get(&asset.id) else {
                continue;
            };
            let decoded = image::open(path).map_err(|error| RenderError {
                diagnostic: Diagnostic::error(
                    "MVP-IMAGE-DECODE",
                    Category::Media,
                    format!("cannot decode image '{}': {error}", asset.id),
                    "",
                ),
                temporary_removed: false,
            })?;
            images.insert(asset.id.clone(), decoded.to_rgba8());
        }
    }
    Ok(images)
}

#[allow(
    clippy::result_large_err,
    reason = "the caller must receive the complete render diagnostic"
)]
fn start_ffmpeg(
    validated: &ValidatedProject,
    temporary: &Path,
    width: u32,
    height: u32,
) -> Result<std::process::Child, RenderError> {
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
        ])
        .arg(format!("{width}x{height}"))
        .arg("-framerate")
        .arg(format!(
            "{}/{}",
            validated.frame_rate.0, validated.frame_rate.1
        ))
        .args(["-i", "pipe:0"]);
    if include_audio(validated) {
        let track = validated.project.audio.as_ref().expect("audio checked");
        let path = validated
            .asset_paths
            .get(&track.asset)
            .expect("audio path validated");
        let source_duration = validated
            .audio_durations
            .get(&track.asset)
            .copied()
            .expect("audio duration validated");
        let selected_duration = track.trim_end.unwrap_or(source_duration) - track.trim_start;
        command
            .arg("-ss")
            .arg(seconds(track.trim_start))
            .arg("-t")
            .arg(seconds(selected_duration))
            .arg("-i")
            .arg(path);
        command.args([
            "-filter_complex",
            &audio_filter(track, selected_duration, validated.duration),
            "-map",
            "0:v:0",
            "-map",
            "[audio]",
        ]);
    } else {
        command.args(["-map", "0:v:0"]);
    }
    command.args([
        "-frames:v",
        &validated.frame_count.to_string(),
        "-c:v",
        "libx264",
        "-crf",
        &validated.project.output.quality.crf().to_string(),
        "-pix_fmt",
        "yuv420p",
    ]);
    if include_audio(validated) {
        command.args(["-c:a", "aac", "-b:a", "192k"]);
    }
    command
        .args(["-movflags", "+faststart"])
        .arg(temporary)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| RenderError {
            diagnostic: Diagnostic::error(
                "MVP-BACKEND-START",
                Category::Backend,
                format!("cannot start FFmpeg: {error}"),
                "",
            ),
            temporary_removed: false,
        })
}

fn audio_filter(
    track: &crate::project::AudioTrack,
    selected_duration: f64,
    project_duration: f64,
) -> String {
    let mut filters = vec![
        "[1:a]asetpts=PTS-STARTPTS".to_owned(),
        format!("volume={}", seconds(track.volume)),
    ];
    if track.fade_in > 0.0 {
        filters.push(format!("afade=t=in:st=0:d={}", seconds(track.fade_in)));
    }
    if track.fade_out > 0.0 {
        filters.push(format!(
            "afade=t=out:st={}:d={}",
            seconds((selected_duration - track.fade_out).max(0.0)),
            seconds(track.fade_out)
        ));
    }
    filters.push(format!(
        "adelay={}:all=1",
        (track.timeline_start * 1000.0).round() as u64
    ));
    filters.push(format!("apad=whole_dur={}", seconds(project_duration)));
    filters.push(format!(
        "atrim=duration={}[audio]",
        seconds(project_duration)
    ));
    filters.join(",")
}

fn seconds(value: f64) -> String {
    format!("{value:.9}")
}
fn include_audio(validated: &ValidatedProject) -> bool {
    validated.project.output.audio
        && validated
            .project
            .audio
            .as_ref()
            .is_some_and(|audio| !audio.mute)
}

fn compose_frame(
    validated: &ValidatedProject,
    images: &BTreeMap<String, RgbaImage>,
    width: u32,
    height: u32,
    time: u128,
) -> RgbaImage {
    let background = parse_colour(&validated.project.output.background).expect("validated colour");
    let mut canvas = RgbaImage::from_pixel(width, height, Rgba(background));
    let time_seconds = time as f64 / 1_000_000_000.0;
    let mut items = Vec::new();
    for clip in &validated.project.visual.clips {
        let start = crate::timeline::seconds_to_nanos(clip.start).unwrap_or(0);
        let duration = crate::timeline::seconds_to_nanos(clip.duration).unwrap_or(0);
        if clip.visible && active(time, start, duration) {
            items.push(Item::Clip(clip));
        }
    }
    for flash in &validated.project.visual.flashes {
        let start = crate::timeline::seconds_to_nanos(flash.start).unwrap_or(0);
        let duration = crate::timeline::seconds_to_nanos(flash.duration).unwrap_or(0);
        if active(time, start, duration) {
            items.push(Item::Flash(flash));
        }
    }
    items.sort_by(|left, right| left.key().cmp(&right.key()));
    for item in items {
        match item {
            Item::Clip(clip) => {
                if let Some(source) = images.get(&clip.asset) {
                    draw_clip(
                        &mut canvas,
                        source,
                        clip,
                        &validated.project.visual.transitions,
                        time_seconds,
                    );
                }
            }
            Item::Flash(flash) => draw_flash(&mut canvas, flash, time_seconds),
        }
    }
    canvas
}

enum Item<'a> {
    Clip(&'a Clip),
    Flash(&'a Flash),
}
impl Item<'_> {
    fn key(&self) -> (i32, u64, &str, u8) {
        match self {
            Self::Clip(clip) => (
                clip.layer,
                (clip.start * 1_000_000_000.0).round() as u64,
                &clip.id,
                0,
            ),
            Self::Flash(flash) => (
                flash.layer,
                (flash.start * 1_000_000_000.0).round() as u64,
                &flash.id,
                1,
            ),
        }
    }
}

fn draw_clip(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    clip: &Clip,
    transitions: &[Transition],
    time: f64,
) {
    let relative = time - clip.start;
    let position = evaluate_point(clip.position, &clip.animations, relative);
    let crop = evaluate_crop(
        clip.crop.unwrap_or(Crop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }),
        &clip.animations,
        relative,
    );
    let scale = evaluate_scalar(1.0, AnimationTarget::Scale, &clip.animations, relative);
    let opacity = evaluate_scalar(
        clip.opacity,
        AnimationTarget::Opacity,
        &clip.animations,
        relative,
    ) * transition_opacity(clip, transitions, time);
    if opacity <= 0.0 {
        return;
    }
    let cropped = crop_image(source, crop);
    let (base_width, base_height) = sizing_dimensions(
        &clip.sizing,
        cropped.width(),
        cropped.height(),
        canvas.width(),
        canvas.height(),
    );
    let target_width = (f64::from(base_width) * scale).round().max(1.0) as u32;
    let target_height = (f64::from(base_height) * scale).round().max(1.0) as u32;
    let resized =
        image::imageops::resize(&cropped, target_width, target_height, FilterType::Lanczos3);
    let x = (position.x * f64::from(canvas.width()) - clip.anchor.x * f64::from(target_width))
        .round() as i64;
    let y = (position.y * f64::from(canvas.height()) - clip.anchor.y * f64::from(target_height))
        .round() as i64;
    blend_at(canvas, &resized, x, y, opacity);
}

fn evaluate_point(base: Point, animations: &[Animation], time: f64) -> Point {
    let mut value = base;
    for animation in sorted(animations, AnimationTarget::Position) {
        let start = serde_json::from_value::<Point>(animation.start_value.clone())
            .expect("validated animation");
        let end = serde_json::from_value::<Point>(animation.end_value.clone())
            .expect("validated animation");
        match animation_progress(animation, time) {
            None => break,
            Some(progress) => {
                value = if progress >= 1.0 {
                    end
                } else {
                    interpolate_point(start, end, progress)
                };
            }
        }
    }
    value
}
fn evaluate_crop(base: Crop, animations: &[Animation], time: f64) -> Crop {
    let mut value = base;
    for animation in sorted(animations, AnimationTarget::Crop) {
        let start = serde_json::from_value::<Crop>(animation.start_value.clone())
            .expect("validated animation");
        let end = serde_json::from_value::<Crop>(animation.end_value.clone())
            .expect("validated animation");
        match animation_progress(animation, time) {
            None => break,
            Some(progress) => {
                value = if progress >= 1.0 {
                    end
                } else {
                    interpolate_crop(start, end, progress)
                };
            }
        }
    }
    value
}
fn evaluate_scalar(base: f64, target: AnimationTarget, animations: &[Animation], time: f64) -> f64 {
    let mut value = base;
    for animation in sorted(animations, target) {
        let start = animation.start_value.as_f64().expect("validated animation");
        let end = animation.end_value.as_f64().expect("validated animation");
        match animation_progress(animation, time) {
            None => break,
            Some(progress) => {
                value = if progress >= 1.0 {
                    end
                } else {
                    start + (end - start) * progress
                };
            }
        }
    }
    value
}
fn sorted(animations: &[Animation], target: AnimationTarget) -> Vec<&Animation> {
    let mut items: Vec<_> = animations
        .iter()
        .filter(|animation| animation.target == target)
        .collect();
    items.sort_by(|left, right| left.start.total_cmp(&right.start));
    items
}

fn crop_image(source: &RgbaImage, crop: Crop) -> RgbaImage {
    let x = (crop.x * f64::from(source.width())).floor() as u32;
    let y = (crop.y * f64::from(source.height())).floor() as u32;
    let right = ((crop.x + crop.width) * f64::from(source.width())).ceil() as u32;
    let bottom = ((crop.y + crop.height) * f64::from(source.height())).ceil() as u32;
    image::imageops::crop_imm(source, x, y, right - x, bottom - y).to_image()
}
fn sizing_dimensions(
    sizing: &Sizing,
    source_width: u32,
    source_height: u32,
    canvas_width: u32,
    canvas_height: u32,
) -> (u32, u32) {
    match sizing {
        Sizing::Original => (source_width, source_height),
        Sizing::Stretch { width, height } => (*width, *height),
        Sizing::Scale { scale } => (
            (f64::from(source_width) * scale).round().max(1.0) as u32,
            (f64::from(source_height) * scale).round().max(1.0) as u32,
        ),
        Sizing::Fit | Sizing::Cover => {
            let horizontal = f64::from(canvas_width) / f64::from(source_width);
            let vertical = f64::from(canvas_height) / f64::from(source_height);
            let factor = if matches!(sizing, Sizing::Fit) {
                horizontal.min(vertical)
            } else {
                horizontal.max(vertical)
            };
            (
                (f64::from(source_width) * factor).round().max(1.0) as u32,
                (f64::from(source_height) * factor).round().max(1.0) as u32,
            )
        }
    }
}

fn transition_opacity(clip: &Clip, transitions: &[Transition], time: f64) -> f64 {
    for transition in transitions {
        match transition {
            Transition::Crossfade {
                outgoing,
                incoming,
                start,
                duration,
                easing,
                ..
            } if outgoing == &clip.id || incoming == &clip.id => {
                let t = eased(*easing, ((time - start) / duration).clamp(0.0, 1.0));
                return if outgoing == &clip.id { 1.0 - t } else { t };
            }
            Transition::FadeToBackground {
                clip: id,
                start,
                duration,
                easing,
                ..
            } if id == &clip.id => {
                return 1.0 - eased(*easing, ((time - start) / duration).clamp(0.0, 1.0));
            }
            Transition::FadeFromBackground {
                clip: id,
                start,
                duration,
                easing,
                ..
            } if id == &clip.id => {
                return eased(*easing, ((time - start) / duration).clamp(0.0, 1.0));
            }
            _ => {}
        }
    }
    1.0
}
fn draw_flash(canvas: &mut RgbaImage, flash: &Flash, time: f64) {
    let relative = time - flash.start;
    let attack = if flash.fade_in > 0.0 && relative < flash.fade_in {
        relative / flash.fade_in
    } else {
        1.0
    };
    let release_start = flash.duration - flash.fade_out;
    let release = if flash.fade_out > 0.0 && relative > release_start {
        (flash.duration - relative) / flash.fade_out
    } else {
        1.0
    };
    let opacity = flash.opacity * attack.min(release).clamp(0.0, 1.0);
    let colour = parse_colour(&flash.colour).expect("validated colour");
    let source = RgbaImage::from_pixel(canvas.width(), canvas.height(), Rgba(colour));
    blend_at(canvas, &source, 0, 0, opacity);
}
fn blend_at(
    canvas: &mut RgbaImage,
    source: &RgbaImage,
    offset_x: i64,
    offset_y: i64,
    opacity: f64,
) {
    for (x, y, pixel) in source.enumerate_pixels() {
        let dx = offset_x + i64::from(x);
        let dy = offset_y + i64::from(y);
        if dx < 0 || dy < 0 || dx >= i64::from(canvas.width()) || dy >= i64::from(canvas.height()) {
            continue;
        }
        let destination = canvas.get_pixel_mut(dx as u32, dy as u32);
        *destination = source_over(*destination, *pixel, opacity);
    }
}
fn source_over(destination: Rgba<u8>, source: Rgba<u8>, opacity: f64) -> Rgba<u8> {
    let source_alpha = f64::from(source[3]) / 255.0 * opacity;
    let destination_alpha = f64::from(destination[3]) / 255.0;
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if alpha <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mut result = [0; 4];
    for channel in 0..3 {
        let value = (f64::from(source[channel]) * source_alpha
            + f64::from(destination[channel]) * destination_alpha * (1.0 - source_alpha))
            / alpha;
        result[channel] = value.round().clamp(0.0, 255.0) as u8;
    }
    result[3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alpha_composition_is_known() {
        let result = source_over(Rgba([0, 0, 255, 255]), Rgba([255, 0, 0, 128]), 1.0);
        assert_eq!(result, Rgba([128, 0, 127, 255]));
    }
    #[test]
    fn preview_keeps_aspect_ratio() {
        assert_eq!(effective_dimensions(1920, 1080, true), (640, 360));
    }
}
