struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    radius: f32, direction: u32, _padding1: vec2<u32>,
};
@group(0) @binding(3) var<uniform> params: Params;

fn gaussian(coord: vec2<i32>) -> vec4<f32> {
    let radius = clamp(params.radius, 0.0, 32.0);
    let support = i32(ceil(max(radius, 1.0)));
    let sigma = max(radius / 3.0, 0.5);
    let horizontal = params.direction == 0u;
    var total = 0.0;
    var accumulated = vec4<f32>(0.0);
    for (var offset = -32; offset <= 32; offset = offset + 1) {
        if (abs(offset) <= support) {
            let weight = exp(-0.5 * pow(f32(offset) / sigma, 2.0));
            var step = vec2<i32>(0, offset);
            if (horizontal) { step = vec2<i32>(offset, 0); }
            let pixel = load_edge(source, coord + step);
            accumulated += vec4<f32>(pixel.rgb * pixel.a, pixel.a) * weight;
            total += weight;
        }
    }
    let result = accumulated / total;
    if (result.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(result.rgb / result.a, result.a));
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    textureStore(output, vec2<i32>(id.xy), gaussian(vec2<i32>(id.xy)));
}
