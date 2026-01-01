//! Lighting test headless

use bevy::app::AppExit;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy_render::view::screenshot::{save_to_disk, Screenshot};
use bevy_render::view::Hdr;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(GlobalAmbientLight::NONE)
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (capture_screenshot, exit_after_frames))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sphere_mesh = meshes.add(Sphere::new(1.0).mesh().uv(128, 64));
    let surface_material = materials.add(StandardMaterial {
        // Set RGB to 0 to disable diffuse, otherwise use 0.5
        base_color: Color::linear_rgba(0.5, 0.5, 0.5, 1.0),
        // Set reflectance to 0 to disable specular, otherwise use 0.5
        reflectance: 0.5,
        perceptual_roughness: 0.5, // use 0.089 or 0.5
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(100.0, 100.0))),
        MeshMaterial3d(surface_material.clone()),
        Transform::from_xyz(-50.0, 0.0, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(100.0, 100.0))),
        MeshMaterial3d(surface_material.clone()),
        Transform::from_xyz(50.0, 0.0, 0.0),
    ));
    commands.spawn((
        Transform::from_xyz(-2.5, 1.0, 3.0),
        Mesh3d(sphere_mesh.clone()),
        MeshMaterial3d(surface_material.clone()),
    ));
    commands.spawn((
        Transform::from_xyz(0.0, 2.5, 3.0),
        Mesh3d(sphere_mesh.clone()),
        MeshMaterial3d(surface_material.clone()),
    ));
    commands.spawn((
        PointLight {
            intensity: 80000.0,
            radius: 1.0,
            ..default()
        },
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 45.2f32.to_radians(),
            ..default()
        }),
        Tonemapping::None,
        Transform::from_xyz(0.0, 1.0, 7.0).looking_at(Vec3::ZERO, Vec3::Y),
        Hdr,
    ));
}

fn capture_screenshot(mut commands: Commands, mut counter: Local<u32>) {
    if *counter % 60 == 20 {
        let filename = format!("screenshot-ltc-{}.png", *counter / 60);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(filename));
    }
    *counter += 1;
}

fn exit_after_frames(mut counter: Local<u32>, mut app_exit_writer: MessageWriter<AppExit>) {
    if *counter == 100 {
        app_exit_writer.write(AppExit::Success);
    }
    *counter += 1;
}
