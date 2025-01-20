//! This example showcases pbr atmospheric scattering

use std::f32::consts::PI;

use bevy::{
    color::palettes,
    core_pipeline::{
        auto_exposure::{AutoExposure, AutoExposureCompensationCurve, AutoExposurePlugin},
        bloom::Bloom,
        tonemapping::Tonemapping,
        Skybox,
    },
    pbr::{light_consts::lux, Atmosphere, AtmosphereSettings, CascadeShadowConfigBuilder},
    prelude::*,
};
use bevy_render::view::ColorGrading;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, AutoExposurePlugin))
        .add_systems(Startup, (setup_camera_fog, setup_terrain_scene))
        .add_systems(Update, dynamic_scene)
        .insert_resource(AmbientLight::NONE)
        .run();
}

fn setup_camera_fog(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut compensation_curves: ResMut<Assets<AutoExposureCompensationCurve>>,
) {
    let metering_mask = asset_server.load("textures/basic_metering_mask.png");
    let night_sky = asset_server.load("environment_maps/night.ktx2");

    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        AutoExposure {
            range: -4.5..=14.0,
            speed_brighten: 60.0,
            speed_darken: 20.0,
            metering_mask: metering_mask.clone(),
            compensation_curve: compensation_curves.add(
                AutoExposureCompensationCurve::from_curve(CubicCardinalSpline::new(
                    0.5,
                    [vec2(-8.0, 1.0), vec2(4.0, -2.0)],
                ))
                .unwrap(),
            ),
            ..Default::default()
        },
        Skybox {
            image: night_sky,
            brightness: 500.0,
            rotation: Quat::default(),
        },
        Tonemapping::AcesFitted,
        Transform::from_xyz(0.0, 0.15, -1.0).looking_at(Vec3::Y * 0.3, Vec3::Y),
        Bloom::NATURAL,
        Atmosphere::EARTH,
        AtmosphereSettings {
            aerial_view_lut_max_distance: 3.2e5,
            scene_units_to_m: 1e+4,
            ..Default::default()
        },
    ));
}

#[derive(Component)]
struct Terrain;

#[derive(Component)]
struct Sun;

fn setup_terrain_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Configure a properly scaled cascade shadow map for this scene (defaults are too large, mesh units are in km)
    let cascade_shadow_config = CascadeShadowConfigBuilder {
        first_cascade_far_bound: 0.3,
        maximum_distance: 10.0,
        ..default()
    }
    .build();

    // Sun
    commands.spawn((
        Sun,
        DirectionalLight {
            shadows_enabled: true,
            illuminance: lux::RAW_SUNLIGHT,
            ..default()
        },
        Transform::default(),
        cascade_shadow_config.clone(),
    ));

    // Moon
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: lux::FULL_MOON_NIGHT * 500.0,
            ..default()
        },
        Transform::default(),
        cascade_shadow_config,
    ));

    let sphere_mesh = meshes.add(Mesh::from(Sphere { radius: 1.0 }));

    // light probe spheres
    commands.spawn((
        Mesh3d(sphere_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            metallic: 1.0,
            perceptual_roughness: 0.0,
            ..default()
        })),
        Transform::from_xyz(-0.3, 0.3, 0.0).with_scale(Vec3::splat(0.03)),
    ));
    commands.spawn((
        Mesh3d(sphere_mesh.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            metallic: 0.0,
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.3, 0.3, 0.0).with_scale(Vec3::splat(0.03)),
    ));

    // Terrain
    commands.spawn((
        Terrain,
        SceneRoot(
            asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/terrain/terrain.glb")),
        ),
        Transform::from_xyz(0.0, 0.1, 0.0)
            .with_scale(Vec3::splat(0.5))
            .with_rotation(Quat::from_rotation_y(-0.1)),
    ));

    // Ocean
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(100.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: palettes::tailwind::BLUE_200.into(),
            perceptual_roughness: 0.0,
            cull_mode: None,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.1, 0.0),
    ));
}

fn dynamic_scene(
    mut suns: Query<(&mut Transform, Has<Sun>), With<DirectionalLight>>,
    mut skyboxes: Query<&mut Skybox>,
    mut cams: Query<(&mut Transform, &mut ColorGrading), (Without<DirectionalLight>, With<Camera>)>,
    time: Res<Time>,
) {
    let day_length_s = 60.0;
    // Pause for a moment before animating to let things load, start a bit before sunrise:
    let t = (time.elapsed_secs() - 1.0).max(0.0) + day_length_s * 0.8;
    let earth_tilt_rad = PI / 3.0;
    let day_fract = ((t % day_length_s) / day_length_s).clamp(0.0, 1.0);

    suns.iter_mut().for_each(|(mut tf, is_sun)| {
        let moon_offset = if is_sun { 0.0 } else { 1.1 };
        tf.rotation = Quat::from_euler(
            EulerRot::ZYX,
            earth_tilt_rad,
            0.0,
            -day_fract * PI * 2.0 + moon_offset * PI,
        );
    });
    for mut skybox in &mut skyboxes {
        let rot = Quat::from_euler(EulerRot::ZYX, -earth_tilt_rad, 0.0, day_fract * PI * 2.0);
        skybox.rotation = rot;
    }
    for (mut cam_transform, mut grading) in &mut cams {
        cam_transform.translation.z = -1.0 + 0.5 * ops::sin(-day_fract * PI * 2.0);
        grading.global.temperature = (ops::sin(day_fract * PI * 2.0) * 0.05).min(0.0);
        grading.global.post_saturation = (1.0 + ops::sin(day_fract * PI * 2.0) * 0.5).min(1.0);
    }
}
