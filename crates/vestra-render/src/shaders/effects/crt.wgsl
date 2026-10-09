fn noise_hash(input:u32)->u32{var v=input;v^=v>>16u;v*=0x7feb352du;v^=v>>15u;v*=0x846ca68bu;return v^(v>>16u);}
fn noise(x:u32,y:u32,phase:f32)->f32{let h=noise_hash(x*374761393u^y*668265263u^params.flags.x);let a=f32(h&65535u)/32767.5-1.0;let b=f32(h>>16u)/32767.5-1.0;return a*cos(phase)+b*sin(phase);}
@compute @workgroup_size(8,8)
fn compose(@builtin(global_invocation_id) id:vec3<u32>){
 if(id.x>=params.canvas_width || id.y>=params.canvas_height){return;}
 let xy=vec2<i32>(id.xy);let original=textureLoad(source,xy,0);let dimensions=vec2<f32>(f32(params.canvas_width),f32(params.canvas_height));let p=vec2<f32>(id.xy)+vec2<f32>(0.5);let normalized=p/dimensions*2.0-vec2<f32>(1.0);let phase=value(10u);
 var sampled=(normalized*(vec2<f32>(1.0)+value(1u)*normalized.yx*normalized.yx)+vec2<f32>(1.0))*dimensions/2.0;sampled.x+=value(6u)*noise(0u,id.y/4u,phase);
 let edge=clamp(min(min(sampled.x,dimensions.x-sampled.x),min(sampled.y,dimensions.y-sampled.y))+0.5,0.0,1.0);var screen=bilinear_edge(source,sampled);screen.a*=edge;
 let frequency=6.283185307179586/value(3u);let aa=sin(frequency/2.0)/(frequency/2.0);let scan=1.0-value(2u)*(0.5-0.5*cos(f32(id.y)*frequency)*aa);
 let rolling_phase=p.y/dimensions.y*6.283185307179586-phase;let w=value(9u)*6.283185307179586;let wave=sin(rolling_phase);let roll=exp(-2.0*wave*wave/(w*w));let light=scan*(1.0+value(7u)*sin(phase))*(1.0-value(8u)*roll);let grain=value(5u)*noise(id.x,id.y,phase*3.0);
 for(var ch=0u;ch<3u;ch++){let mask=select(1.0-value(4u),1.0,(id.x/params.flags.y)%3u==ch);screen[ch]=clamp(screen[ch]*light*mask+grain,0.0,1.0);}
 textureStore(output,xy,premult_mix(original,screen,value(0u)));
}
