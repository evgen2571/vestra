@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let pixel = textureLoad(source, vec2<i32>(id.xy), 0);
    if (pixel.a == 0.0) { textureStore(output, vec2<i32>(id.xy), pixel); return; }
    let position = luminance_key(pixel.rgb) * (params.count - 1u);
    var colour: vec3<u32>;
    if (params.nearest != 0u) {
        colour = palette_bytes((position + 32640u) / 65280u);
    } else {
        let lower = position / 65280u;
        let upper = min(lower + 1u, params.count - 1u);
        let fraction = position % 65280u;
        colour = (palette_bytes(lower) * (65280u - fraction) + palette_bytes(upper) * fraction + vec3<u32>(32640u)) / 65280u;
    }
    textureStore(output, vec2<i32>(id.xy), mix_palette(pixel, colour));
}
