// Ordered instanced geometry for the renderer-independent particle instances.
// Position is normalized top-left canvas space; size is normalized against the
// minimum canvas dimension, matching the CPU rasterizer.
struct Params {
    canvas_width: u32,
    canvas_height: u32,
    primitive: u32,
    _padding: u32,
};
@group(0) @binding(0) var<uniform> params: Params;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) centre: vec2<f32>,
    @location(1) @interpolate(flat) half_size: f32,
    @location(2) @interpolate(flat) rotation: vec2<f32>,
    @location(3) @interpolate(flat) colour: vec4<f32>,
};

fn quad_vertex(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(-1.0, -1.0); }
        case 1u: { return vec2<f32>(1.0, -1.0); }
        case 2u: { return vec2<f32>(1.0, 1.0); }
        case 3u: { return vec2<f32>(-1.0, -1.0); }
        case 4u: { return vec2<f32>(1.0, 1.0); }
        default: { return vec2<f32>(-1.0, 1.0); }
    }
}

@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    @location(2) position_size_rotation: vec4<f32>,
    @location(3) colour: vec4<f32>,
) -> VertexOutput {
    let canvas_size = vec2<f32>(f32(params.canvas_width), f32(params.canvas_height));
    let centre = position_size_rotation.xy;
    let half_size = position_size_rotation.z * 0.5;
    var rotation = vec2<f32>(1.0, 0.0);
    if (params.primitive != 0u) {
        let angle = radians(position_size_rotation.w);
        rotation = vec2<f32>(cos(angle), sin(angle));
    }
    // Put the bounding quad on pixel edges so triangle edge rules cannot drop a pixel
    // centre that lands on the actual primitive boundary.
    let extent = half_size * (abs(rotation.x) + abs(rotation.y));
    let corner = quad_vertex(vertex_index);
    let border = centre + corner * extent;
    let pixel = clamp(select(floor(border), ceil(border), corner > vec2<f32>(0.0)), vec2<f32>(0.0), canvas_size);
    let clip = (pixel / canvas_size * 2.0 - 1.0) * vec2<f32>(1.0, -1.0);
    var output: VertexOutput;
    output.position = vec4<f32>(clip, 0.0, 1.0);
    output.centre = centre;
    output.half_size = half_size;
    output.rotation = rotation;
    output.colour = colour;
    return output;
}

@fragment
fn fragment(input: VertexOutput) -> @location(0) vec4<f32> {
    if (input.half_size <= 0.0) {
        discard;
    }
    let delta = (input.position.xy - input.centre) / input.half_size;
    if (params.primitive == 0u) {
        if (dot(delta, delta) > 1.0) {
            discard;
        }
    } else {
        let local = vec2<f32>(
            dot(delta, input.rotation),
            dot(delta, vec2<f32>(-input.rotation.y, input.rotation.x)),
        );
        if (any(abs(local) > vec2<f32>(1.0))) {
            discard;
        }
    }
    return input.colour;
}
