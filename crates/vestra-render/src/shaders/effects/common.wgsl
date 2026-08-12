@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var auxiliary: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;

fn round_byte(pixel: vec4<f32>) -> vec4<f32> {
    return round(clamp(pixel, vec4<f32>(0.0), vec4<f32>(1.0)) * 255.0) / 255.0;
}

fn load_edge(texture: texture_2d<f32>, coordinate: vec2<i32>) -> vec4<f32> {
    let dimensions = textureDimensions(texture);
    return textureLoad(texture, clamp(coordinate, vec2<i32>(0), vec2<i32>(dimensions) - vec2<i32>(1)), 0);
}

fn bilinear_edge(texture: texture_2d<f32>, position: vec2<f32>) -> vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(texture));
    let adjusted = clamp(position - vec2<f32>(0.5), vec2<f32>(0.0), dimensions - vec2<f32>(1.0));
    let base = vec2<i32>(floor(adjusted));
    let fraction = adjusted - vec2<f32>(base);
    let a = load_edge(texture, base);
    let b = load_edge(texture, base + vec2<i32>(1, 0));
    let c = load_edge(texture, base + vec2<i32>(0, 1));
    let d = load_edge(texture, base + vec2<i32>(1, 1));
    let mixed = mix(
        mix(vec4<f32>(a.rgb * a.a, a.a), vec4<f32>(b.rgb * b.a, b.a), fraction.x),
        mix(vec4<f32>(c.rgb * c.a, c.a), vec4<f32>(d.rgb * d.a, d.a), fraction.x),
        fraction.y,
    );
    if (mixed.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(mixed.rgb / mixed.a, mixed.a));
}
