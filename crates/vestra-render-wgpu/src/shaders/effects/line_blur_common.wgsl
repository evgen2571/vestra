struct Params {
    canvas_width: u32,
    canvas_height: u32,
    _padding: vec2<u32>,
    radius: f32,
    angle: f32,
    samples: u32,
    _padding1: u32,
};
@group(0) @binding(3) var<uniform> params: Params;

fn line_blur(coord: vec2<i32>) -> vec4<f32> {
    let radius = clamp(params.radius, 0.0, 32.0);
    let samples = max(i32(params.samples), 1);
    let direction = vec2<f32>(cos(params.angle), sin(params.angle));
    var accumulated = vec4<f32>(0.0);
    for (var index = 0; index < 33; index = index + 1) {
        if (index < samples) {
            let offset = (f32(index) / f32(max(samples - 1, 1)) - 0.5) * 2.0 * radius;
            let pixel = bilinear_edge(source, vec2<f32>(coord) + direction * offset + vec2<f32>(0.5));
            accumulated += vec4<f32>(pixel.rgb * pixel.a, pixel.a);
        }
    }
    let result = accumulated / f32(samples);
    if (result.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(result.rgb / result.a, result.a));
}
