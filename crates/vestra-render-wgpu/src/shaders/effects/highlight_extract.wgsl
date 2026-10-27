@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let pixel = textureLoad(source, vec2<i32>(id.xy), 0); let luminance = dot(pixel.rgb, vec3<f32>(0.2126, 0.7152, 0.0722)); let highlight = clamp((luminance - params.threshold) / max(1.0 - params.threshold, 0.0001), 0.0, 1.0);
    textureStore(output, vec2<i32>(id.xy), round_byte(vec4<f32>(params.colour.rgb / 255.0, pixel.a * highlight * params.colour.a / 255.0)));
}
struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    threshold: f32, _padding1: f32, _padding2: f32, _padding3: f32,
    colour: vec4<f32>,
};
@group(0) @binding(3) var<uniform> params: Params;
