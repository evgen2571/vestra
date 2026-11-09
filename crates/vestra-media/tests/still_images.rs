use std::fs;

use vestra_media::save_frame;

#[test]
fn lossless_stills_preserve_rgba_and_publish_atomically() {
    let directory = tempfile::tempdir().unwrap();
    let rgba = [240, 80, 20, 128, 50, 100, 150, 0];
    for extension in ["png", "webp", "PNG"] {
        let path = directory.path().join(format!("frame.{extension}"));
        save_frame(&path, 2, 1, &rgba, false).unwrap();
        assert_eq!(image::open(&path).unwrap().into_rgba8().as_raw(), &rgba);
        assert!(save_frame(&path, 2, 1, &[0; 8], false).is_err());
        assert_eq!(image::open(&path).unwrap().into_rgba8().as_raw(), &rgba);
        save_frame(&path, 2, 1, &[0; 8], true).unwrap();
        assert_eq!(image::open(&path).unwrap().into_rgba8().as_raw(), &[0; 8]);
    }
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
}

#[test]
fn jpeg_composites_straight_alpha_over_black() {
    let directory = tempfile::tempdir().unwrap();
    let rgba = [240, 80, 20, 128].repeat(16 * 16);
    for extension in ["jpg", "jpeg"] {
        let path = directory.path().join(format!("frame.{extension}"));
        save_frame(&path, 16, 16, &rgba, false).unwrap();
        let decoded = image::open(path).unwrap().into_rgb8();
        for pixel in decoded.pixels() {
            for (channel, expected) in pixel.0.into_iter().zip([120u8, 40, 10]) {
                assert!(channel.abs_diff(expected) <= 3);
            }
        }
    }
}

#[test]
fn invalid_still_requests_leave_no_files_or_preserve_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    for (name, width, height, rgba) in [
        ("bad.png", 2, 1, &[0u8; 4][..]),
        ("empty.png", 0, 1, &[][..]),
        ("bad.gif", 1, 1, &[0; 4][..]),
    ] {
        let path = directory.path().join(name);
        fs::write(&path, b"existing").unwrap();
        assert!(save_frame(&path, width, height, rgba, true).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing");
    }
    assert!(
        save_frame(
            &directory.path().join("missing/frame.png"),
            1,
            1,
            &[0; 4],
            false
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 3);
}
