struct AnalogParams { canvas_width:u32, canvas_height:u32, padding:vec2<u32>, values:array<vec4<f32>,4>, flags:vec4<u32> }
@group(0) @binding(3) var<uniform> params:AnalogParams;
fn value(index:u32)->f32 {return params.values[index/4u][index%4u];}
fn premult_mix(a:vec4<f32>,b:vec4<f32>,t:f32)->vec4<f32> {
 let alpha=mix(a.a,b.a,t);if(alpha<=0.0000001){return vec4<f32>(0.0);}
 return round_byte(vec4<f32>(mix(a.rgb*a.a,b.rgb*b.a,t)/alpha,alpha));
}
fn analog_key(p:vec4<f32>)->u32 {let b=vec3<u32>(round(p.rgb*255.0));return 54u*b.r+183u*b.g+19u*b.b;}
