//! Adapter-independent readback row-layout checks.

use super::requirements::align_up;

#[test]
fn readback_row_alignment_matches_wgpu_copy_requirements() {
    for (width, expected) in [
        (62_u32, 256_u32),
        (64, 256),
        (66, 512),
        (318, 1280),
        (320, 1280),
        (322, 1536),
        (718, 3072),
        (720, 3072),
        (722, 3072),
        (1080, 4352),
    ] {
        assert_eq!(
            align_up(width * 4, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT),
            expected
        );
    }
}

#[test]
fn row_repacking_removes_padding_without_shifting_rows() {
    let row_bytes = 62 * 4;
    let padded = align_up(row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let mut mapped = vec![0_u8; (padded * 3) as usize];
    for row in 0..3_usize {
        mapped[row * padded as usize..row * padded as usize + row_bytes as usize]
            .fill((row + 1) as u8);
    }
    let mut contiguous = vec![0_u8; (row_bytes * 3) as usize];
    for (row, target) in contiguous.chunks_exact_mut(row_bytes as usize).enumerate() {
        let start = row * padded as usize;
        target.copy_from_slice(&mapped[start..start + row_bytes as usize]);
    }
    assert_eq!(contiguous.len(), (62 * 3 * 4) as usize);
    assert!(
        contiguous[..row_bytes as usize]
            .iter()
            .all(|byte| *byte == 1)
    );
    assert!(
        contiguous[row_bytes as usize..row_bytes as usize * 2]
            .iter()
            .all(|byte| *byte == 2)
    );
    assert!(
        contiguous[row_bytes as usize * 2..]
            .iter()
            .all(|byte| *byte == 3)
    );
}
