// One workgroup invocation composes one destination pixel. The output buffer
// stores straight-alpha RGBA bytes packed as 0xAABBGGRR on little-endian hosts,
// which is also the byte layout expected by Rgba8Unorm texture copies.

struct Params {
    canvas_width: u32,
    canvas_height: u32,
    output_stride_pixels: u32,
    mode: u32, // 0 clear, 1 image, 2 solid
    source_width: u32,
    source_height: u32,
    source_origin_x: u32,
    source_origin_y: u32,
    crop: vec4<f32>,
    effective_size: vec2<f32>,
    opacity: f32,
    _pad1: f32,
    inverse_row0: vec3<f32>,
    _pad2: f32,
    inverse_row1: vec3<f32>,
    _pad3: f32,
    colour_row0: vec3<f32>,
    _pad4: f32,
    colour_row1: vec3<f32>,
    _pad5: f32,
    colour_row2: vec3<f32>,
    _pad6: f32,
    colour_offset: vec3<f32>,
    _pad7: f32,
    solid_or_background: vec4<f32>,
};

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<u32>;
@group(0) @binding(2) var<uniform> params: Params;

fn unpack(value: u32) -> vec4<f32> {
    return vec4<f32>(f32(value & 255u), f32((value >> 8u) & 255u), f32((value >> 16u) & 255u), f32((value >> 24u) & 255u));
}

fn pack(value: vec4<f32>) -> u32 {
    let bytes = vec4<u32>(round(clamp(value, vec4<f32>(0.0), vec4<f32>(255.0))));
    return bytes.x | (bytes.y << 8u) | (bytes.z << 16u) | (bytes.w << 24u);
}

fn texel(coord: vec2<i32>) -> vec4<f32> {
    if (coord.x < 0 || coord.y < 0 || coord.x >= i32(params.source_width) || coord.y >= i32(params.source_height)) {
        return vec4<f32>(0.0);
    }
    return textureLoad(source, coord + vec2<i32>(i32(params.source_origin_x), i32(params.source_origin_y)), 0) * 255.0;
}

fn bilinear(position: vec2<f32>) -> vec4<f32> {
    let adjusted = position - vec2<f32>(0.5);
    let base = vec2<i32>(floor(adjusted));
    let fraction = adjusted - vec2<f32>(base);
    let top = mix(texel(base), texel(base + vec2<i32>(1, 0)), fraction.x);
    let bottom = mix(texel(base + vec2<i32>(0, 1)), texel(base + vec2<i32>(1, 1)), fraction.x);
    return round(mix(top, bottom, fraction.y));
}

fn transformed_colour(pixel: vec4<f32>) -> vec4<f32> {
    let rgb = pixel.rgb;
    return vec4<f32>(round(clamp(vec3<f32>(dot(params.colour_row0, rgb), dot(params.colour_row1, rgb), dot(params.colour_row2, rgb)) + params.colour_offset, vec3<f32>(0.0), vec3<f32>(255.0))), pixel.a);
}

fn source_over(destination: vec4<f32>, source_pixel: vec4<f32>) -> vec4<f32> {
    let source_alpha = source_pixel.a / 255.0 * params.opacity;
    let destination_alpha = destination.a / 255.0;
    let alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if (alpha <= 0.0) { return vec4<f32>(0.0); }
    let rgb = round(clamp((source_pixel.rgb * source_alpha + destination.rgb * destination_alpha * (1.0 - source_alpha)) / alpha, vec3<f32>(0.0), vec3<f32>(255.0)));
    return vec4<f32>(rgb, round(clamp(alpha * 255.0, 0.0, 255.0)));
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let index = id.y * params.output_stride_pixels + id.x;
    if (params.mode == 0u) { output[index] = pack(params.solid_or_background); return; }
    let destination = unpack(output[index]);
    if (params.mode == 2u) { output[index] = pack(source_over(destination, transformed_colour(params.solid_or_background))); return; }
    let point = vec2<f32>(f32(id.x) + 0.5, f32(id.y) + 0.5);
    let mapped = vec2<f32>(dot(params.inverse_row0, vec3<f32>(point, 1.0)), dot(params.inverse_row1, vec3<f32>(point, 1.0)));
    if (mapped.x < 0.0 || mapped.y < 0.0 || mapped.x >= params.effective_size.x || mapped.y >= params.effective_size.y) { return; }
    let source_position = vec2<f32>(params.crop.x * f32(params.source_width), params.crop.y * f32(params.source_height)) + mapped / params.effective_size * vec2<f32>(params.crop.z * f32(params.source_width), params.crop.w * f32(params.source_height));
    output[index] = pack(source_over(destination, transformed_colour(bilinear(source_position))));
}
