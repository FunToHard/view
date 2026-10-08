struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) @interpolate(flat) straight: f32,
}
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>,
             @location(2) color: vec4<f32>, @location(3) straight: f32) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    out.straight = straight;
    return out;
}
@fragment fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    var sampled = textureSample(image, image_sampler, in.uv);
    // Straight-alpha texels must be premultiplied BEFORE interpolation; doing
    // it after textureSample creates colored fringes at transparent boundaries.
    if in.straight > 0.5 {
        let extent = vec2<i32>(textureDimensions(image));
        let p = in.uv * vec2<f32>(extent) - vec2<f32>(0.5);
        let base = vec2<i32>(floor(p));
        let f = fract(p);
        let a = premultiply(textureLoad(image, clamp(base, vec2<i32>(0), extent - 1), 0));
        let b = premultiply(textureLoad(image, clamp(base + vec2<i32>(1,0), vec2<i32>(0), extent - 1), 0));
        let c = premultiply(textureLoad(image, clamp(base + vec2<i32>(0,1), vec2<i32>(0), extent - 1), 0));
        let d = premultiply(textureLoad(image, clamp(base + vec2<i32>(1,1), vec2<i32>(0), extent - 1), 0));
        sampled = mix(mix(a,b,f.x),mix(c,d,f.x),f.y);
    }
    return sampled * in.color;
}
fn premultiply(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(color.rgb * color.a, color.a);
}
