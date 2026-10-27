var<workgroup> colours:array<vec4<f32>,256>;
var<workgroup> keys:array<u32,256>;
var<workgroup> groups:array<u32,256>;
var<workgroup> indices:array<u32,256>;
fn coordinate(line:u32,pos:u32)->vec2<i32>{if(params.flags.x!=0u){return vec2<i32>(i32(line),i32(pos));}return vec2<i32>(i32(pos),i32(line));}
fn eligible(p:vec4<f32>)->bool{let k=analog_key(p);return p.a>0.0 && f32(k)>=value(0u) && f32(k)<=value(1u) && params.flags.w==0u;}
fn less(a:u32,b:u32)->bool{if(groups[a]!=groups[b]){return groups[a]<groups[b];}if(keys[a]!=keys[b]){return keys[a]<keys[b];}return indices[a]<indices[b];}
@compute @workgroup_size(8,8)
fn compose(@builtin(workgroup_id) block:vec3<u32>,@builtin(local_invocation_index) thread:u32){
 let extent=select(params.canvas_width,params.canvas_height,params.flags.x!=0u);let lines=select(params.canvas_height,params.canvas_width,params.flags.x!=0u);
 let start=block.x*params.flags.z;let line=block.y;
 // Dispatch guarantees all 64 lanes reach each barrier, including partial blocks.
 var capacity=2u;while(capacity<params.flags.z){capacity*=2u;}
 for(var i=thread;i<capacity;i+=64u){indices[i]=i;groups[i]=256u;keys[i]=0xffffffffu;colours[i]=vec4<f32>(0.0);
  if(i<params.flags.z && start+i<extent && line<lines){let p=textureLoad(source,coordinate(line,start+i),0);colours[i]=p;groups[i]=i;
   if(eligible(p)){let k=analog_key(p);keys[i]=select(k,65280u-k,params.flags.y!=0u);}
  }
 }
 workgroupBarrier();
 for(var i=thread;i<capacity;i+=64u){if(keys[i]!=0xffffffffu){var first=i;while(first>0u){if(keys[first-1u]==0xffffffffu){break;}first--;}groups[i]=first;}}
 workgroupBarrier();
 for(var size=2u;size<=capacity;size*=2u){for(var stride=size/2u;stride>0u;stride/=2u){
  for(var i=thread;i<capacity;i+=64u){let j=i^stride;if(j>i){let ascending=(i&size)==0u;let swap=select(less(i,j),less(j,i),ascending);if(swap){let p=colours[i];colours[i]=colours[j];colours[j]=p;let k=keys[i];keys[i]=keys[j];keys[j]=k;let g=groups[i];groups[i]=groups[j];groups[j]=g;let n=indices[i];indices[i]=indices[j];indices[j]=n;}}}
  workgroupBarrier();
 }}
 for(var i=thread;i<params.flags.z;i+=64u){if(start+i<extent && line<lines){let xy=coordinate(line,start+i);let original=textureLoad(source,xy,0);if(indices[i]==i){textureStore(output,xy,original);}else{textureStore(output,xy,premult_mix(original,colours[i],value(2u)));}}}
}
