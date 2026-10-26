struct Object {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    color: vec4<f32>,
    params: vec4<f32>, // metallic, roughness, lit flag, ambient strength
    light_direction: vec4<f32>,
    light_color: vec4<f32>,
    camera_position: vec4<f32>,
    normal_matrix: mat3x3<f32>,
};
@group(0) @binding(0) var<uniform> object: Object;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>) -> VertexOut {
    var out: VertexOut;
    let local_position = vec4<f32>(position, 1.0);
    out.clip_position = object.mvp * local_position;
    out.world_position = (object.model * local_position).xyz;
    out.normal = object.normal_matrix * normal;
    return out;
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(1.0 - clamp(cos_theta, 0.0, 1.0), 5.0);
}

// A light opposite the view gives a zero halfway vector. Scale before taking
// the length so large finite normals/view vectors do not overflow their dot.
fn safe_direction(value: vec3<f32>) -> vec3<f32> {
    let magnitude = max(max(abs(value.x), abs(value.y)), abs(value.z));
    if !(magnitude > 0.0 && magnitude <= 3.4e38) {
        return vec3<f32>(0.0);
    }
    return normalize(value / magnitude);
}

@fragment fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    if object.params.z < 0.5 {
        return object.color;
    }

    let n = safe_direction(in.normal);
    let l = safe_direction(object.light_direction.xyz);
    let v = safe_direction(object.camera_position.xyz - in.world_position);
    let h = safe_direction(v + l);
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_v = max(dot(n, v), 1.0e-4);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);

    let metallic = object.params.x;
    let roughness = object.params.y;
    let alpha = roughness * roughness;
    let alpha2 = alpha * alpha;
    let denom = n_dot_h * n_dot_h * (alpha2 - 1.0) + 1.0;
    let distribution = alpha2 / max(3.14159265 * denom * denom, 1.0e-7);
    let geometry_k = (roughness + 1.0) * (roughness + 1.0) / 8.0;
    let geometry_v = n_dot_v / max(n_dot_v * (1.0 - geometry_k) + geometry_k, 1.0e-5);
    let geometry_l = n_dot_l / max(n_dot_l * (1.0 - geometry_k) + geometry_k, 1.0e-5);
    let f0 = mix(vec3<f32>(0.04), object.color.rgb, metallic);
    let fresnel = fresnel_schlick(v_dot_h, f0);
    let specular = distribution * geometry_v * geometry_l * fresnel / max(4.0 * n_dot_v * n_dot_l, 1.0e-4);
    let diffuse_weight = (vec3<f32>(1.0) - fresnel) * (1.0 - metallic);
    let diffuse = diffuse_weight * object.color.rgb / 3.14159265;
    let direct = (diffuse + specular) * object.light_color.rgb * n_dot_l;
    let ambient = object.color.rgb * object.params.w;
    return vec4<f32>(direct + ambient, 1.0);
}
