use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use image::{ExtendedColorType, ImageFormat};

use crate::{MediaError, OutputTarget};

/// Saves tightly packed straight-alpha RGBA8. PNG/WebP are lossless; JPEG
/// composites over black. Publication preserves an existing file on failure.
pub fn save_frame(
    path: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
    overwrite: bool,
) -> Result<(), MediaError> {
    let format = match path.extension().and_then(|value| value.to_str()) {
        Some(value) if value.eq_ignore_ascii_case("png") => ImageFormat::Png,
        Some(value) if value.eq_ignore_ascii_case("jpg") || value.eq_ignore_ascii_case("jpeg") => {
            ImageFormat::Jpeg
        }
        Some(value) if value.eq_ignore_ascii_case("webp") => ImageFormat::WebP,
        _ => return Err(MediaError::UnsupportedStillFormat),
    };
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4));
    if width == 0 || height == 0 || bytes != u64::try_from(rgba.len()).ok() {
        return Err(MediaError::InvalidImageLayout);
    }
    let output = OutputTarget::prepare(path.to_path_buf(), overwrite)?;
    let result = (|| {
        let mut writer =
            BufWriter::new(File::create(&output.temporary_path).map_err(MediaError::ImageWrite)?);
        if format == ImageFormat::Jpeg {
            let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
            for pixel in rgba.chunks_exact(4) {
                for channel in &pixel[..3] {
                    rgb.push(((u16::from(*channel) * u16::from(pixel[3]) + 127) / 255) as u8);
                }
            }
            image::write_buffer_with_format(
                &mut writer,
                &rgb,
                width,
                height,
                ExtendedColorType::Rgb8,
                format,
            )?;
        } else {
            image::write_buffer_with_format(
                &mut writer,
                rgba,
                width,
                height,
                ExtendedColorType::Rgba8,
                format,
            )?;
        }
        // BufWriter's destructor ignores flush errors; never publish from it.
        writer.flush().map_err(MediaError::ImageWrite)?;
        drop(writer);
        output.publish()
    })();
    if result.is_err() {
        let _ = output.cleanup();
    }
    result
}
