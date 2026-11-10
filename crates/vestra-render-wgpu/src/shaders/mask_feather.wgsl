struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    radius: f32, direction: u32, _padding1: vec2<u32>,
    inverse_row0: vec4<f32>, inverse_row1: vec4<f32>,
    local_extent: vec2<u32>, transform_owner: u32, preserve_state: u32,
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;

fn local_coverage(coord: vec2<i32>) -> f32 {
    if (any(coord < vec2<i32>(0)) || any(coord >= vec2<i32>(params.local_extent))) { return 0.0; }
    return textureLoad(source, coord, 0).a;
}

fn coverage(coord: vec2<i32>) -> f32 {
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(params.canvas_width) || coord.y >= i32(params.canvas_height)) {
        return 0.0;
    }
    if (params.transform_owner == 0u) {
        let pixel = textureLoad(source, coord, 0);
        return pixel.a + select(0.0, (pixel.b - 0.5) / 255.0, params.preserve_state != 0u);
    }
    let point = vec3<f32>(vec2<f32>(coord) + vec2<f32>(0.5), 1.0);
    let position = vec2<f32>(dot(params.inverse_row0.xyz, point), dot(params.inverse_row1.xyz, point));
    if (any(position < vec2<f32>(0.0)) || any(position >= vec2<f32>(params.local_extent))) { return 0.0; }
    let adjusted = position - vec2<f32>(0.5);
    let base = vec2<i32>(floor(adjusted));
    let fraction = adjusted - vec2<f32>(base);
    let value = mix(
        mix(local_coverage(base), local_coverage(base + vec2<i32>(1, 0)), fraction.x),
        mix(local_coverage(base + vec2<i32>(0, 1)), local_coverage(base + vec2<i32>(1, 1)), fraction.x),
        fraction.y,
    );
    return floor(value * 255.0 + 0.5) / 255.0;
}

fn blur(coord: vec2<i32>) -> f32 {
    let half_width = clamp(params.radius, 0.0, 256.0) / 3.0;
    let extent = 2.0 * half_width + 1.0;
    let center = select(f32(coord.x), f32(coord.y), params.direction != 0u);
    let first = i32(floor(center - half_width));
    let last = i32(ceil(center + half_width)) + 1;
    var value = 0.0;
    for (var index = first; index < last; index = index + 1) {
        let overlap = min(f32(index) + 0.5, center + half_width + 0.5)
            - max(f32(index) - 0.5, center - half_width - 0.5);
        if (overlap > 0.0) {
            let sample_coord = select(vec2<i32>(index, coord.y), vec2<i32>(coord.x, index), params.direction != 0u);
            value += coverage(sample_coord) * overlap;
        }
    }
    return value / extent;
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let value = clamp(blur(vec2<i32>(id.xy)), 0.0, 1.0);
    var state = vec3<f32>(0.0);
    if (params.preserve_state != 0u) { state = textureLoad(source, vec2<i32>(id.xy), 0).rgb; }
    var alpha = value;
    if (params.preserve_state != 0u) {
        // Coverage state uses only R/G; retain alpha's rounding residual in B between box passes.
        alpha = floor(value * 255.0 + 0.5) / 255.0;
        state.b = (value - alpha) * 255.0 + 0.5;
    }
    textureStore(output, vec2<i32>(id.xy), vec4<f32>(state, alpha));
}
