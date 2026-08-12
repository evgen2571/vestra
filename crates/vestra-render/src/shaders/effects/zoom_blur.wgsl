struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    radius: f32, samples: u32, anchor: vec2<f32>,
    direction: u32, _padding1: u32, _padding2: u32, _padding3: u32,
};
@group(0) @binding(3) var<uniform> params: Params;

fn zoom_blur(coord: vec2<i32>) -> vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(source));
    let amount = clamp(params.radius / max(dimensions.x, dimensions.y), 0.0, 0.5);
    let samples = max(i32(params.samples), 1);
    let centre = params.anchor * (dimensions - vec2<f32>(1.0));
    let ray = vec2<f32>(coord) - centre;
    var accumulated = vec4<f32>(0.0);
    for (var index = 0; index < 64; index = index + 1) {
        if (index < samples) {
            let unit = f32(index) / f32(max(samples - 1, 1));
            var exposure = unit * 2.0 - 1.0;
            if (params.direction == 1u) { exposure = -unit; }
            if (params.direction == 2u) { exposure = unit; }
            let pixel = bilinear_edge(source, centre + ray * (1.0 + exposure * amount) + vec2<f32>(0.5));
            accumulated += vec4<f32>(pixel.rgb * pixel.a, pixel.a);
        }
    }
    let result = accumulated / f32(samples);
    if (result.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(result.rgb / result.a, result.a));
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    textureStore(output, vec2<i32>(id.xy), zoom_blur(vec2<i32>(id.xy)));
}
