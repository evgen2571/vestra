fn floor_div(n:i32,d:i32)->i32{if(n<0){return (n-d+1)/d;}return n/d;}
fn cell(p:vec2<f32>,s:f32,c:f32)->vec2<i32>{
 let centers=vec2<i32>(p*2.0);let si=i32(s*65536.0);let co=i32(c*65536.0);let denominator=i32(value(0u)*65536.0)*2;
 return vec2<i32>(floor_div(centers.x*co-centers.y*si,denominator),floor_div(centers.x*si+centers.y*co,denominator));
}

fn cell_bounds(id:vec2<i32>,s:f32,c:f32)->vec4<i32>{
 var lo=vec2<f32>(1e20);var hi=vec2<f32>(-1e20);
 for(var dx=0;dx<=1;dx++){for(var dy=0;dy<=1;dy++){let uv=vec2<f32>(id+vec2<i32>(dx,dy))*value(0u);let p=vec2<f32>(uv.x*c+uv.y*s,-uv.x*s+uv.y*c)/(s*s+c*c);lo=min(lo,p);hi=max(hi,p);}}
 return vec4<i32>(vec2<i32>(max(floor(lo-vec2<f32>(0.5)),vec2<f32>(0.0))),vec2<i32>(min(ceil(hi-vec2<f32>(0.5)),vec2<f32>(f32(params.canvas_width)-1.0,f32(params.canvas_height)-1.0))));
}
fn interval(lo:f32,hi:f32,a:f32,b:f32)->vec2<f32>{if(abs(a)<0.000001){if(b>=lo && b<hi){return vec2<f32>(-1e20,1e20);}return vec2<f32>(1e20,-1e20);}let x=(lo-b)/a;let y=(hi-b)/a;return vec2<f32>(min(x,y),max(x,y));}
fn row_span(id:vec2<i32>,s:f32,c:f32,y:i32)->vec2<i32>{
 let yf=f32(y)+0.5;let u=interval(f32(id.x)*value(0u),f32(id.x+1)*value(0u),c,-yf*s);let v=interval(f32(id.y)*value(0u),f32(id.y+1)*value(0u),s,yf*c);
 return vec2<i32>(i32(clamp(floor(max(u.x,v.x)-0.5),0.0,f32(params.canvas_width))),i32(clamp(ceil(min(u.y,v.y)-0.5),-1.0,f32(params.canvas_width)-1.0)));
}
fn representative(id:vec2<i32>,s:f32,c:f32)->vec2<i32>{
 let bounds=cell_bounds(id,s,c);
 for(var y=bounds.y;y<=bounds.w;y++){let span=row_span(id,s,c,y);for(var x=span.x;x<=min(span.y,span.x+3);x++){if(all(cell(vec2<f32>(f32(x)+0.5,f32(y)+0.5),s,c)==id)){return vec2<i32>(x,y);}}}
 return vec2<i32>(0);
}
