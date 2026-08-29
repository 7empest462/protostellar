//! Deep space environment, starfield background, and ambient cosmic lighting.

use bevy::prelude::*;

pub fn setup_space_environment(mut commands: Commands) {
    // Ambient cosmic illumination (dim background starlight)
    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.08, 0.09, 0.14),
        brightness: 120.0,
        ..default()
    });

    // Distant galactic directional light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.92, 0.95, 1.0),
            illuminance: 350.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.8, 0.0)),
    ));
}
