struct Params {
    canvas: vec4<u32>,
    inverse_row0: vec4<f32>,
    inverse_row1: vec4<f32>,
    operation: u32,
    invert: u32,
    strength: f32,
    feather: f32,
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var mask: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;

fn combine(current: f32, value: f32) -> f32 {
    let m = select(value, 1.0 - value, params.invert != 0u);
    let operation = params.operation & 3u;
    var combined = m;
    if (operation == 1u) { combined = current * m; }
    if (operation == 2u) { combined = current + m - current * m; }
    if (operation == 3u) { combined = current * (1.0 - m); }
    return clamp(current + (combined - current) * clamp(params.strength, 0.0, 1.0), 0.0, 1.0);
}

fn coverage_texel(coord: vec2<i32>) -> f32 {
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(params.canvas.x) || coord.y >= i32(params.canvas.y)) {
        return 0.0;
    }
    return textureLoad(mask, coord, 0).a;
}

fn bilinear_coverage(position: vec2<f32>) -> f32 {
    let adjusted = position - vec2<f32>(0.5);
    let base = vec2<i32>(floor(adjusted));
    let fraction = adjusted - vec2<f32>(base);
    let top = mix(
        coverage_texel(base),
        coverage_texel(base + vec2<i32>(1, 0)),
        fraction.x,
    );
    let bottom = mix(
        coverage_texel(base + vec2<i32>(0, 1)),
        coverage_texel(base + vec2<i32>(1, 1)),
        fraction.x,
    );
    return mix(top, bottom, fraction.y);
}

fn feathered_coverage(position: vec2<f32>) -> f32 {
    let radius = clamp(params.feather, 0.0, 256.0);
    if (radius <= 0.0) { return bilinear_coverage(position); }
    let step_x = vec2<f32>(params.inverse_row0.x, params.inverse_row1.x);
    let step_y = vec2<f32>(params.inverse_row0.y, params.inverse_row1.y);
    let sigma = radius / 3.0;
    var total = 0.0;
    var weight_total = 0.0;
    for (var y = -4; y <= 4; y++) {
        let offset_y = f32(y) * radius / 4.0;
        let weight_y = exp(-0.5 * pow(offset_y / sigma, 2.0));
        var horizontal = 0.0;
        var horizontal_weight = 0.0;
        for (var x = -4; x <= 4; x++) {
            let offset_x = f32(x) * radius / 4.0;
            let weight_x = exp(-0.5 * pow(offset_x / sigma, 2.0));
            horizontal += weight_x * bilinear_coverage(position + step_y * offset_y + step_x * offset_x);
            horizontal_weight += weight_x;
        }
        total += weight_y * horizontal / horizontal_weight;
        weight_total += weight_y;
    }
    return total / weight_total;
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas.x || id.y >= params.canvas.y) { return; }
    let coord = vec2<i32>(id.xy);
    let point = vec2<f32>(id.xy) + vec2<f32>(0.5);
    let local = vec2<f32>(
        dot(params.inverse_row0.xyz, vec3<f32>(point, 1.0)),
        dot(params.inverse_row1.xyz, vec3<f32>(point, 1.0)),
    );
    let value = feathered_coverage(local);
    let previous = textureLoad(mask, coord, 0);
    let first = (params.operation & 4u) != 0u;
    var current = previous.g;
    var base_alpha = previous.r;
    if (first) {
        current = 1.0;
        base_alpha = textureLoad(source, coord, 0).a;
    }
    textureStore(output, coord, vec4<f32>(base_alpha, combine(current, value), 0.0, 0.0));
}
