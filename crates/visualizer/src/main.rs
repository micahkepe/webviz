#![expect(clippy::needless_pass_by_value, reason = "Bevy ECS signatures.")]

use clap::Parser;
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
};

use bevy::prelude::*;
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
    mut query: Query<&mut Transform, With<Camera3d>>,
    input: Res<ButtonInput<KeyCode>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    // Get all button input.
    let mut transform_delta = Transform::default();
    let pressed = input.get_pressed();
    if pressed.len() == 0 {
        return;
    }
    input.get_pressed().for_each(|key| match *key {
        KeyCode::ArrowUp => transform_delta.translation.y += 1.0,
        KeyCode::ArrowDown => transform_delta.translation.y -= 1.0,
        KeyCode::ArrowLeft => transform_delta.translation.x -= 1.0,
        KeyCode::ArrowRight => transform_delta.translation.x += 1.0,
        _ => {}
    });

    // Normalize the transform vector.
    transform_delta.translation =
        transform_delta.translation.normalize_or_zero();

    // Apply.
    transform.translation += transform_delta.translation;
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
