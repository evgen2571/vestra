@compute @workgroup_size(8,8)
fn compose(@builtin(global_invocation_id) invocation:vec3<u32>) {
    if invocation.x>=params.canvas_width || invocation.y>=params.canvas_height {return;}
    let coords=vec2<i32>(invocation.xy);
    let input=textureLoad(auxiliary,coords,0);
    if input.a==0.0 {textureStore(output,coords,input);return;}
    let cell=invocation.xy/vec2<u32>(params.cell_width,params.cell_height);
    var metadata:array<vec4<u32>,2>;
    if params.canvas_width<2u || params.canvas_height<2u {metadata=analyze_cell(auxiliary, cell);}
    else {
        let columns=(params.canvas_width+params.cell_width-1u)/params.cell_width;
        let index=2u*(cell.y*columns+cell.x);
        metadata[0]=vec4<u32>(round(textureLoad(source,vec2<i32>(i32(index%params.canvas_width),i32(index/params.canvas_width)),0)*255.0));
        metadata[1]=vec4<u32>(round(textureLoad(source,vec2<i32>(i32((index+1u)%params.canvas_width),i32((index+1u)/params.canvas_width)),0)*255.0));
    }
    let color=metadata[0];let info=metadata[1];
    let uv=(vec2<f32>(invocation.xy%vec2<u32>(params.cell_width,params.cell_height))+vec2<f32>(0.5))/vec2<f32>(f32(params.cell_width),f32(params.cell_height));
    let first=coverage(info.r,uv,false);
    let second=coverage(min(info.r+1u,params.glyph_count-1u),uv,false);
    let fill=first+(second-first)*f32(info.g)/255.0;
    let edge=coverage(params.glyph_count+info.b,uv,true);
    let weight=f32(info.a)/255.0;
    var ink=fill;
    if params.mode==1u {ink=edge*weight;}
    else if params.mode==2u {ink=fill+(edge-fill)*weight;}
    var foreground=rgba_bytes(params.foreground);
    if params.color_mode==1u {foreground=vec4<f32>(vec3<f32>(color.rgb),255.0);}
    else if params.color_mode>=2u {
        var tone=ascii_tone(color.rgb);
        if params.invert!=0u {tone=1.0-tone;}
        let position=tone*f32(params.count-1u);
        let lower=u32(floor(position));let upper=min(lower+1u,params.count-1u);
        foreground=round(rgba_bytes(params.colours[lower/4u][lower%4u])*(1.0-fract(position))+rgba_bytes(params.colours[upper/4u][upper%4u])*fract(position));
    }
    let background=rgba_bytes(params.background);
    let fa=foreground.a/255.0*ink;let ba=background.a/255.0*(1.0-ink);
    let style_alpha=(fa+ba)*input.a;
    let strength=params.amount*(1.0-params.source_mix);
    let alpha=input.a*(1.0-strength)+style_alpha*strength;
    let styled=(foreground.rgb*fa+background.rgb*ba)*input.a;
    let premul=input.rgb*255.0*input.a*(1.0-strength)+styled*strength;
    var result=vec4<f32>(0.0);
    if alpha>0.0 {result=vec4<f32>(round(clamp(premul/alpha,vec3<f32>(0.0),vec3<f32>(255.0)))/255.0,round(clamp(alpha*255.0,0.0,255.0))/255.0);}
    textureStore(output,coords,result);
}
