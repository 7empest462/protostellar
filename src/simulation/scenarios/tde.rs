use crate::rendering::bodies::VisualBody;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use bevy::prelude::*;

pub fn spawn_tde_scenario(commands: &mut Commands, disk_params: &mut DiskParameters) -> Entity {
    let bh_mass = 1_000_000.0;
    disk_params.central_star_mass = bh_mass;
    disk_params.inner_radius_au = 0.1;
    disk_params.outer_radius_au = 25.0;
    disk_params.disk_mass = 0.0; // Clean disk

    // Spawn the Supermassive Black Hole
    let bh_entity = commands
        .spawn((
            CelestialBody {
                name: "TDE Supermassive Black Hole".into(),
                body_type: BodyType::BlackHole,
            },
            Mass(bh_mass),
            Radius((1.974e-8 * bh_mass).max(1e-7)), // Schwarzschild radius
            SimPosition(bevy::math::DVec3::ZERO),
            SimVelocity(bevy::math::DVec3::ZERO),
            VisualBody,
            PointLight {
                color: Color::WHITE,
                intensity: 0.0, // Dark until accretion!
                range: 500.0,
                shadow_maps_enabled: true,
                ..default()
            },
        ))
        .id();

    // Spawn a doomed Red Giant
    let star_mass = 2.0;
    let star_radius = 45.0; // Large puffed up radius makes it very vulnerable to tidal shear

    // We will place the star on a highly elliptical orbit with pericenter < r_tidal
    // Let pericenter = 5.0 AU, apocenter = 400.0 AU
    let r_p = 5.0;
    let r_a = 400.0;
    let a = f64::midpoint(r_p, r_a);

    // Vis-viva equation for velocity at apocenter
    let g = 39.478; // G in AU, M_solar, yr
    let v_apocenter = (g * bh_mass * (2.0 / r_a - 1.0 / a)).sqrt();

    commands.spawn((
        CelestialBody {
            name: "Doomed Red Giant".into(),
            body_type: BodyType::RedGiant,
        },
        Mass(star_mass),
        Radius(star_radius),
        SimPosition(bevy::math::DVec3::new(r_a, 0.0, 0.0)),
        SimVelocity(bevy::math::DVec3::new(0.0, 0.0, v_apocenter)),
        VisualBody,
        Luminosity(100.0),
        Temperature(3500.0),
        Composition::pure_hydrogen(),
    ));

    bh_entity
}
