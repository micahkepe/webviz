#![expect(clippy::needless_pass_by_value, reason = "Bevy ECS signatures.")]

use clap::Parser;
use std::{
    f32::consts::PI,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
};
use url::Url;
use wiki_scraper::ParsedPage;

fn setup_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight { illuminance: 10_000.0, ..default() },
        Transform::from_xyz(3.0, 3.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight { intensity: 500_000.0, ..default() },
        Transform::from_xyz(2.0, 2.0, 2.0),
    ));
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 10.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[derive(Resource)]
struct GraphFile(PathBuf);

#[derive(Component)]
struct Page {
    url: Url,
}

/// Ran at initial load to ingest all the parsed pages.
fn ingest_pages(
    source: Res<GraphFile>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok(file) = std::fs::File::open(&source.0) else {
        error!("unable to open file");
        return;
    };
    let reader = BufReader::new(file);
    let pages: Vec<ParsedPage> = reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<ParsedPage>(&line).ok())
        .collect();
    let mesh = meshes.add(Sphere::new(0.5));
    let material = materials.add(Color::srgb(0.994, 0.419, 0.518));
    for page in pages {
        commands.spawn((
            Page { url: page.url().clone() },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(
                rand::random_range(-100.0..100.0),
                rand::random_range(-100.0..100.0),
                rand::random_range(-100.0..100.0),
            ),
        ));
    }
}

fn handle_camera_movement(
    mut query: Query<(&mut Transform, &mut Projection), With<Camera3d>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    drag: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
) {
    let Ok((mut transform, mut projection)) = query.single_mut() else {
        return;
    };

    // Zoom via scroll.
    if let Projection::Perspective(ref mut perspective) = *projection {
        perspective.fov = (-scroll.delta.y).mul_add(0.01, perspective.fov);
        perspective.fov = perspective.fov.clamp(0.1, PI / 2.0);
    }

    // Orbit with left click mouse drag.
    if mouse.pressed(MouseButton::Left) {
        let sensitivity = 0.005;
        transform.rotate_y(drag.delta.x * sensitivity);
        transform.rotate_local_x(-drag.delta.y * sensitivity);
        return;
    }

    let mut direction = Vec3::ZERO;

    // Pan with right click mouse drag.
    if mouse.pressed(MouseButton::Right) {
        let sensitivity = 0.05;
        let right = *transform.right();
        let up = *transform.up();
        transform.translation += right * drag.delta.x * sensitivity;
        transform.translation -= up * drag.delta.y * sensitivity;
    } else {
        // Get all button input.
        let pressed = keyboard.get_pressed();
        if pressed.len() == 0 {
            return;
        }

        keyboard.get_pressed().for_each(|key| match *key {
            KeyCode::ArrowUp => direction += *transform.forward(),
            KeyCode::ArrowDown => direction += *transform.back(),
            KeyCode::ArrowLeft => direction += *transform.left(),
            KeyCode::ArrowRight => direction += *transform.right(),
            _ => {}
        });
        transform.translation += direction.normalize_or_zero();
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Graph input file.
    input: PathBuf,
}

fn main() {
    let args = Args::parse();
    App::new()
        .insert_resource(GraphFile(args.input))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup_camera, setup_light, ingest_pages))
        .add_systems(Update, handle_camera_movement)
        .run();
}
