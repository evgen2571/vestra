// Evaluated linear Spectrum2D source rasterizer. The shader only draws the
// authored source colour and already-evaluated bands; ordinary effects run
// after source generation in the shared effect chain.
struct Params {
    canvas_width: u32, canvas_height: u32, band_count: u32, colour_alpha: u32,
    region: vec4<f32>,
    style: vec4<f32>,
    bands: array<vec4<f32>, 12>,
};

@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> params: Params;

fn band(index: u32) -> f32 {
    return params.bands[index / 4u][index % 4u];
}

fn authored_colour() -> vec4<f32> {
    return vec4<f32>(params.style.yzw / 255.0, f32(params.colour_alpha) / 255.0);
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let coord = vec2<i32>(id.xy);
    let point = vec2<f32>(f32(id.x) + 0.5, f32(id.y) + 0.5);
    let frame = vec2<f32>(f32(params.canvas_width), f32(params.canvas_height));
    let region_left = params.region.x * frame.x;
    let region_top = params.region.y * frame.y;
    let region_width = params.region.z * frame.x;
    let region_height = params.region.w * frame.y;
    let bottom = region_top + region_height;
    var result = vec4<f32>(0.0);

    if (params.band_count > 0u && point.x >= region_left && point.x < region_left + region_width
        && point.y >= region_top && point.y < bottom) {
        let cell_width = region_width / f32(params.band_count);
        let cell = min(u32(floor((point.x - region_left) / cell_width)), params.band_count - 1u);
        let bar_width = cell_width * (1.0 - params.style.x);
        let bar_left = region_left + f32(cell) * cell_width + (cell_width - bar_width) * 0.5;
        let bar_right = bar_left + bar_width;
        let amplitude = clamp(band(cell), 0.0, 1.0);
        let bar_top = bottom - region_height * amplitude;
        if (amplitude > 0.0 && point.x >= bar_left && point.x < bar_right
            && point.y >= bar_top && point.y < bottom) {
            result = authored_colour();
        }
    }
    textureStore(output, coord, result);
}
