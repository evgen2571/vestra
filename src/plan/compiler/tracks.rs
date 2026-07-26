//! Conversion of project tracks and interpolation into compiled tracks.

use crate::{
    Diagnostic,
    animation::{Interpolation, Keyframe, Track},
};

pub(super) fn compile<T: Copy>(
    track: &crate::project::Track<T>,
    id: &str,
) -> Result<Track<T>, Diagnostic> {
    let mut keyframes = Vec::with_capacity(track.keyframes.len());
    for keyframe in &track.keyframes {
        keyframes.push(Keyframe {
            time: super::to_nanos(keyframe.time, id)?,
            value: keyframe.value,
            interpolation: interpolation(&keyframe.interpolation),
        });
    }
    Ok(Track {
        base_value: track.base_value,
        keyframes,
    })
}

pub(super) fn degrees_to_radians(mut track: Track<f64>) -> Track<f64> {
    track.base_value = track.base_value.to_radians();
    for keyframe in &mut track.keyframes {
        keyframe.value = keyframe.value.to_radians();
    }
    track
}

pub(super) fn interpolation(interpolation: &crate::project::Interpolation) -> Interpolation {
    interpolation.to_animation()
}
