struct Params {
    canvas: vec4<u32>,
    inverse_row0: vec4<f32>,
    inverse_row1: vec4<f32>,
    operation: u32,
    invert: u32,
    strength: f32,
    _padding: u32,
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

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas.x || id.y >= params.canvas.y) { return; }
    let coord = vec2<i32>(id.xy);
    let point = vec2<f32>(id.xy) + vec2<f32>(0.5);
    let local = vec2<f32>(
        dot(params.inverse_row0.xyz, vec3<f32>(point, 1.0)),
        dot(params.inverse_row1.xyz, vec3<f32>(point, 1.0)),
    );
    let local_coord = vec2<i32>(floor(local));
    let safe_coord = clamp(
        local_coord,
        vec2<i32>(0, 0),
        vec2<i32>(i32(params.canvas.x) - 1, i32(params.canvas.y) - 1),
    );
    let value = select(
        0.0,
        textureLoad(mask, safe_coord, 0).a,
        local_coord.x >= 0 && local_coord.y >= 0
            && local_coord.x < i32(params.canvas.x)
            && local_coord.y < i32(params.canvas.y),
    );
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
