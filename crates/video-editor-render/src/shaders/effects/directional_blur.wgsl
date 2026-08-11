@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) { if (id.x < params.canvas_width && id.y < params.canvas_height) { textureStore(output, vec2<i32>(id.xy), line_blur(vec2<i32>(id.xy))); } }
