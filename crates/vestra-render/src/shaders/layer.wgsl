// Encoded straight-alpha RGBA8 texture layer renderer. Coordinate origin is
// top-left and destination pixels are addressed at pixel centres.
struct Params {
    canvas_width: u32, canvas_height: u32, _unused_stride: u32, mode: u32,
    source_width: u32, source_height: u32, source_origin_x: u32, source_origin_y: u32,
    crop: vec4<f32>, effective: vec4<f32>,
    inverse_row0: vec4<f32>, inverse_row1: vec4<f32>,
    colour_row0: vec4<f32>, colour_row1: vec4<f32>, colour_row2: vec4<f32>,
    colour_offset: vec4<f32>, solid_or_background: vec4<f32>,
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(2) var<uniform> params: Params;

fn texel(coord: vec2<i32>) -> vec4<f32> {
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(params.source_width) || coord.y >= i32(params.source_height)) { return vec4<f32>(0.0); }
    return textureLoad(source, coord + vec2<i32>(i32(params.source_origin_x), i32(params.source_origin_y)), 0) * 255.0;
}
fn bilinear(position: vec2<f32>) -> vec4<f32> {
    let adjusted = position - vec2<f32>(0.5); let base = vec2<i32>(floor(adjusted)); let fraction = adjusted - vec2<f32>(base);
    let weights = vec4<f32>(
        (1.0 - fraction.x) * (1.0 - fraction.y),
        fraction.x * (1.0 - fraction.y),
        (1.0 - fraction.x) * fraction.y,
        fraction.x * fraction.y,
    );
    let top_left = texel(base);
    let top_right = texel(base + vec2<i32>(1, 0));
    let bottom_left = texel(base + vec2<i32>(0, 1));
    let bottom_right = texel(base + vec2<i32>(1, 1));
    let samples = vec4<f32>(top_left.a, top_right.a, bottom_left.a, bottom_right.a) / 255.0;
    let weighted_alpha = samples * weights;
    var premultiplied = vec3<f32>(0.0);
    let alpha = dot(weighted_alpha, vec4<f32>(1.0));
    premultiplied = premultiplied
        + top_left.rgb / 255.0 * weighted_alpha.x
        + top_right.rgb / 255.0 * weighted_alpha.y
        + bottom_left.rgb / 255.0 * weighted_alpha.z
        + bottom_right.rgb / 255.0 * weighted_alpha.w;
    if (alpha <= 0.0000001) { return vec4<f32>(0.0); }
    return vec4<f32>(round(clamp(premultiplied / alpha * 255.0, vec3<f32>(0.0), vec3<f32>(255.0))), round(alpha * 255.0));
}
fn transformed_colour(pixel: vec4<f32>) -> vec4<f32> {
    let rgb = round(clamp(vec3<f32>(dot(params.colour_row0.xyz, pixel.rgb), dot(params.colour_row1.xyz, pixel.rgb), dot(params.colour_row2.xyz, pixel.rgb)) + params.colour_offset.xyz, vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<f32>(rgb, pixel.a * params.effective.z);
}
fn write_pixel(coord: vec2<i32>, pixel: vec4<f32>) { textureStore(output, coord, clamp(pixel / 255.0, vec4<f32>(0.0), vec4<f32>(1.0))); }
@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coord = vec2<i32>(id.xy);
    if (params.mode == 0u) { write_pixel(coord, params.solid_or_background); return; }
    if (params.mode == 2u) { write_pixel(coord, transformed_colour(params.solid_or_background)); return; }
    let point = vec2<f32>(f32(id.x) + 0.5, f32(id.y) + 0.5);
    let mapped = vec2<f32>(dot(params.inverse_row0.xyz, vec3<f32>(point, 1.0)), dot(params.inverse_row1.xyz, vec3<f32>(point, 1.0)));
    let local_origin = params.solid_or_background.xy;
    if (mapped.x < local_origin.x || mapped.y < local_origin.y || mapped.x >= local_origin.x + params.effective.x || mapped.y >= local_origin.y + params.effective.y) { write_pixel(coord, vec4<f32>(0.0)); return; }
    let local = mapped - local_origin;
    let source_position = vec2<f32>(params.crop.x * f32(params.source_width), params.crop.y * f32(params.source_height)) + local / params.effective.xy * vec2<f32>(params.crop.z * f32(params.source_width), params.crop.w * f32(params.source_height));
    write_pixel(coord, transformed_colour(bilinear(source_position)));
}
