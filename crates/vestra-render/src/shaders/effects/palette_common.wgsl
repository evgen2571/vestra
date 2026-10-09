struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    amount: u32, strength: f32, count: u32, mode: u32,
    bits: u32, scale: u32, seed: u32, levels: u32,
    colours: array<vec4<u32>, 4>,
    features: array<vec4<u32>, 4>,
    stops: array<vec4<u32>, 4>,
};
@group(0) @binding(3) var<uniform> params: Params;

fn tonal_stop(index: u32) -> u32 { return params.stops[index / 4u][index % 4u]; }

// Lower index, distance from the lower stop, interval width.
fn tonal_interval(key: u32) -> vec3<u32> {
    var lower = 0u;
    while (lower + 2u < params.count && key >= tonal_stop(lower + 1u)) { lower += 1u; }
    return vec3<u32>(lower, key - tonal_stop(lower), tonal_stop(lower + 1u) - tonal_stop(lower));
}

fn channel_quantize(rgb: vec3<u32>, threshold: u32, strength: u32) -> vec3<u32> {
    let last = params.levels - 1u;
    let position = rgb * last;
    let lower = position / 255u;
    let fraction = position % 255u;
    let nearest = select(vec3<u32>(0u), vec3<u32>(4096u), fraction >= vec3<u32>(128u));
    let probability = ((fraction * 4096u + vec3<u32>(127u)) / 255u * strength
        + nearest * (4096u - strength) + vec3<u32>(2048u)) / 4096u;
    let index = min(lower + select(vec3<u32>(0u), vec3<u32>(1u), vec3<u32>(threshold) < probability), vec3<u32>(last));
    return (index * 255u + vec3<u32>(last / 2u)) / last;
}

fn palette_bytes(index: u32) -> vec3<u32> {
    let packed = params.colours[index / 4u][index % 4u];
    return vec3<u32>(packed & 255u, (packed >> 8u) & 255u, (packed >> 16u) & 255u);
}

fn luminance_key(pixel: vec3<f32>) -> u32 {
    let bytes = vec3<u32>(round(pixel * 255.0));
    return 54u * bytes.r + 183u * bytes.g + 19u * bytes.b;
}

fn mix_palette(pixel: vec4<f32>, colour: vec3<u32>) -> vec4<f32> {
    let input = vec3<u32>(round(pixel.rgb * 255.0));
    let mixed = (input * (65535u - params.amount) + colour * params.amount + vec3<u32>(32767u)) / 65535u;
    return vec4<f32>(vec3<f32>(mixed) / 255.0, pixel.a);
}

fn chromatic_features(rgb: vec3<u32>) -> vec3<u32> {
    if (params.mode == 5u) { return oklab_features(rgb); }
    if (params.mode != 4u) { return rgb; }
    let c = vec3<i32>(rgb);
    let maximum = max(max(c.r, c.g), c.b);
    let minimum = min(min(c.r, c.g), c.b);
    let span = maximum - minimum;
    if (span == 0) { return vec3<u32>(0u, 0u, u32(maximum)); }
    var hue: i32;
    if (maximum == c.r) { hue = (c.g - c.b) * 255 / span; }
    else if (maximum == c.g) { hue = 510 + (c.b - c.r) * 255 / span; }
    else { hue = 1020 + (c.r - c.g) * 255 / span; }
    return vec3<u32>(u32((hue + 1530) % 1530), u32(span * 255 / maximum), u32(maximum));
}

fn cube_root_q10(value: u32) -> u32 {
    let scaled = value * 16384u;
    var lower = 0u;
    var upper = 1025u;
    while (lower + 1u < upper) {
        let middle = (lower + upper) / 2u;
        if (middle * middle * middle <= scaled) { lower = middle; }
        else { upper = middle; }
    }
    let halfway = lower * lower * lower + (12u * lower * lower + 6u * lower + 1u) / 8u;
    return min(lower + select(0u, 1u, scaled > halfway), 1024u);
}

fn signed_q15_round(value: i32) -> i32 {
    let magnitude = (abs(value) + 16384) / 32768;
    return select(magnitude, -magnitude, value < 0);
}

fn oklab_features(rgb: vec3<u32>) -> vec3<u32> {
    let linear = vec3<u32>(SRGB_LINEAR[rgb.r], SRGB_LINEAR[rgb.g], SRGB_LINEAR[rgb.b]);
    let l = linear * RGB_TO_LMS_0;
    let m = linear * RGB_TO_LMS_1;
    let s = linear * RGB_TO_LMS_2;
    let roots = vec3<i32>(i32(cube_root_q10((l.x + l.y + l.z + 16384u) / 32768u)),
        i32(cube_root_q10((m.x + m.y + m.z + 16384u) / 32768u)),
        i32(cube_root_q10((s.x + s.y + s.z + 16384u) / 32768u)));
    let lightness = roots * LMS_TO_LAB_0;
    let a = roots * LMS_TO_LAB_1;
    let b = roots * LMS_TO_LAB_2;
    return vec3<u32>(u32(signed_q15_round(lightness.x + lightness.y + lightness.z)),
        u32(signed_q15_round(a.x + a.y + a.z) + 512),
        u32(signed_q15_round(b.x + b.y + b.z) + 512));
}

fn chromatic_distance(a: vec3<u32>, b: vec3<u32>) -> u32 {
    var delta = max(a, b) - min(a, b);
    if (params.mode == 4u) {
        delta.x = min(delta.x, 1530u - delta.x) / 3u * min(a.y, b.y) / 255u;
    }
    return delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
}

fn chromatic_index(rgb: vec3<u32>, threshold: u32, strength: u32) -> u32 {
    let input = chromatic_features(rgb);
    var indices = vec2<u32>(0u);
    var distances = vec2<u32>(0xffffffffu);
    for (var i = 0u; i < params.count; i += 1u) {
        if (params.mode == 5u && all(rgb == palette_bytes(i))) { return i; }
        let packed = params.features[i / 4u][i % 4u];
        let feature = vec3<u32>(packed & 2047u, (packed >> 11u) & 1023u, (packed >> 21u) & 1023u);
        let distance = chromatic_distance(input, feature);
        if (distance < distances.x) {
            distances.y = distances.x; indices.y = indices.x;
            distances.x = distance; indices.x = i;
        } else if (distance < distances.y) {
            distances.y = distance; indices.y = i;
        }
    }
    let sum = distances.x + distances.y;
    var probability = 0u;
    if (sum != 0u) {
        if (params.mode == 5u) {
            // Binary long division avoids overflowing the Q10-distance numerator.
            var remainder = distances.x;
            for (var bit = 0u; bit < 12u; bit += 1u) {
                remainder *= 2u;
                probability *= 2u;
                if (remainder >= sum) { remainder -= sum; probability += 1u; }
            }
            if (remainder * 2u >= sum) { probability += 1u; }
        } else { probability = (distances.x * 4096u + sum / 2u) / sum; }
    }
    return select(indices.x, indices.y, threshold < (probability * strength + 2048u) / 4096u);
}
