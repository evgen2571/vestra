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

use serde_json::{Value, json};

pub(super) fn particle_upload_project(count: usize) -> Value {
    let clips: Vec<_> = (0..count)
        .map(|index| {
            json!({
                "id": format!("particles-{index}"),
                "source": {
                    "type": "particle_system", "seed": 41,
                    "emitter": {"type": "point", "position": {"x": 0.5, "y": 0.5}},
                    "emission": {"bursts": [{"time": 0.0, "count": 8}]},
                    "particle": {
                        "lifetime": 1.0, "size": 0.12, "speed": 0.0,
                        "direction_spread_degrees": 0.0, "colour": "#FFD27A80",
                        "primitive": "square", "blend_mode": "additive"
                    }
                },
                "start": 0.0, "duration": 2.0, "layer": index,
                "opacity": {"base_value": 1.0}
            })
        })
        .collect();
    json!({
        "schema_version": 1,
        "output": {
            "path": "particle-upload.mp4", "width": 1280, "height": 720,
            "frame_rate": "24/1", "background": "#101018", "quality": "preview",
            "audio": false, "duration_mode": "explicit", "duration": 2.0
        },
        "assets": [],
        "visual": {
            "clips": clips,
            "transitions": [], "flashes": [], "post_effects": []
        }
    })
}
