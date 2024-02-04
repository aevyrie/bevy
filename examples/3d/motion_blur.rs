//! Demonstrates how to enable per-object motion blur. This rendering feature can be configured per
//! camera using the [`MotionBlur`] component.z

use bevy::{
    core_pipeline::motion_blur::{MotionBlur, MotionBlurBundle},
    prelude::*,
};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup_camera, setup_scene, setup_ui))
        .add_systems(Update, (update_settings, move_cars, update_cam).chain())
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle::default(),
        // Add the MotionBlurBundle to a camera to enable motion blur.
        // Motion blur requires the depth and motion vector prepass, which this bundle adds.
        // Configure the amount and quality of motion blur per-camera using this component.
        MotionBlurBundle::default(),
    ));
}

// Everything past this point is used to build the example, but is not required for usage.

#[derive(Component)]
struct Moves(f32);

#[derive(Component)]
struct CameraTracked;

#[derive(Component)]
struct Rotates;

fn setup_scene(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 2_000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::default().looking_to(Vec3::new(-1.0, -0.7, -1.0), Vec3::X),
        ..default()
    });
    // Sky
    commands.spawn(PbrBundle {
        mesh: meshes.add(shape::UVSphere::default()),
        material: materials.add(StandardMaterial {
            unlit: true,
            base_color: Color::rgb(0.3, 0.8, 1.0),
            ..default()
        }),
        transform: Transform::default().with_scale(Vec3::splat(-4000.0)),
        ..default()
    });
    // Ground
    commands.spawn(PbrBundle {
        mesh: meshes.add(shape::Plane::default()),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.3, 0.5, 0.25),
            perceptual_roughness: 1.0,
            ..default()
        }),
        transform: Transform::from_xyz(0.0, -0.65, 0.0).with_scale(Vec3::splat(100.0)),
        ..default()
    });
    commands.spawn(SceneBundle {
        scene: asset_server.load("models/terrain/Mountains.gltf#Scene0"),
        transform: Transform::from_scale(Vec3::new(4000.0, 800.0, 4000.0))
            .with_translation(Vec3::new(0.0, -2.0, 0.0)),
        ..default()
    });

    // Cars

    let box_mesh = meshes.add(shape::Box::new(0.3, 0.15, 0.55));
    let cylinder = meshes.add(shape::Cylinder::default());
    let logo = asset_server.load("branding/icon.png");
    let wheel_matl = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(logo.clone()),
        ..default()
    });

    let colors = [
        materials.add(Color::RED),
        materials.add(Color::YELLOW),
        materials.add(Color::BLACK),
        materials.add(Color::BLUE),
        materials.add(Color::GREEN),
        materials.add(Color::PURPLE),
        materials.add(Color::BEIGE),
        materials.add(Color::ORANGE),
    ];

    for i in 0..40 {
        let color = colors[i % colors.len()].clone();
        let mut entity = commands.spawn((
            PbrBundle {
                mesh: box_mesh.clone(),
                material: color.clone(),
                ..default()
            },
            Moves(i as f32),
        ));
        if i == 0 {
            entity.insert(CameraTracked);
        }
        entity.with_children(|parent| {
            parent.spawn(PbrBundle {
                mesh: box_mesh.clone(),
                material: color,
                transform: Transform::from_xyz(0.0, 0.08, 0.03)
                    .with_scale(Vec3::new(1.0, 1.0, 0.5)),
                ..default()
            });
            let mut spawn_wheel = |x: f32, z: f32| {
                parent.spawn((
                    PbrBundle {
                        mesh: cylinder.clone(),
                        material: wheel_matl.clone(),
                        transform: Transform::from_xyz(0.14 * x, -0.045, 0.15 * z)
                            .with_scale(Vec3::new(0.15, 0.05, 0.15))
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2)),
                        ..default()
                    },
                    Rotates,
                ));
            };
            spawn_wheel(1.0, 1.0);
            spawn_wheel(1.0, -1.0);
            spawn_wheel(-1.0, 1.0);
            spawn_wheel(-1.0, -1.0);
        });
    }

    // Trees

    let capsule = meshes.add(shape::Capsule::default());
    let sphere = meshes.add(shape::UVSphere::default());
    let leaves = materials.add(Color::GREEN);
    let trunk = materials.add(Color::rgb(0.4, 0.2, 0.2));
    let n_trees = 50;
    for theta in 0..n_trees * 4 {
        let theta = theta as f32 * 1.3;
        let dist = 40.0 + theta * 0.3;
        let x = (theta / n_trees as f32 * 2.0 * std::f32::consts::PI).sin() * dist;
        let z = (theta / n_trees as f32 * 2.0 * std::f32::consts::PI).cos() * dist;
        commands.spawn(PbrBundle {
            mesh: sphere.clone(),
            material: leaves.clone(),
            transform: Transform::from_xyz(x + 3.0, 0.8, z + 3.0),
            ..default()
        });
        commands.spawn(PbrBundle {
            mesh: capsule.clone(),
            material: trunk.clone(),
            transform: Transform::from_xyz(x + 3.0, -0.4, z + 3.0)
                .with_scale(Vec3::new(0.3, 2.0, 0.3)),
            ..default()
        });
    }
}

