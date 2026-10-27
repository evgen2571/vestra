@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let pixel = textureLoad(source, vec2<i32>(id.xy), 0);
    let scale = 1.0 / max(params.white_point - params.black_point, 0.0001);
    var value = clamp((round(pixel.rgb * 255.0) * exp2(params.exposure) - vec3<f32>(params.black_point * 255.0)) * scale, vec3<f32>(0.0), vec3<f32>(255.0));
    if (params.gamma != 1.0) { value = pow(value / 255.0, vec3<f32>(1.0 / max(params.gamma, 0.001))) * 255.0; }
    // WGSL round uses even ties; Rust byte conversion uses upward ties.
    textureStore(output, vec2<i32>(id.xy), vec4<f32>(floor(value + vec3<f32>(0.5)) / 255.0, pixel.a));
}
struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    exposure: f32, gamma: f32, black_point: f32, white_point: f32,
};
@group(0) @binding(3) var<uniform> params: Params;
