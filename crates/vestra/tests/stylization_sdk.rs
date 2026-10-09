//! Public JSON/value authoring remains the Rust SDK route for visual effects.

use std::time::Duration;

use serde_json::json;
use vestra::{BackendPreference, Editor, PrepareOptions, Project};

#[test]
fn public_value_project_renders_ordered_palette_effects_and_periodic_seeks() {
    let project = Project::from_value(
        json!({
            "schema_version": 1,
            "output": {"path": "unused.mp4", "width": 4, "height": 4,
                "frame_rate": "4/1", "background": "#808080", "quality": "preview",
                "audio": false, "duration_mode": "explicit", "duration": 4},
            "assets": [],
            "visual": {"clips": [{"id": "background", "source": {
                "type": "solid_color", "colour": "#808080"}, "start": 0,
                "duration": 4, "layer": 0, "opacity": {"base_value": 1}}], "post_effects": [
                {"id": "red-ramp", "type": "palette_map",
                    "palette": ["#000000", "#ff0000"],
                    "mode": "gradient", "amount": {"base_value": 1}, "phase": {"base_value": 0}},
                {"id": "fine-dither", "type": "ordered_dither",
                    "palette": ["#000000", "#ffffff"],
                    "mode": "nearest", "amount": {"base_value": 1}, "phase": {"base_value": 0},
                    "period": 2, "strength": {"base_value": 1}, "matrix": "bayer4", "scale": 1}
            ]}
        }),
        ".",
    )
    .expect("load effects through public value API");
    let editor = Editor::new();
    let validation = editor.validate(&project);
    assert!(validation.is_valid(), "{validation:?}");
    let mut prepared = editor
        .prepare(&project, PrepareOptions::new(BackendPreference::Cpu))
        .expect("prepare public effect project");
    let later = prepared
        .render_frame(Duration::from_millis(2500))
        .expect("seek later cycle");
    let middle = prepared
        .render_frame(Duration::from_millis(500))
        .expect("seek first cycle");
    assert_eq!(
        middle.as_bytes(),
        later.as_bytes(),
        "a two-second period repeats at random-access times"
    );
    assert!(
        middle
            .as_bytes()
            .chunks_exact(4)
            .all(|pixel| pixel == [128, 128, 128, 255])
    );
    let boundary_later = prepared
        .render_frame(Duration::from_secs(2))
        .expect("loop boundary");
    let start = prepared.render_frame(Duration::ZERO).expect("first frame");
    assert_eq!(start.as_bytes(), boundary_later.as_bytes());
    // PaletteMap first changes gray128 to red128. Its integer luminance is
    // 6912/65280, so only Bayer4 ranks14 and15 choose white: (2,1) and (0,3).
    let expected = [
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
        [0, 0, 0, 255],
    ];
    assert_eq!(start.as_bytes(), expected.as_flattened());
    let repeated = prepared
        .render_frame(Duration::ZERO)
        .expect("repeat first frame");
    assert_eq!(start.as_bytes(), repeated.as_bytes());
}