fn setup_ui(mut commands: Commands) {
    let style = TextStyle {
        font_size: 20.0,
        ..default()
    };
    commands.spawn(
        TextBundle::from_sections(vec![
            TextSection::new(String::new(), style.clone()),
            TextSection::new(String::new(), style.clone()),
            TextSection::new("\n1/2 - Decrease/Increase shutter angle\n", style.clone()),
            TextSection::new("3/4 - Decrease/Increase sample count\n", style.clone()),
        ])
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        }),
    );
}

fn update_settings(
    mut settings: Query<&mut MotionBlur>,
    presses: Res<ButtonInput<KeyCode>>,
    mut text: Query<&mut Text>,
) {
    let mut settings = settings.single_mut();
    if presses.just_pressed(KeyCode::Digit1) {
        settings.shutter_angle -= 0.1;
    } else if presses.just_pressed(KeyCode::Digit2) {
        settings.shutter_angle += 0.1;
    } else if presses.just_pressed(KeyCode::Digit3) {
        settings.samples = settings.samples.saturating_sub(1);
    } else if presses.just_pressed(KeyCode::Digit4) {
        settings.samples += 1;
    }
    settings.shutter_angle = settings.shutter_angle.clamp(0.0, 1.0);
    settings.samples = settings.samples.clamp(0, 64);
    let mut text = text.single_mut();
    text.sections[0].value = format!("Shutter angle: {:.5}\n", settings.shutter_angle);
    text.sections[1].value = format!("Samples: {:.5}\n", settings.samples);
}

fn move_cars(
    time: Res<Time>,
    mut movables: Query<(&mut Transform, &Moves, &Children)>,
    mut spins: Query<&mut Transform, (Without<Moves>, With<Rotates>)>,
) {
    for (mut transform, moves, children) in &mut movables {
        let time = time.elapsed_seconds() * 0.3;
        let t = time + 0.5 * moves.0;
        let t = t + t.sin() * 0.5 + 0.5;
        let prev = transform.translation;
        transform.translation.x = (1.0 * t).sin() * 10.0;
        transform.translation.z = (3.0 * t).cos() * 10.0;
        transform.translation.y = -0.53;
        let delta = transform.translation - prev;
        transform.look_to(delta, Vec3::Y);
        for child in children.iter() {
            let Ok(mut wheel) = spins.get_mut(*child) else {
                continue;
            };
            let radius = wheel.scale.x;
            let circumference = 2.0 * std::f32::consts::PI * radius;
            let angle = delta.length() / circumference * std::f32::consts::PI * 2.0;
            wheel.rotate_local_y(angle);
        }
    }
}

fn update_cam(
    mut camera: Query<(&mut Transform, &mut Projection, &mut Camera)>,
    tracked: Query<&Transform, (With<CameraTracked>, Without<Camera>)>,
) {
    let tracked = tracked.single();
    let (mut transform, mut projection, mut camera) = camera.single_mut();
    transform.look_at(tracked.translation, Vec3::Y);
    if let Projection::Perspective(perspective) = &mut *projection {
        perspective.fov = 0.3;
    }
    camera.hdr = true;
}
