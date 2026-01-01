#define_import_path bevy_pbr::ltc

#import bevy_pbr::mesh_view_bindings::{ltc_1_texture, ltc_2_texture, ltc_mag_texture, ltc_sampler}
#import bevy_render::maths::PI

// LTC lookup table size
const LTC_LUT_SIZE: f32 = 11.0;
const LTC_LUT_SCALE: f32 = (LTC_LUT_SIZE - 1.0) / LTC_LUT_SIZE;
const LTC_LUT_BIAS: f32 = 0.5 / LTC_LUT_SIZE;

fn ltc_integrate_disk(cos_theta: f32, sin_alpha_sq: f32) -> f32 {
    let cos_alpha = sqrt(saturate(1.0 - sin_alpha_sq));
    if (cos_theta >= cos_alpha) {
        return sin_alpha_sq * cos_theta;
    }
    if (cos_theta <= -cos_alpha) {
        return 0.0;
    }
    let sin_alpha = sqrt(sin_alpha_sq);
    let x = cos_theta / sin_alpha;
    let y = sqrt(saturate(1.0 - x * x));
    return (sin_alpha_sq * cos_theta * acos(clamp(-x, -1.0, 1.0)) + sin_alpha * cos_alpha * y) / PI;
}

fn ltc_evaluate_sphere(
    N: vec3<f32>,
    V: vec3<f32>,
    P: vec3<f32>,
    m_inv: mat3x3<f32>,
    light_pos: vec3<f32>,
    light_radius: f32,
    basis_z: vec3<f32>
) -> f32 {
    // Construct orthonormal basis around basis_z
    var T1: vec3<f32>;
    let ZdotV = dot(basis_z, V);
    if (abs(ZdotV) < 0.9999) {
        T1 = normalize(V - basis_z * ZdotV);
    } else {
        // V is parallel to basis_z, pick any tangent
        T1 = vec3(1.0, 0.0, 0.0);
        if (abs(dot(T1, basis_z)) > 0.99) {
            T1 = vec3(0.0, 1.0, 0.0);
        }
        T1 = normalize(T1 - basis_z * dot(T1, basis_z));
    }
    let T2 = cross(basis_z, T1);

    // Geometry
    let L = light_pos - P;
    let dist2 = dot(L, L);
    let dist = max(sqrt(dist2), light_radius * 1.01);
    let Lhat = L / dist;

    // Sphere cap
    let sinTheta = saturate(light_radius / dist);
    let cosTheta = sqrt(saturate(1.0 - sinTheta * sinTheta));

    // Transformed cosine axis
    let m_local = mat3x3<f32>(T1, T2, basis_z);
    let k = normalize(transpose(m_inv) * vec3(0.0, 0.0, 1.0));
    
    // Angle between LTC axis and light center
    // We need to transform Lhat into the local basis (T1, T2, basis_z)
    let Lhat_local = transpose(m_local) * Lhat;
    let cosAlpha = dot(k, Lhat_local);

    // Arvo spherical-cap cosine integral (exact)
    var I: f32;
    if (cosAlpha <= -cosTheta) {
        I = 0.0;
    } else if (cosAlpha >= cosTheta) {
        // fully visible
        I = PI * sinTheta * sinTheta * cosAlpha;
    } else {
        // partial (crescent) region
        let x = cosAlpha / sinTheta;
        let root = sqrt(saturate(1.0 - x * x));
        I = sinTheta * sinTheta * (acos(clamp(-x, -1.0, 1.0)) * cosAlpha + sinTheta * root) * 0.5;
    }

    // Near-field correction
    let sphereCorrection = (dist * dist) / (dist * dist - light_radius * light_radius);
    I *= sphereCorrection;

    // Normalize such that the punctual limit is cosAlpha / dist2
    I /= max(1e-7, PI * sinTheta * sinTheta);

    return I;
}

fn ltc_brdf(
    N: vec3<f32>,
    V: vec3<f32>,
    P: vec3<f32>,
    roughness: f32,
    light_pos: vec3<f32>,
    light_radius: f32,
    F0: vec3<f32>,
) -> vec3<f32> {
    let NdotV = saturate(dot(N, V));
    let uv = vec2(roughness, sqrt(1.0 - NdotV));
    let uv_clamped = uv * LTC_LUT_SCALE + LTC_LUT_BIAS;

    let t1 = textureSampleLevel(ltc_1_texture, ltc_sampler, uv_clamped, 0.0);
    let t2 = textureSampleLevel(ltc_2_texture, ltc_sampler, uv_clamped, 0.0);

    // Reconstruct m_inv.
    // Mapping for 4-parameter LUT: t1.x=m33, t1.y=m13, t1.z=m31, t1.w=m22. m11 is 1.0.
    let m_inv = mat3x3<f32>(
        vec3(1.0, 0.0, t1.z),
        vec3(0.0, t1.w, 0.0),
        vec3(t1.y, 0.0, t1.x)
    );

    // For specular, use the reflection vector as the basis Z-axis
    let R = reflect(-V, N);
    let spec = ltc_evaluate_sphere(N, V, P, m_inv, light_pos, light_radius, R);

    // t2.x is the magnitude (integral of the LTC BRDF)
    // t2.y is the fresnel term
    return spec * (F0 * t2.x + (1.0 - F0) * t2.y);
}

fn ltc_diffuse(
    N: vec3<f32>,
    V: vec3<f32>,
    P: vec3<f32>,
    roughness: f32,
    light_pos: vec3<f32>,
    light_radius: f32,
) -> vec3<f32> {
    let NdotV = saturate(dot(N, V));
    // For Lambertian diffuse, LTC is just the identity matrix.
    let m_inv = mat3x3<f32>(
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 0.0, 1.0)
    );

    let uv = vec2(roughness, sqrt(1.0 - NdotV));
    let uv_clamped = uv * LTC_LUT_SCALE + LTC_LUT_BIAS;
    let t_mag = textureSampleLevel(ltc_mag_texture, ltc_sampler, uv_clamped, 0.0);

    // For diffuse, use the normal as the basis Z-axis
    // For Lambertian, m_inv is identity, so k is (0,0,1)
    let diff = ltc_evaluate_sphere(N, V, P, m_inv, light_pos, light_radius, N);

    // For diffuse, we want to match Bevy's Fd_Burley or Lambertian.
    // Punctual limit should be NdotL / PI.
    // ltc_evaluate_sphere already returns something that approaches cosAlpha (NdotL) as radius -> 0.
    // So we just need to divide by PI.
    return vec3(diff * t_mag.x / PI);
}
