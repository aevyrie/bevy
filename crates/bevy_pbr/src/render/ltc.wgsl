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

    // Transform light position to LTC space
    let L = light_pos - P;
    let m_local = mat3x3<f32>(T1, T2, basis_z);
    let L_local = transpose(m_local) * L;
    let L_ltc = m_inv * L_local;
    let dist_ltc = length(L_ltc);
    let Lhat_ltc = L_ltc / max(dist_ltc, 1e-6);

    // Approximate transformed sphere radius
    // Det = m_inv[1][1] * (m_inv[0][0] * m_inv[2][2] - m_inv[2][0] * m_inv[0][2])
    let det = m_inv[1][1] * (m_inv[0][0] * m_inv[2][2] - m_inv[2][0] * m_inv[0][2]);
    let radius_scale = pow(abs(det), 0.333333);
    let light_radius_ltc = light_radius * radius_scale;

    // Frostbite/Unreal sphere integral (handles horizon clipping)
    let cosTheta = Lhat_ltc.z;
    let sinTheta = sqrt(saturate(1.0 - cosTheta * cosTheta));
    let sinAlpha = saturate(light_radius_ltc / max(dist_ltc, 1e-6));
    let cosAlpha = sqrt(saturate(1.0 - sinAlpha * sinAlpha));

    var I: f32;
    if (cosTheta >= cosAlpha) {
        // Fully visible
        I = sinAlpha * sinAlpha * cosTheta;
    } else if (cosTheta <= -cosAlpha) {
        // Fully invisible
        I = 0.0;
    } else {
        // Partially visible
        let cotTheta = cosTheta / max(sinTheta, 1e-6);
        let x = cotTheta * cosAlpha / max(sinAlpha, 1e-6);
        let y = sqrt(saturate(1.0 - x * x));
        I = (sinAlpha * sinAlpha * cosTheta * acos(clamp(-x, -1.0, 1.0)) + sinAlpha * cosAlpha * sinTheta * y) / PI;
    }

    // Normalize such that the punctual limit is cosTheta / PI
    return I / max(1e-7, PI * sinAlpha * sinAlpha);
}

fn ltc_brdf(
    N: vec3<f32>,
    V: vec3<f32>,
    P: vec3<f32>,
    roughness: f32,
    light_pos: vec3<f32>,
    light_radius: f32,
    F0: vec3<f32>,
) -> vec4<f32> {
    let NdotV = saturate(dot(N, V));
    let uv = vec2(roughness, acos(NdotV) / (0.5 * PI));
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

    // Use the normal as the basis Z-axis for consistent horizon clipping
    let spec = ltc_evaluate_sphere(N, V, P, m_inv, light_pos, light_radius, N);

    // t2.x is the magnitude (integral of the LTC BRDF)
    // t2.y is the fresnel term
    let fresnel = F0 * t2.x + (1.0 - F0) * t2.y;
    return vec4(spec * fresnel, t2.x);
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
    let uv = vec2(roughness, acos(NdotV) / (0.5 * PI));
    let uv_clamped = uv * LTC_LUT_SCALE + LTC_LUT_BIAS;
    
    let m_inv = mat3x3<f32>(
        vec3(1.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
        vec3(0.0, 0.0, 1.0)
    );
    let t_mag = textureSampleLevel(ltc_mag_texture, ltc_sampler, uv_clamped, 0.0);

    // For diffuse, use the normal as the basis Z-axis
    let diff = ltc_evaluate_sphere(N, V, P, m_inv, light_pos, light_radius, N);

    // ltc_evaluate_sphere already returns something that approaches cosTheta / PI as radius -> 0.
    return vec3(diff * t_mag.x);
}
