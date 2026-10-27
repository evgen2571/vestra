@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coordinate = vec2<i32>(id.xy); let overlay = textureLoad(source, coordinate, 0); let base = textureLoad(auxiliary, coordinate, 0); var result = base;
    if (params.mode == 0u) { let glow_alpha = clamp(overlay.a * params.amount, 0.0, 1.0); let alpha = base.a + glow_alpha * (1.0 - base.a); result = select(vec4<f32>(0.0), vec4<f32>((base.rgb * base.a + overlay.rgb * glow_alpha) / alpha, alpha), alpha > 0.0000001); }
    else {
        // Subtract byte values so normalized float cancellation cannot move half-byte ties down.
        let rgb = round(base.rgb * 255.0);
        let blurred = round(overlay.rgb * 255.0);
        let adjusted = floor(clamp(rgb + (rgb - blurred) * params.amount, vec3<f32>(0.0), vec3<f32>(255.0)) + vec3<f32>(0.5));
        result = vec4<f32>(adjusted / 255.0, base.a);
    }
    textureStore(output, coordinate, round_byte(result));
}
struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    mode: u32, amount: f32, _padding1: vec2<u32>,
};
@group(0) @binding(3) var<uniform> params: Params;
