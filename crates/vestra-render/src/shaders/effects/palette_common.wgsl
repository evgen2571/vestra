struct Params {
    canvas_width: u32, canvas_height: u32, _padding: vec2<u32>,
    amount: u32, strength: f32, count: u32, nearest: u32,
    bits: u32, scale: u32, _padding1: vec2<u32>,
    colours: array<vec4<u32>, 4>,
};
@group(0) @binding(3) var<uniform> params: Params;

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
