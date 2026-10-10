use crate::simulation::components::*;
use crate::simulation::resources::*;
use bevy::math::DVec3;
use bevy::prelude::*;

pub fn spawn_galactic_collision_scenario(
    commands: &mut Commands,
    disk_params: &mut DiskParameters,
) -> Entity {
    // We setup two supermassive black holes representing the centers of the two galaxies
    // Andromeda (M31) and Milky Way analog collision.

    disk_params.central_star_mass = 4_300_000.0; // Milky way SMBH mass (approx 4.3M M_sun)
    disk_params.disk_mass = 140_000_000.0; // Managed by custom n-body logic if needed, or set to 0 to prevent default protoplanetary disk
    disk_params.inner_radius_au = 1000.0;
    disk_params.outer_radius_au = 100_000.0;
    disk_params.gas_disk_lifetime_yr = 0.0;

    // Milky Way SMBH
    let mw_smbh = commands
        .spawn((
            CelestialBody {
                name: "Milky Way SMBH (Sagittarius A*)".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(disk_params.central_star_mass),
            Radius(0.1),
            SimPosition(DVec3::ZERO),
            SimVelocity(DVec3::ZERO),
            CentralStar,
            Temperature(0.0),
            Luminosity(0.0),
            IgnitionState::default(),
            Composition::default(),
        ))
        .id();

    // Andromeda SMBH incoming
    let m31_mass = 140_000_000.0; // Andromeda SMBH is much larger
    commands.spawn((
        CelestialBody {
            name: "Andromeda SMBH".to_string(),
            body_type: BodyType::BlackHole,
        },
        Mass(m31_mass),
        Radius(0.5),
        SimPosition(DVec3::new(2_500_000.0, 50_000.0, 1_000_000.0)), // Distances in AU, highly scaled down for visual demo, real is 2.5 Mly -> ~1.58e11 AU
        // We'll give it an inward velocity
        SimVelocity(DVec3::new(-100.0, -2.0, -40.0)),
        Temperature(0.0),
        Luminosity(0.0),
        IgnitionState::default(),
        Composition::default(),
    ));

    mw_smbh
}
