//! Integrity checks for WGPU-specific test assets.

#[test]
fn small_rgba_parity_fixture_has_transparency_and_nontrivial_detail() {
    let image = image::open(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/assets/wgpu-small-rgba.png"),
    )
    .expect("WGPU RGBA fixture decodes")
    .into_rgba8();
    assert_eq!(image.dimensions(), (173, 129));
    let alpha = image.pixels().map(|pixel| pixel[3]).collect::<Vec<_>>();
    assert_eq!(alpha.iter().copied().min(), Some(0));
    assert_eq!(alpha.iter().copied().max(), Some(255));
    assert!(alpha.iter().any(|value| (1..255).contains(value)));
    assert!(
        image
            .pixels()
            .map(|pixel| pixel.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            > 32
    );
}
