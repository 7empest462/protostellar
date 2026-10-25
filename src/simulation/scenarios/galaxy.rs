use crate::simulation::components::*;
use crate::simulation::relativity::kerr::KerrBlackHole;
use crate::simulation::relativity::types::RelativisticState;
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
    disk_params.outer_radius_au = 65_000.0;
    disk_params.gas_disk_lifetime_yr = 0.0;

    // Milky Way SMBH
    let mw_smbh = commands
        .spawn((
            CentralStar,
            CelestialBody {
                name: "Milky Way SMBH (Sagittarius A*)".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(disk_params.central_star_mass),
            Radius(0.085),
            SimPosition(DVec3::new(-45_000.0, -6_000.0, -22_500.0)),
            SimVelocity(DVec3::new(90.0, 12.0, 45.0)),
            SimAcceleration::default(),
            Temperature(1e-7),
            Luminosity(100.0),
            IgnitionState::default(),
            Composition::pure_hydrogen(),
            AngularMomentum::default(),
            RelativisticState::default(),
            SpinState::default(),
        ))
        .insert(KerrBlackHole::sagittarius_a())
        .id();

    // Andromeda SMBH incoming
    let m31_mass = 140_000_000.0; // Andromeda SMBH is much larger
    commands
        .spawn((
            CelestialBody {
                name: "Andromeda SMBH".to_string(),
                body_type: BodyType::BlackHole,
            },
            Mass(m31_mass),
            Radius(2.76),
            SimPosition(DVec3::new(45_000.0, 6_000.0, 22_500.0)),
            SimVelocity(DVec3::new(-90.0, -12.0, -45.0)),
            SimAcceleration::default(),
            Temperature(1e-7),
            Luminosity(500.0),
            IgnitionState::default(),
            Composition::pure_hydrogen(),
            AngularMomentum::default(),
            RelativisticState::default(),
            SpinState::default(),
        ))
        .insert(KerrBlackHole::new(m31_mass, 0.85, DVec3::Y));

    mw_smbh
}
