// CPU-compatible encoded straight-alpha effect passes.  Textures are RGBA8
// UNORM; every write is rounded to the same byte grid used by the CPU path.
struct Params {
    canvas_width: u32, canvas_height: u32, kind: u32, _unused: u32,
    source_width: u32, source_height: u32, source_origin_x: u32, source_origin_y: u32,
    crop: vec4<f32>, effective: vec4<f32>,
    inverse_row0: vec4<f32>, inverse_row1: vec4<f32>,
    colour_row0: vec4<f32>, colour_row1: vec4<f32>, colour_row2: vec4<f32>,
    colour_offset: vec4<f32>, solid_or_background: vec4<f32>,
};
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var auxiliary: texture_2d<f32>;
@group(0) @binding(2) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(3) var<uniform> params: Params;

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
    // CPU bilinear sampling accumulates premultiplied RGB before unpremultiplying.
    let pa = vec4<f32>(a.rgb * a.a, a.a);
    let pb = vec4<f32>(b.rgb * b.a, b.a);
    let pc = vec4<f32>(c.rgb * c.a, c.a);
    let pd = vec4<f32>(d.rgb * d.a, d.a);
    let mixed = mix(mix(pa, pb, fraction.x), mix(pc, pd, fraction.x), fraction.y);
    if (mixed.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(mixed.rgb / mixed.a, mixed.a));
}
fn gaussian(coord: vec2<i32>, horizontal: bool) -> vec4<f32> {
    let radius = clamp(params.effective.x, 0.0, 32.0);
    let support = i32(ceil(max(radius, 1.0)));
    let sigma = max(radius / 3.0, 0.5);
    var total = 0.0;
    var accumulated = vec4<f32>(0.0);
    for (var offset = -32; offset <= 32; offset = offset + 1) {
        if (abs(offset) <= support) {
            let weight = exp(-0.5 * pow(f32(offset) / sigma, 2.0));
            let step = select(vec2<i32>(0, offset), vec2<i32>(offset, 0), horizontal);
            let pixel = load_edge(source, coord + step);
            accumulated += vec4<f32>(pixel.rgb * pixel.a, pixel.a) * weight;
            total += weight;
        }
    }
    let result = accumulated / total;
    if (result.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(result.rgb / result.a, result.a));
}
fn line_blur(coord: vec2<i32>) -> vec4<f32> {
    let radius = clamp(params.effective.x, 0.0, 32.0);
    let samples = max(i32(params.effective.z), 1);
    let direction = vec2<f32>(cos(params.effective.y), sin(params.effective.y));
    var accumulated = vec4<f32>(0.0);
    for (var index = 0; index < 33; index = index + 1) {
        if (index < samples) {
            let offset = (f32(index) / f32(max(samples - 1, 1)) - 0.5) * 2.0 * radius;
            let pixel = bilinear_edge(source, vec2<f32>(coord) + direction * offset + vec2<f32>(0.5));
            accumulated += vec4<f32>(pixel.rgb * pixel.a, pixel.a);
        }
    }
    let result = accumulated / f32(samples);
    if (result.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(result.rgb / result.a, result.a));
}
fn zoom_blur(coord: vec2<i32>) -> vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(source));
    let amount = clamp(params.effective.x / max(dimensions.x, dimensions.y), 0.0, 0.5);
    let samples = max(i32(params.effective.y), 1);
    let centre = params.effective.zw * (dimensions - vec2<f32>(1.0));
    let ray = vec2<f32>(coord) - centre;
    var accumulated = vec4<f32>(0.0);
    for (var index = 0; index < 64; index = index + 1) {
        if (index < samples) {
            let unit = f32(index) / f32(max(samples - 1, 1));
            var exposure = unit * 2.0 - 1.0;
            if (params.solid_or_background.x == 1.0) { exposure = -unit; }
            if (params.solid_or_background.x == 2.0) { exposure = unit; }
            let pixel = bilinear_edge(source, centre + ray * (1.0 + exposure * amount) + vec2<f32>(0.5));
            accumulated += vec4<f32>(pixel.rgb * pixel.a, pixel.a);
        }
    }
    let result = accumulated / f32(samples);
    if (result.a <= 0.0000001) { return vec4<f32>(0.0); }
    return round_byte(vec4<f32>(result.rgb / result.a, result.a));
}
@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coord = vec2<i32>(id.xy);
    let pixel = textureLoad(source, coord, 0);
    var result = pixel;
    if (params.kind == 1u) {
        let encoded = pixel.rgb * 255.0;
        result = vec4<f32>((round(clamp(vec3<f32>(dot(params.colour_row0.xyz, encoded), dot(params.colour_row1.xyz, encoded), dot(params.colour_row2.xyz, encoded)) + params.colour_offset.xyz, vec3<f32>(0.0), vec3<f32>(255.0))) / 255.0), pixel.a);
    } else if (params.kind == 2u) { result = gaussian(coord, true); }
    else if (params.kind == 3u) { result = gaussian(coord, false); }
    else if (params.kind == 4u) {
        let luminance = dot(pixel.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        let highlight = clamp((luminance - params.effective.x) / max(1.0 - params.effective.x, 0.0001), 0.0, 1.0);
        result = vec4<f32>(params.solid_or_background.rgb / 255.0, pixel.a * highlight * params.solid_or_background.a / 255.0);
    } else if (params.kind == 5u) {
        let base = textureLoad(auxiliary, coord, 0); let glow_alpha = clamp(pixel.a * params.effective.x, 0.0, 1.0); let alpha = base.a + glow_alpha * (1.0 - base.a);
        result = select(vec4<f32>(0.0), vec4<f32>((base.rgb * base.a + pixel.rgb * glow_alpha) / alpha, alpha), alpha > 0.0000001);
    } else if (params.kind == 6u) {
        let base = textureLoad(auxiliary, coord, 0); result = vec4<f32>(base.rgb + (base.rgb - pixel.rgb) * params.effective.x, base.a);
    } else if (params.kind == 7u || params.kind == 12u) { result = line_blur(coord); }
    else if (params.kind == 8u) { result = zoom_blur(coord); }
    else if (params.kind == 9u) {
        let direction = vec2<f32>(cos(params.effective.y), sin(params.effective.y)) * params.effective.x;
        let left = bilinear_edge(source, vec2<f32>(coord) - direction + vec2<f32>(0.5)); let right = bilinear_edge(source, vec2<f32>(coord) + direction + vec2<f32>(0.5));
        result = vec4<f32>(left.r, pixel.g, right.b, pixel.a);
    } else if (params.kind == 10u) {
        let dimensions = vec2<f32>(textureDimensions(source)); let d = (vec2<f32>(coord) + vec2<f32>(0.5) - dimensions / 2.0) / (dimensions / 2.0);
        let edge = clamp((length(d) - params.effective.y) / max(params.effective.z, 0.001), 0.0, 1.0); let mix_amount = clamp(params.effective.x * edge, 0.0, 1.0);
        result = vec4<f32>(mix(pixel.rgb, params.solid_or_background.rgb / 255.0, mix_amount), pixel.a);
    } else if (params.kind == 11u) {
        let scale = 1.0 / max(params.effective.w - params.effective.z, 0.0001); let value = clamp((pixel.rgb * exp2(params.effective.x) - vec3<f32>(params.effective.z)) * scale, vec3<f32>(0.0), vec3<f32>(1.0));
        result = vec4<f32>(round(pow(value, vec3<f32>(1.0 / max(params.effective.y, 0.001))) * 255.0) / 255.0, pixel.a);
    }
    textureStore(output, coord, round_byte(result));
}
