@compute @workgroup_size(8,8)
fn compose(@builtin(global_invocation_id) invocation:vec3<u32>) {
    if params.canvas_width<2u || params.canvas_height<2u {return;}
    let columns=(params.canvas_width+params.cell_width-1u)/params.cell_width;
    let rows=(params.canvas_height+params.cell_height-1u)/params.cell_height;
    if invocation.x>=columns || invocation.y>=rows {return;}
    let metadata=analyze_cell(source, invocation.xy);
    let index=2u*(invocation.y*columns+invocation.x);
    textureStore(output,vec2<i32>(i32(index%params.canvas_width),i32(index/params.canvas_width)),vec4<f32>(metadata[0])/255.0);
    textureStore(output,vec2<i32>(i32((index+1u)%params.canvas_width),i32((index+1u)/params.canvas_width)),vec4<f32>(metadata[1])/255.0);
}
