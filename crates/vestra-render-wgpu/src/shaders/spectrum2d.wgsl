struct Params {
    canvas_width: u32, canvas_height: u32, band_count: u32, flags: u32,
    region: vec4<f32>, style: vec4<f32>, extra: vec4<u32>,
    bands: array<vec4<f32>, 12>,
};
@group(0) @binding(0) var output: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(1) var<uniform> params: Params;
fn band(index: u32) -> f32 { return params.bands[index / 4u][index % 4u]; }
fn unpack(value: u32) -> vec4<f32> { return vec4<f32>(f32(value & 255u), f32((value >> 8u) & 255u), f32((value >> 16u) & 255u), f32((value >> 24u) & 255u)) / 255.0; }
fn colour(t: f32) -> vec4<f32> { return mix(unpack(params.extra.x), unpack(params.extra.y), clamp(t, 0.0, 1.0)); }
fn positive_mod(value: f32, modulus: f32) -> f32 { return value - floor(value / modulus) * modulus; }
@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    let point = vec2<f32>(f32(id.x) + 0.5, f32(id.y) + 0.5);
    let frame = vec2<f32>(f32(params.canvas_width), f32(params.canvas_height));
    let left = params.region.x * frame.x; let top = params.region.y * frame.y;
    let size = params.region.zw * frame; let bottom = top + size.y;
    var result = vec4<f32>(0.0); let is_radial = (params.flags & 64u) != 0u;
    if (is_radial) {
        let center = vec2<f32>(left + size.x * 0.5, top + size.y * 0.5); let delta = point - center;
        let radius = length(delta); let outer = min(size.x, size.y) * 0.5; let inner = outer * bitcast<f32>(params.extra.z);
        let angle = positive_mod(atan2(delta.x, -delta.y) - params.style.z, 6.28318530718); let sweep = params.style.w;
        let cell = sweep / f32(params.band_count); let visual = min(u32(floor(angle / cell)), params.band_count - 1u);
        let reverse = ((params.flags >> 2u) & 3u) == 1u; var index = select(visual, params.band_count - 1u - visual, reverse);
        let a = mix(params.style.y, 1.0, clamp(band(index), 0.0, 1.0)); let span = outer - inner; let direction = params.flags & 3u;
        let lo = select(select(inner, outer - a * span, direction == 1u), inner + (1.0-a) * span * 0.5, direction == 2u); let hi = select(select(inner + a * span, outer, direction == 1u), outer - (1.0-a) * span * 0.5, direction == 2u);
        let within = angle - f32(visual) * cell;
        if (a > 0.0 && angle < sweep && within >= cell * params.style.x * 0.5 && within < cell * (1.0 - params.style.x * 0.5) && radius >= lo && radius < hi) { let radial_t = select(select((radius-inner)/span, (outer-radius)/span, direction == 1u), abs(radius-(inner+outer)*0.5)/(span*0.5), direction == 2u); let band_t = select(0.5, f32(index)/f32(params.band_count-1u), params.band_count > 1u); let t = select(radial_t, band_t, (params.flags & 32u) != 0u); result = colour(t); }
    } else if (params.band_count > 0u) {
        let center = (top + bottom) * 0.5; let mapping = (params.flags >> 2u) & 3u; let count = select(params.band_count, params.band_count * 2u, mapping == 2u);
        let cell = size.x / f32(count); let visual = min(u32(floor((point.x-left)/cell)), count-1u);
        var index = select(visual, params.band_count-1u-visual, mapping == 1u);
        if (mapping == 2u) { index = select(params.band_count-1u-visual, visual-params.band_count, visual >= params.band_count); }
        let a = mix(params.style.y, 1.0, clamp(band(index), 0.0, 1.0)); let anchor = params.flags & 3u;
        let bar_left = left + f32(visual)*cell + cell*params.style.x*0.5; let bar_right = left + f32(visual+1u)*cell - cell*params.style.x*0.5;
        var bar_top = bottom - size.y * a;
        var bar_bottom = bottom;
        var distance = bottom - point.y;
        if (anchor == 1u) {
            bar_top = top;
            bar_bottom = top + size.y * a;
            distance = point.y - top;
        } else if (anchor == 2u) {
            bar_top = center - size.y * a * 0.5;
            bar_bottom = center + size.y * a * 0.5;
            distance = abs(point.y - center) * 2.0;
        }
        if (a > 0.0 && point.x >= bar_left && point.x < bar_right && point.y >= bar_top && point.y < bar_bottom) { let band_t = select(0.5, f32(index)/f32(params.band_count-1u), params.band_count > 1u); let t = select(distance/size.y, band_t, (params.flags & 32u) != 0u); result = colour(t); }
    }
    textureStore(output, vec2<i32>(id.xy), result);
}
