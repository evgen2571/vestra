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
