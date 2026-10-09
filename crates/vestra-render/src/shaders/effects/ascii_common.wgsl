struct Params {
    canvas_width: u32, canvas_height: u32, cell_width: u32, cell_height: u32,
    glyph_count: u32, glyph_style: u32, mode: u32, color_mode: u32,
    foreground: u32, background: u32, invert: u32, count: u32,
    edge_threshold: f32, edge_strength: f32, source_mix: f32, amount: f32,
    colours: array<vec4<u32>,4>,
};
@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var glyph_atlas: texture_2d<f32>;

fn rgba_bytes(packed: u32) -> vec4<f32> {
    return vec4<f32>(f32(packed & 255u), f32((packed >> 8u) & 255u), f32((packed >> 16u) & 255u), f32(packed >> 24u));
}
fn ascii_tone(color: vec3<u32>) -> f32 {
    return f32(54u*color.r+183u*color.g+19u*color.b)/65280.0;
}
fn half_mean(pair: vec2<u32>) -> f32 {
    if pair.y == 0u { return 0.0; }
    return f32(pair.x)/f32(pair.y)/255.0;
}
fn analyze_cell(input_texture: texture_2d<f32>, cell: vec2<u32>) -> array<vec4<u32>,2> {
    let start=cell*vec2<u32>(params.cell_width,params.cell_height);
    let end=min(start+vec2<u32>(params.cell_width,params.cell_height),vec2<u32>(params.canvas_width,params.canvas_height));
    var sums=vec4<u32>(0u);
    var halves:array<vec2<u32>,4>;
    for(var y=start.y;y<end.y;y++){ for(var x=start.x;x<end.x;x++){
        let pixel=vec4<u32>(round(textureLoad(input_texture,vec2<i32>(i32(x),i32(y)),0)*255.0));
        sums += vec4<u32>(pixel.rgb*pixel.a,pixel.a);
        let luma=(54u*pixel.r+183u*pixel.g+19u*pixel.b+128u)/256u;
        let side_x=select(0u,1u,2u*(x-start.x)>=end.x-start.x);
        let side_y=select(2u,3u,2u*(y-start.y)>=end.y-start.y);
        let pair=vec2<u32>(luma*pixel.a,pixel.a);
        halves[side_x]+=pair; halves[side_y]+=pair;
    }}
    var color=vec4<u32>(0u);
    let count=(end.x-start.x)*(end.y-start.y);
    if sums.a>0u { color=vec4<u32>((sums.rgb+vec3<u32>(sums.a/2u))/sums.a,(sums.a+count/2u)/count); }
    var tone=ascii_tone(color.rgb);
    if params.invert!=0u {tone=1.0-tone;}
    let position=tone*f32(params.glyph_count-1u);
    let index=u32(min(floor(position),f32(params.glyph_count-1u)));
    let fraction=clamp((position-f32(index)-0.4375)*8.0,0.0,1.0);
    let gx=half_mean(halves[1])-half_mean(halves[0]);
    let gy=half_mean(halves[3])-half_mean(halves[2]);
    let magnitude=sqrt(gx*gx+gy*gy);
    var orientation=3u;
    if abs(gx)>abs(gy)*2.4142137 {orientation=1u;}
    else if abs(gy)>abs(gx)*2.4142137 {orientation=0u;}
    else if gx*gy>=0.0 {orientation=2u;}
    let weight=clamp((magnitude-params.edge_threshold)*8.0+0.5,0.0,1.0)*params.edge_strength;
    return array<vec4<u32>,2>(color,vec4<u32>(index,u32(round(fraction*255.0)),orientation,u32(round(clamp(weight,0.0,1.0)*255.0))));
}
fn atlas_sample(index:u32, pos:vec2<i32>) -> f32 {
    let level=params.glyph_style>>1u;
    let tile=vec2<i32>(i32(32u>>level),i32(48u>>level));
    let origin=vec2<i32>(i32(index%16u),i32(index/16u))*tile;
    return textureLoad(glyph_atlas,origin+clamp(pos,vec2<i32>(0),tile-vec2<i32>(1)),i32(level)).r;
}
fn coverage(index:u32,uv:vec2<f32>,edge:bool)->f32 {
    if (params.glyph_style&1u)!=0u {
        let p=(uv-vec2<f32>(0.5))*vec2<f32>(f32(params.cell_width),f32(params.cell_height));
        if edge {
            let orientation=index-params.glyph_count;
            var distance=abs(p.x-p.y)*0.70710677;
            if orientation==0u {distance=abs(p.y);}
            else if orientation==1u {distance=abs(p.x);}
            else if orientation==2u {distance=abs(p.x+p.y)*0.70710677;}
            return clamp(0.75-distance,0.0,1.0);
        }
        let level=f32(index)/f32(max(1u,params.glyph_count-1u));
        if level==0.0 {return 0.0;}
        let dot=clamp(0.75-(length(p)-level*f32(min(params.cell_width,params.cell_height))*0.45),0.0,1.0);
        let cross=max(clamp(0.75-abs(p.x),0.0,1.0),clamp(0.75-abs(p.y),0.0,1.0));
        return max(dot,cross*clamp((level-0.5)*2.0,0.0,1.0));
    }
    let level=params.glyph_style>>1u;
    let pos=uv*vec2<f32>(f32(32u>>level),f32(48u>>level))-vec2<f32>(0.5);
    let corner=vec2<i32>(floor(pos));
    let f=pos-floor(pos);
    let top=atlas_sample(index,corner)*(1.0-f.x)+atlas_sample(index,corner+vec2<i32>(1,0))*f.x;
    let bottom=atlas_sample(index,corner+vec2<i32>(0,1))*(1.0-f.x)+atlas_sample(index,corner+vec2<i32>(1,1))*f.x;
    return top*(1.0-f.y)+bottom*f.y;
}
