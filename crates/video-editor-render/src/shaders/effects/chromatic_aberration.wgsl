@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coordinate = vec2<i32>(id.xy); let pixel = textureLoad(source, coordinate, 0); let direction = vec2<f32>(cos(params.angle), sin(params.angle)) * params.amount;
    let left = bilinear_edge(source, vec2<f32>(coordinate) - direction + vec2<f32>(0.5)); let right = bilinear_edge(source, vec2<f32>(coordinate) + direction + vec2<f32>(0.5));
    textureStore(output, coordinate, round_byte(vec4<f32>(left.r, pixel.g, right.b, pixel.a)));
}
struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    amount: f32, angle: f32, _padding1: vec2<f32>,
};
@group(0) @binding(3) var<uniform> params: Params;
