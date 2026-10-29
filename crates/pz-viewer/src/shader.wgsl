struct Cam { center: vec2<f32>, half_view: vec2<f32>, }
@group(0) @binding(0) var<uniform> cam: Cam;
@group(1) @binding(0) var tex: texture_2d_array<f32>;
@group(1) @binding(1) var smp: sampler;

struct VOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec4<f32>, @location(2) @interpolate(flat) layer: i32, }

@vertex
fn vs(@builtin(vertex_index) vi: u32,
      @location(0) ipos: vec2<f32>, @location(1) isize: vec2<f32>, @location(2) iuv: vec4<f32>, @location(3) icolor: vec4<f32>, @location(4) ilayer: f32) -> VOut {
    let c = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let world = ipos + c * isize;
    let ndc = (world - cam.center) / cam.half_view;
    var o: VOut;
    o.pos = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);
    o.uv = mix(iuv.xy, iuv.zw, c);
    o.color = icolor;
    o.layer = i32(ilayer);
    return o;
}

@fragment
fn fs(i: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, smp, i.uv, i.layer) * i.color;
    if (c.a < 0.004) { discard; }
    return c;
}
