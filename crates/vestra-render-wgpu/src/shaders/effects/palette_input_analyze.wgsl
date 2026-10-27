struct Params { canvas_width: u32, canvas_height: u32, scale: u32, mode: u32 };
@group(0) @binding(3) var<uniform> params: Params;

fn weighted_pixel(coordinate: vec2<u32>) -> vec4<u32> {
    let pixel = vec4<u32>(round(textureLoad(source, vec2<i32>(coordinate), 0) * 255.0));
    return vec4<u32>(pixel.rgb * pixel.a, pixel.a);
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= params.canvas_width || id.y >= params.canvas_height) { return; }
    if (any(id.xy % params.scale != vec2<u32>(0u))) { return; }
    let extent = min(vec2<u32>(params.scale), vec2<u32>(params.canvas_width, params.canvas_height) - id.xy);
    var sum = vec4<u32>(0u);
    var count = 0u;
    if (params.mode == 0u) {
        sum = weighted_pixel(id.xy + extent / 2u);
        count = 1u;
    } else if (params.mode == 1u) {
        let low = id.xy + (extent - 1u) / 2u;
        let high = id.xy + extent / 2u;
        sum = weighted_pixel(low) + weighted_pixel(high)
            + weighted_pixel(vec2<u32>(low.x, high.y))
            + weighted_pixel(vec2<u32>(high.x, low.y));
        count = 4u;
    } else {
        for (var y = id.y; y < id.y + extent.y; y++) {
            for (var x = id.x; x < id.x + extent.x; x++) {
                sum += weighted_pixel(vec2<u32>(x, y));
            }
        }
        count = extent.x * extent.y;
    }
    var result = vec4<u32>(0u);
    if (sum.a != 0u) {
        result = vec4<u32>((sum.rgb + vec3<u32>(sum.a / 2u)) / sum.a, (sum.a + count / 2u) / count);
    }
    textureStore(output, vec2<i32>(id.xy), vec4<f32>(result) / 255.0);
}
