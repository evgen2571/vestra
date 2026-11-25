// Converts the dedicated premultiplied particle accumulation texture back to
// Vestra's straight-alpha working-texture representation.
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var destination: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    let dimensions = textureDimensions(source);
    if (id.x >= dimensions.x || id.y >= dimensions.y) {
        return;
    }
    let pixel = textureLoad(source, vec2<i32>(id.xy), 0);
    var rgb = vec3<f32>(0.0);
    if (pixel.a > 0.0) {
        rgb = pixel.rgb / pixel.a;
    }
    textureStore(destination, vec2<i32>(id.xy), vec4<f32>(rgb, pixel.a));
}
