fn bayer_rank(x: u32, y: u32) -> u32 {
    var rank = 0u;
    for (var bit = 0u; bit < params.bits; bit += 1u) {
        let xb = (x >> bit) & 1u;
        let yb = (y >> bit) & 1u;
        rank = 4u * rank + ((xb ^ yb) << 1u) + yb;
    }
    return rank;
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let pixel = textureLoad(source, vec2<i32>(id.xy), 0);
    if (pixel.a == 0.0) { textureStore(output, vec2<i32>(id.xy), pixel); return; }
    let threshold = (f32(bayer_rank(id.x / params.scale, id.y / params.scale)) + 0.5) / f32(1u << (2u * params.bits));
    let last = params.count - 1u;
    let position = f32(luminance_key(pixel.rgb)) / 65280.0 * f32(last) + 0.5 + params.strength * (threshold - 0.5);
    let selected = u32(clamp(floor(position), 0.0, f32(last)));
    textureStore(output, vec2<i32>(id.xy), mix_palette(pixel, palette_bytes(selected)));
}
