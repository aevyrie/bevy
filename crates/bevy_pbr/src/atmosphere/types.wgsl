#define_import_path bevy_pbr::atmosphere::types

struct Atmosphere {
    // Radius of the planet
    bottom_radius: f32, //units: km

    // Radius at which we consider the atmosphere to 'end' for out calculations (from center of planet)
    top_radius: f32, //units: km

    ground_albedo: vec3<f32>,

    rayleigh_density_exp_scale: f32,
    rayleigh_scattering: vec3<f32>,

    mie_density_exp_scale: f32,
    mie_scattering: f32, //units: km^-1
    mie_absorption: f32, //units: km^-1
    mie_asymmetry: f32, //the "asymmetry" value of the phase function, unitless. Domain: (-1, 1)

    ozone_layer_center_altitude: f32, //units: km
    ozone_layer_half_width: f32, //units: km
    ozone_absorption: vec3<f32>, //ozone absorption. units: km^-1
}

struct AtmosphereSettings {
    transmittance_lut_size: vec2<u32>,
    multiscattering_lut_size: vec2<u32>,
    sky_view_lut_size: vec2<u32>,
    transmittance_lut_samples: u32,
    multiscattering_lut_dirs: u32,
    aerial_view_lut_size: vec3<u32>, //Gross ordering for padding reasons
    multiscattering_lut_samples: u32,
    sky_view_lut_samples: u32,
    aerial_view_lut_samples: u32,
    scene_units_to_km: f32,
}
