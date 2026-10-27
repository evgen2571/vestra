// OriginalAnd binds processed metadata as source and retained pixels as auxiliary.
fn screen(xy:vec2<f32>,ch:u32)->vec4<f32>{
 let s=value(1u+ch*2u);let c=value(2u+ch*2u);let id=cell(xy,s,c);let colour=textureLoad(source,representative(id,s,c),0);
 var tone=(54.0*colour.r+183.0*colour.g+19.0*colour.b)/256.0;if(params.flags.x==2u){tone=colour[ch];}if(params.flags.y!=0u){tone=1.0-tone;}
 let uv=vec2<f32>(xy.x*c-xy.y*s,xy.x*s+xy.y*c);let center=(vec2<f32>(id)+vec2<f32>(0.5))*value(0u);
 let distance=length(uv-center);let radius=sqrt(tone)*value(0u)/sqrt(2.0);var coverage=select(0.0,1.0,distance<=radius);
 if(value(7u)>0.0){coverage=smoothstep(distance-value(7u)*0.5,distance+value(7u)*0.5,radius);}
 if(tone<=0.0){coverage=0.0;}else if(tone>=1.0){coverage=1.0;}
 return vec4<f32>(colour.rgb,coverage);
}
@compute @workgroup_size(8,8)
fn compose(@builtin(global_invocation_id) gid:vec3<u32>){
 if(gid.x>=params.canvas_width || gid.y>=params.canvas_height){return;}
 let xy=vec2<i32>(gid.xy);let original=textureLoad(auxiliary,xy,0);if(original.a==0.0){textureStore(output,xy,original);return;}
 let p=vec2<f32>(gid.xy)+vec2<f32>(0.5);let mono=screen(p,0u);var colour=vec3<f32>(0.0);
 for(var ch=0u;ch<3u;ch++){var dot=mono.a;if(params.flags.x==2u){dot=screen(p,ch).a;}let fg=select(value(9u+ch),mono[ch],params.flags.x==1u);colour[ch]=mix(value(12u+ch),fg,dot);}
 textureStore(output,xy,round_byte(vec4<f32>(mix(original.rgb,colour,value(8u)),original.a)));
}
