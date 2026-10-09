fn threshold_rank(x: u32, y: u32) -> u32 {
    if (params.bits != 5u) { return bayer_rank(x, y); }
    let seed = params.seed ^ (params.seed >> 13u) ^ (params.seed >> 26u);
    var p = vec2<u32>((x + (seed & 31u)) & 31u, (y + ((seed >> 5u) & 31u)) & 31u);
    if ((seed & 1024u) != 0u) { p = p.yx; }
    if ((seed & 2048u) != 0u) { p.x = 31u - p.x; }
    if ((seed & 4096u) != 0u) { p.y = 31u - p.y; }
    return BLUE_NOISE[p.y * 32u + p.x];
}

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
    var base = pixel;
    if ((params._padding.y & 2u) != 0u) { base = textureLoad(auxiliary, vec2<i32>(id.xy), 0); }
    if (base.a == 0.0) { textureStore(output, vec2<i32>(id.xy), base); return; }
    let threshold = (f32(threshold_rank(id.x / params.scale, id.y / params.scale)) + 0.5) / f32(1u << (2u * params.bits));
    let last = params.count - 1u;
    let position = f32(luminance_key(pixel.rgb)) / 65280.0 * f32(last) + 0.5 + params.strength * (threshold - 0.5);
    var selected = u32(clamp(floor(position), 0.0, f32(last)));
    if (params.mode >= 3u) {
        selected = chromatic_index(vec3<u32>(round(pixel.rgb * 255.0)), u32(threshold * 4096.0), u32(round(params.strength * 4096.0)));
    } else if (params._padding.x != 0u) {
        let interval = tonal_interval(luminance_key(pixel.rgb));
        let strength = u32(round(params.strength * 4096.0));
        let nearest = select(0u, 4096u, interval.y * 2u >= interval.z);
        let probability = ((interval.y * 4096u + interval.z / 2u) / interval.z * strength
            + nearest * (4096u - strength) + 2048u) / 4096u;
        selected = interval.x + select(0u, 1u, u32(threshold * 4096.0) >= 4096u - probability);
    }
    var colour = palette_bytes(selected);
    if (params.mode == 2u) {
        colour = channel_quantize(vec3<u32>(round(pixel.rgb * 255.0)), u32(threshold * 4096.0), u32(round(params.strength * 4096.0)));
    }
    textureStore(output, vec2<i32>(id.xy), mix_palette(base, colour));
}
