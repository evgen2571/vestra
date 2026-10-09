// Encoded straight-alpha compositing. `mode` follows project::BlendMode:
// normal, add, screen, multiply, overlay. Opacity is applied to source alpha.
struct Params {
    canvas_width: u32, canvas_height: u32, _unused_stride: u32, mode: u32,
    source_width: u32, source_height: u32, source_origin_x: u32, source_origin_y: u32,
    crop: vec4<f32>, effective: vec4<f32>,
    inverse_row0: vec4<f32>, inverse_row1: vec4<f32>,
    colour_row0: vec4<f32>, colour_row1: vec4<f32>, colour_row2: vec4<f32>,
    colour_offset: vec4<f32>, solid_or_background: vec4<f32>,
};
@group(0) @binding(0) var canvas: texture_2d<f32>;
@group(0) @binding(1) var layer: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;
fn source_over(destination: vec4<f32>, source: vec4<f32>) -> vec4<f32> {
    let alpha = source.a + destination.a * (1.0 - source.a);
    if (alpha <= 0.0) { return vec4<f32>(0.0); }
    return vec4<f32>((source.rgb * source.a + destination.rgb * destination.a * (1.0 - source.a)) / alpha, alpha);
}
fn blend(destination: vec4<f32>, unscaled_source: vec4<f32>) -> vec4<f32> {
    let source = vec4<f32>(unscaled_source.rgb, unscaled_source.a * params.effective.z);
    if (params.mode == 0u) { return source_over(destination, source); }
    let alpha = source.a + destination.a * (1.0 - source.a);
    if (alpha <= 0.0) { return vec4<f32>(0.0); }
    var rgb = vec3<f32>(0.0);
    for (var channel = 0; channel < 3; channel = channel + 1) {
        let s = source[channel]; let d = destination[channel];
        var mixed = s;
        if (params.mode == 1u) { mixed = min(s + d, 1.0); }
        if (params.mode == 2u) { mixed = 1.0 - (1.0 - s) * (1.0 - d); }
        if (params.mode == 3u) { mixed = s * d; }
        if (params.mode == 4u) { mixed = select(1.0 - 2.0 * (1.0 - s) * (1.0 - d), 2.0 * s * d, d <= 0.5); }
        rgb[channel] = (mixed * source.a * destination.a + s * source.a * (1.0 - destination.a) + d * destination.a * (1.0 - source.a)) / alpha;
    }
    return vec4<f32>(rgb, alpha);
}
@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coord = vec2<i32>(id.xy);
    // Match CPU byte rounding explicitly: normalized storage conversion differs
    // between GL and Vulkan at half-byte boundaries (for example 127.5).
    let rgba = clamp(blend(textureLoad(canvas, coord, 0), textureLoad(layer, coord, 0)), vec4<f32>(0.0), vec4<f32>(1.0));
    textureStore(output, coord, floor(rgba * 255.0 + vec4<f32>(0.5)) / 255.0);
}
