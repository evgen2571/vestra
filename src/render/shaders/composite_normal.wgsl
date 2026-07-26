// Normal source-over composition for encoded straight-alpha RGBA8 textures.
struct Params { canvas_width: u32, canvas_height: u32, _unused0: u32, _unused1: u32, };
@group(0) @binding(0) var canvas: texture_2d<f32>;
@group(0) @binding(1) var layer: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;
fn source_over(destination: vec4<f32>, source: vec4<f32>) -> vec4<f32> {
    let alpha = source.a + destination.a * (1.0 - source.a);
    if (alpha <= 0.0) { return vec4<f32>(0.0); }
    return vec4<f32>((source.rgb * source.a + destination.rgb * destination.a * (1.0 - source.a)) / alpha, alpha);
}
@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coord = vec2<i32>(id.xy);
    let result = source_over(textureLoad(canvas, coord, 0), textureLoad(layer, coord, 0));
    textureStore(output, coord, clamp(result, vec4<f32>(0.0), vec4<f32>(1.0)));
}
