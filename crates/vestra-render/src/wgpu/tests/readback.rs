//! Adapter-independent readback row-layout checks against the production helper.

use super::requirements::align_up;
use crate::wgpu::readback::repack_rows;

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
    repack_rows(&mapped, &mut contiguous, 62, 3, padded).expect("production repack succeeds");
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

#[test]
fn production_repack_handles_unpadded_and_alignment_boundary_widths() {
    for (width, height) in [(1_u32, 1_u32), (63, 2), (64, 2), (65, 3), (320, 2)] {
        let row_bytes = width * 4;
        let padded = align_up(row_bytes, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let mut mapped = vec![0_u8; (padded * height) as usize];
        let expected = (0..height)
            .flat_map(|row| (0..row_bytes).map(move |column| (row * 17 + column) as u8))
            .collect::<Vec<_>>();
        for row in 0..height as usize {
            let source = row * padded as usize;
            mapped[source..source + row_bytes as usize].copy_from_slice(
                &expected[row * row_bytes as usize..(row + 1) * row_bytes as usize],
            );
            mapped[source + row_bytes as usize..source + padded as usize].fill(0xEE);
        }
        let mut packed = vec![0_u8; expected.len()];
        repack_rows(&mapped, &mut packed, width, height, padded).expect("repack valid layout");
        assert_eq!(packed, expected, "width {width}");
    }
}

#[test]
fn production_repack_rejects_invalid_layouts_structurally() {
    let mut destination = vec![0; 8];
    let cases = [
        (
            vec![0; 8],
            2,
            1,
            7,
            "padded stride smaller than packed stride",
        ),
        (
            vec![0; 7],
            2,
            1,
            8,
            "mapped source shorter than required layout",
        ),
        (vec![0; 8], 2, 1, 8, "valid control"),
    ];
    for (mapped, width, height, stride, description) in cases {
        let result = repack_rows(&mapped, &mut destination, width, height, stride);
        if description == "valid control" {
            assert!(result.is_ok(), "{description}");
        } else {
            let error = result.expect_err(description);
            assert_eq!(error.code, "WGPU-READBACK-SIZE");
            assert_eq!(error.category, crate::Category::Backend);
        }
    }
    let mut short_destination = vec![0; 7];
    assert_eq!(
        repack_rows(&[0; 8], &mut short_destination, 2, 1, 8)
            .expect_err("short destination")
            .code,
        "WGPU-READBACK-SIZE"
    );
    for (width, height) in [(0, 1), (1, 0)] {
        assert_eq!(
            repack_rows(&[], &mut [], width, height, 0)
                .expect_err("zero dimensions are invalid")
                .code,
            "WGPU-READBACK-SIZE"
        );
    }
    assert_eq!(
        repack_rows(&[], &mut [], u32::MAX, u32::MAX, u32::MAX)
            .expect_err("layout multiplication overflow")
            .code,
        "WGPU-READBACK-SIZE"
    );
}
