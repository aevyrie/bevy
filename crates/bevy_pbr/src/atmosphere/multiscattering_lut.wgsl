#import bevy_pbr::{
    mesh_view_types::{Lights, DirectionalLight},
    atmosphere::{
        types::{Atmosphere, AtmosphereSettings},
        bindings::{atmosphere, settings, multiscattering_lut_out},
        functions::{multiscattering_lut_uv_to_r_mu, sample_transmittance_lut},
        bruneton_functions::{
            distance_to_top_atmosphere_boundary, distance_to_bottom_atmosphere_boundary,
        }
    }
}


fn s2_sequence(n: u32) -> vec2<f32> {
//    const phi_2 = vec2(1.3247179572447460259609088, 1.7548776662466927600495087);
//    fract(0.5 + phi_2 * n);
    return vec2(0.0, 0.0); //TODO
}

//Lambert equal-area projection. 
fn map_to_hemisphere(uv: vec2<f32>) -> vec2<f32> {
    return vec2(0.0, 0.0); //TODO
}

@compute 
@workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let uv: vec2<f32> = (vec2<f32>(global_id.xy) + 0.5) / vec2<f32>(settings.multiscattering_lut_size);

    //See Multiscattering LUT paramatrization
    let r_mu = multiscattering_lut_uv_to_r_mu(uv);

    /*for (var dir_i: u32= 0u; dir_i < settings.multiscattering_lut_dirs; dir_i++) {
        let phi_theta = map_to_hemisphere(s2_sequence(dir_i));
        let mu = phi_theta.y; // cos(azimuth_angle) = dot(vec3::up, dir);

        let top_atmosphere_dist = distance_to_top_atmosphere_boundary(r, mu);
        let bottom_atmosphere_dist = distance_to_bottom_atmosphere_boundary(r, mu);
        let atmosphere_dist = min(top_atmosphere_dist, bottom_atmosphere_dist);

        sample_multiscattering_dir(atmosphere, r_mu, atmosphere_dist);
    }*/
}

fn sample_multiscattering_dir(atmosphere: Atmosphere, r_cos_azimuth: vec2<f32>, dir: vec2<f32>, atmosphere_dist: f32) {
//    for (var step_i: u32 = 0u; step_i < settings.multiscattering_lut_samples; step_i++) {
//    }
}

