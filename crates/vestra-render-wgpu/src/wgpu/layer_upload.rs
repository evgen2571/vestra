//! Padded staging rows for CPU-rasterized source layers.

use crate::Diagnostic;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LayerUpload {
    pub(super) offset: wgpu::BufferAddress,
    pub(super) bytes_per_row: u32,
}

pub(super) fn append_layer_upload(
    arena: &mut Vec<u8>,
    source: &[u8],
    width: u32,
    height: u32,
    max_buffer_size: u64,
) -> Result<LayerUpload, Diagnostic> {
    if width == 0 || height == 0 {
        return Err(Diagnostic::error(
            "WGPU-LAYER-UPLOAD",
            crate::Category::Backend,
            "layer upload dimensions must be non-zero",
            "",
        ));
    }
    let row_bytes = width.checked_mul(4).ok_or_else(|| {
        Diagnostic::error(
            "WGPU-LAYER-UPLOAD",
            crate::Category::Backend,
            "layer row size overflow",
            "",
        )
    })?;
    let bytes_per_row = u64::from(row_bytes)
        .div_ceil(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
        .checked_mul(u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT))
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            Diagnostic::error(
                "WGPU-LAYER-UPLOAD",
                crate::Category::Backend,
                "layer padded row size overflow",
                "",
            )
        })?;
    let source_len = usize::try_from(row_bytes)
        .ok()
        .and_then(|row| {
            usize::try_from(height)
                .ok()
                .and_then(|rows| row.checked_mul(rows))
        })
        .ok_or_else(|| {
            Diagnostic::error(
                "WGPU-LAYER-UPLOAD",
                crate::Category::Backend,
                "layer source size overflow",
                "",
            )
        })?;
    if source.len() != source_len {
        return Err(Diagnostic::error(
            "WGPU-LAYER-UPLOAD",
            crate::Category::Backend,
            "layer source has an unexpected size",
            "",
        ));
    }
    let offset = arena.len();
    debug_assert!(offset.is_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT as usize));
    let padded_length = u64::from(bytes_per_row) * u64::from(height);
    let length = (offset as u64).checked_add(padded_length).ok_or_else(|| {
        Diagnostic::error(
            "WGPU-LAYER-UPLOAD",
            crate::Category::Backend,
            "combined layer upload size overflow",
            "",
        )
    })?;
    if length > max_buffer_size {
        return Err(Diagnostic::error(
            "WGPU-LAYER-UPLOAD-LIMIT",
            crate::Category::Backend,
            format!(
                "layer uploads require {length} bytes, exceeding device max_buffer_size {max_buffer_size}"
            ),
            "",
        ));
    }
    let allocation_error = || {
        Diagnostic::error(
            "WGPU-LAYER-UPLOAD",
            crate::Category::Backend,
            format!("cannot allocate {length} bytes for layer uploads"),
            "",
        )
    };
    let length = usize::try_from(length).map_err(|_| allocation_error())?;
    arena
        .try_reserve_exact(length - offset)
        .map_err(|_| allocation_error())?;
    arena.resize(length, 0);
    let padded_row = usize::try_from(bytes_per_row).expect("WGPU row alignment fits usize");
    let row = usize::try_from(row_bytes).expect("validated row size fits usize");
    for (destination, source) in arena[offset..]
        .chunks_exact_mut(padded_row)
        .zip(source.chunks_exact(row))
    {
        destination[..row].copy_from_slice(source);
    }
    Ok(LayerUpload {
        offset: offset as wgpu::BufferAddress,
        bytes_per_row,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_upload_pads_each_row_and_preserves_row_order() {
        let source: Vec<u8> = (0..24).collect();
        let mut arena = Vec::new();
        let upload =
            append_layer_upload(&mut arena, &source, 3, 2, u64::MAX).expect("packed upload");

        assert_eq!(
            upload,
            LayerUpload {
                offset: 0,
                bytes_per_row: 256,
            }
        );
        assert_eq!(&arena[..12], &source[..12]);
        assert_eq!(&arena[256..268], &source[12..]);
        assert!(arena[12..256].iter().all(|byte| *byte == 0));
        assert!(arena[280..].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn layer_upload_regions_are_distinct_and_aligned() {
        for width in [1, 2, 63, 64, 65, 100, 128, 1920] {
            let source_a = vec![0x11; width * 4];
            let source_b = vec![0x22; width * 4];
            let mut arena = Vec::new();
            let first = append_layer_upload(&mut arena, &source_a, width as u32, 1, u64::MAX)
                .expect("first packed upload");
            let second = append_layer_upload(&mut arena, &source_b, width as u32, 1, u64::MAX)
                .expect("second packed upload");
            assert_eq!(first.offset % 4, 0);
            assert_eq!(second.offset % 4, 0);
            assert_eq!(first.bytes_per_row % 256, 0);
            assert_eq!(second.bytes_per_row % 256, 0);
            assert!(second.offset >= first.offset + u64::from(first.bytes_per_row));
            assert_eq!(arena[first.offset as usize], 0x11);
            assert_eq!(arena[second.offset as usize], 0x22);
        }
    }

    #[test]
    fn layer_upload_rejects_zero_dimensions_and_wrong_source_size() {
        assert!(append_layer_upload(&mut Vec::new(), &[], 0, 1, u64::MAX).is_err());
        assert!(append_layer_upload(&mut Vec::new(), &[0; 4], 2, 1, u64::MAX).is_err());
    }

    #[test]
    fn layer_upload_checks_the_total_padded_size_before_appending() {
        let mut arena = Vec::new();
        let source = [0x11; 12];
        append_layer_upload(&mut arena, &source, 3, 1, 511).unwrap();
        let original = arena.clone();
        let error = append_layer_upload(&mut arena, &source, 3, 1, 511).unwrap_err();
        assert_eq!(error.code, "WGPU-LAYER-UPLOAD-LIMIT");
        assert!(error.message.contains("512"));
        assert_eq!(arena, original);
    }
}
