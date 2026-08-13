// Ordered instanced geometry for the renderer-independent particle instances.
// Position is normalized top-left canvas space; size is normalized against the
// minimum canvas dimension, matching the CPU rasterizer.
struct Params {
    canvas_width: u32,
    canvas_height: u32,
    primitive: u32,
    _padding: u32,
};
struct Instance {
    position_size_rotation_opacity: vec4<f32>,
    colour: vec4<f32>,
};
@group(0) @binding(0) var<uniform> params: Params;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) colour: vec4<f32>,
};

fn quad_vertex(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(-0.5, -0.5); }
        case 1u: { return vec2<f32>(0.5, -0.5); }
        case 2u: { return vec2<f32>(0.5, 0.5); }
        case 3u: { return vec2<f32>(-0.5, -0.5); }
        case 4u: { return vec2<f32>(0.5, 0.5); }
        default: { return vec2<f32>(-0.5, 0.5); }
    }
}

@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    @location(2) position_size_rotation_opacity: vec4<f32>,
    @location(3) colour: vec4<f32>,
) -> VertexOutput {
    let local = quad_vertex(vertex_index);
    let angle = radians(position_size_rotation_opacity.w);
    let rotation = mat2x2<f32>(
        vec2<f32>(cos(angle), sin(angle)),
        vec2<f32>(-sin(angle), cos(angle)),
    );
    let pixel_size = position_size_rotation_opacity.z * f32(min(params.canvas_width, params.canvas_height));
    let offset_pixels = rotation * (local * pixel_size);
    let centre = position_size_rotation_opacity.xy;
    let pixel = centre * vec2<f32>(f32(params.canvas_width), f32(params.canvas_height)) + offset_pixels;
    let clip = vec2<f32>(pixel.x / f32(params.canvas_width) * 2.0 - 1.0, 1.0 - pixel.y / f32(params.canvas_height) * 2.0);
    var output: VertexOutput;
    output.position = vec4<f32>(clip, 0.0, 1.0);
    output.local = rotation * local;
    output.colour = colour;
    return output;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    if (params.primitive == 0u && dot(input.local, input.local) > 0.25) {
        discard;
    }
    return input.colour;
}
