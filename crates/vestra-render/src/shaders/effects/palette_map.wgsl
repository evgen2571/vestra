@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let pixel = textureLoad(source, vec2<i32>(id.xy), 0);
    if (pixel.a == 0.0) { textureStore(output, vec2<i32>(id.xy), pixel); return; }
    let position = luminance_key(pixel.rgb) * (params.count - 1u);
    var colour: vec3<u32>;
    if (params.mode == 2u) {
        colour = channel_quantize(vec3<u32>(round(pixel.rgb * 255.0)), 0u, 0u);
    } else if (params.mode >= 3u) {
        colour = palette_bytes(chromatic_index(vec3<u32>(round(pixel.rgb * 255.0)), 0u, 0u));
    } else if (params._padding.x != 0u) {
        let interval = tonal_interval(luminance_key(pixel.rgb));
        if (params.mode == 1u) {
            colour = palette_bytes(interval.x + select(0u, 1u, interval.y * 2u >= interval.z));
        } else {
            colour = (palette_bytes(interval.x) * (interval.z - interval.y)
                + palette_bytes(interval.x + 1u) * interval.y + vec3<u32>(interval.z / 2u)) / interval.z;
        }
    } else if (params.mode == 1u) {
        colour = palette_bytes((position + 32640u) / 65280u);
    } else {
        let lower = position / 65280u;
        let upper = min(lower + 1u, params.count - 1u);
        let fraction = position % 65280u;
        colour = (palette_bytes(lower) * (65280u - fraction) + palette_bytes(upper) * fraction + vec3<u32>(32640u)) / 65280u;
    }
    textureStore(output, vec2<i32>(id.xy), mix_palette(pixel, colour));
}
