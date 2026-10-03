use crate::simulation::components::celestial::classify_body_by_mass_and_comp;
use crate::simulation::components::{BodyType, CelestialBody, Composition, Mass};
use bevy::prelude::*;

/// Dynamically updates the names of auto-generated bodies (those containing '#')
/// so their names reflect their current physical mass classification rather than their spawn state.
pub fn update_auto_generated_names(
    mut query: Query<
        (&mut CelestialBody, &Mass, &Composition),
        Without<crate::simulation::probes::SpaceProbe>,
    >,
) {
    for (mut body, mass, comp) in query.iter_mut() {
        if let Some(hash_idx) = body.name.find('#') {
            let Some(number_str) = body.name.get(hash_idx..) else {
                continue;
            };
            let is_captured = body.name.starts_with("Captured ");

            // Re-evaluate what it REALLY is physically, ignoring if it's currently marked as a Moon.
            let true_type = classify_body_by_mass_and_comp(mass.0, comp, false);

            let ideal_prefix = match true_type {
                BodyType::Asteroid => "Asteroid",
                BodyType::Comet => "Comet",
                BodyType::Planetesimal => "Planetesimal",
                BodyType::Protoplanet => "Proto-Planet",
                BodyType::TerrestrialPlanet => "Terrestrial Planet",
                BodyType::SuperEarth => "Super-Earth",
                BodyType::GasGiant => "Gas Giant",
                BodyType::IceGiant => "Ice Giant",
                BodyType::BrownDwarf => "Brown Dwarf",
                BodyType::RedDwarf => "Red Dwarf",
                BodyType::WhiteDwarf => "White Dwarf",
                BodyType::NeutronStar => "Neutron Star",
                BodyType::Pulsar => "Pulsar",
                BodyType::Magnetar => "Magnetar",
                BodyType::BlackHole => "Black Hole",
                _ => "Body",
            };

            let expected_name = if is_captured {
                format!("Captured {ideal_prefix} {number_str}")
            } else {
                format!("{ideal_prefix} {number_str}")
            };

            if body.name != expected_name {
                body.name = expected_name;
            }
        }
    }
}
