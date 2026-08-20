struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    radius: f32, direction: u32, _padding1: vec2<u32>,
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;

fn coverage(coord: vec2<i32>) -> f32 {
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(params.canvas_width) || coord.y >= i32(params.canvas_height)) {
        return 0.0;
    }
    return textureLoad(source, coord, 0).a;
}

fn blur(coord: vec2<i32>) -> f32 {
    let half_width = clamp(params.radius, 0.0, 256.0) / 3.0;
    let extent = 2.0 * half_width + 1.0;
    let center = select(f32(coord.x), f32(coord.y), params.direction != 0u);
    let first = i32(floor(center - half_width - 0.5));
    let last = i32(ceil(center + half_width + 0.5));
    var value = 0.0;
    for (var sample = -256; sample <= 256; sample = sample + 1) {
        let index = first + sample;
        if (index >= first && index < last) {
            let overlap = min(f32(index) + 0.5, center + half_width + 0.5)
                - max(f32(index) - 0.5, center - half_width - 0.5);
            if (overlap > 0.0) {
                let sample_coord = select(vec2<i32>(index, coord.y), vec2<i32>(coord.x, index), params.direction != 0u);
                value += coverage(sample_coord) * overlap;
            }
        }
    }
    return value / extent;
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let value = clamp(blur(vec2<i32>(id.xy)), 0.0, 1.0);
    textureStore(output, vec2<i32>(id.xy), vec4<f32>(0.0, 0.0, 0.0, value));
}
