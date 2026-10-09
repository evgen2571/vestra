@compute @workgroup_size(8,8)
fn compose(@builtin(global_invocation_id) gid:vec3<u32>){
 if(gid.x>=params.canvas_width || gid.y>=params.canvas_height){return;}
 let xy=vec2<i32>(gid.xy);var result=vec4<f32>(0.0);let count=select(1u,3u,params.flags.x==2u);
 for(var ch=0u;ch<count;ch++){
  let s=value(1u+ch*2u);let c=value(2u+ch*2u);let id=cell(vec2<f32>(gid.xy)+vec2<f32>(0.5),s,c);
  if(any(representative(id,s,c)!=xy)){continue;}
  let bounds=cell_bounds(id,s,c);var sum=vec4<u32>(0u);
  for(var y=bounds.y;y<=bounds.w;y++){let span=row_span(id,s,c,y);for(var x=span.x;x<=span.y;x++){
   if(any(cell(vec2<f32>(f32(x)+0.5,f32(y)+0.5),s,c)!=id)){continue;}
   let p=vec4<u32>(round(textureLoad(source,vec2<i32>(x,y),0)*255.0));sum+=vec4<u32>(p.rgb*p.a,p.a);
  }}
  if(sum.a>0u){let mean=vec3<f32>((sum.rgb+vec3<u32>(sum.a/2u))/vec3<u32>(sum.a))/255.0;if(params.flags.x==2u){result[ch]=mean[ch];}else{result=vec4<f32>(mean,1.0);}}
  result.a=1.0;
 }
 textureStore(output,xy,round_byte(result));
}
