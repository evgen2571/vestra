@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coordinate = vec2<i32>(id.xy); let pixel = textureLoad(source, coordinate, 0); let dimensions = vec2<f32>(textureDimensions(source)); let d = (vec2<f32>(coordinate) + vec2<f32>(0.5) - dimensions / 2.0) / (dimensions / 2.0);
    let edge = clamp((length(d) - params.radius) / max(params.softness, 0.001), 0.0, 1.0); let mix_amount = clamp(params.amount * edge, 0.0, 1.0);
    textureStore(output, coordinate, round_byte(vec4<f32>(mix(pixel.rgb, params.colour.rgb / 255.0, mix_amount), pixel.a)));
}
struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    amount: f32, radius: f32, softness: f32, _padding1: f32, colour: vec4<f32>,
};
@group(0) @binding(3) var<uniform> params: Params;
