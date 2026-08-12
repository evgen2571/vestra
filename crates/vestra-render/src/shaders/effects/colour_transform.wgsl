@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let pixel = textureLoad(source, vec2<i32>(id.xy), 0); let encoded = pixel.rgb * 255.0;
    let rgb = round(clamp(vec3<f32>(dot(params.colour_row0.xyz, encoded), dot(params.colour_row1.xyz, encoded), dot(params.colour_row2.xyz, encoded)) + params.colour_offset.xyz, vec3<f32>(0.0), vec3<f32>(255.0))) / 255.0;
    textureStore(output, vec2<i32>(id.xy), round_byte(vec4<f32>(rgb, pixel.a)));
}
struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    colour_row0: vec4<f32>, colour_row1: vec4<f32>, colour_row2: vec4<f32>, colour_offset: vec4<f32>,
};
@group(0) @binding(3) var<uniform> params: Params;
