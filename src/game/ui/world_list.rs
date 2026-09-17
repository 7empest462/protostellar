//! Celestial world sorting, classification, and hierarchy utilities for the HUD.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::types::SystemWorld;

/// Returns true if a celestial body name corresponds to one of the canonical major planets
/// (Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune, Pluto, Planet Nine).
pub fn is_canonical_major_planet(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("mercury")
        || lower.contains("venus")
        || lower.contains("earth")
        || lower.contains("mars")
        || lower.contains("jupiter")
        || lower.contains("saturn")
        || lower.contains("uranus")
        || lower.contains("neptune")
        || lower.contains("pluto")
        || lower.contains("planet nine")
        || lower.contains("planet 9")
}

/// Determines if a celestial body is a protoplanetary embryo (e.g. Theia, Callisto Embryo, Titan Embryo,
/// or procedural protoplanets coalesced in planetary feeding zones to seed moons and planetary accretion).
pub fn is_embryo_body(name: &str, body_type: BodyType) -> bool {
    let lower = name.to_lowercase();
    if lower.contains("theia") {
        return false;
    }
    if is_canonical_major_planet(name) {
        return false;
    }
    if lower.contains("embryo") {
        return true;
    }
    body_type == BodyType::Protoplanet
}

/// Determines if a celestial body is a major world (Star, Canonical Planet, Mature Planet, Named Moon)
/// or a generic procedural minor planetesimal / asteroid / embryo.
pub fn is_major_body(name: &str, body_type: BodyType, is_star: bool, mass_solar: f64) -> bool {
    if is_star || body_type.is_star_or_remnant() {
        return true;
    }
    let lower = name.to_lowercase();
    if lower.contains("theia") {
        return true;
    }
    if is_canonical_major_planet(name) {
        return true;
    }
    if matches!(
        body_type,
        BodyType::Asteroid
            | BodyType::Comet
            | BodyType::Planetesimal
            | BodyType::DustGrain
            | BodyType::DebrisRing
    ) {
        return false;
    }
    if lower.starts_with("asteroid-")
        || lower.starts_with("dust-")
        || lower.starts_with("debris-")
        || lower.starts_with("comet-")
    {
        return false;
    }
    if is_embryo_body(name, body_type) {
        return false;
    }
    if body_type == BodyType::GasGiant
        || body_type == BodyType::IceGiant
        || body_type == BodyType::SuperEarth
        || body_type == BodyType::TerrestrialPlanet
        || body_type == BodyType::Moon
        || body_type == BodyType::QuasiStar
    {
        return true;
    }
    if lower.contains("trappist")
        || lower.contains("kepler")
        || lower.contains("nemesis")
        || lower.contains("rogue")
        || lower.contains("host star")
    {
        return true;
    }
    mass_solar >= (EARTH_MASS_SOLAR * 0.0001)
}

/// Collects and deterministically sorts all active celestial bodies into a continuous 0..N numerical sequence:
/// - [0] is ALWAYS the Central Star.
/// - [1..N] are all orbiting worlds (companions, black holes, super-earths, gas giants, embryos),
///   sorted strictly from innermost to outermost by distance from the star.
pub fn collect_sorted_system_worlds<'a, I>(items: I) -> Vec<SystemWorld>
where
    I: IntoIterator<
        Item = (
            Entity,
            &'a CelestialBody,
            &'a SimPosition,
            &'a Mass,
            &'a Radius,
            Option<&'a CentralStar>,
        ),
    >,
{
    let mut central_star: Option<SystemWorld> = None;
    let mut other_worlds: Vec<SystemWorld> = Vec::new();

    for (ent, body, pos, mass, radius, opt_star) in items {
        let dist = if pos.0.is_finite() {
            pos.0.length()
        } else {
            0.0
        };
        let is_star_component = opt_star.is_some();

        if is_star_component {
            central_star = Some(SystemWorld {
                entity: ent,
                name: body.name.clone(),
                body_type: body.body_type,
                is_central_star: true,
                distance_au: dist,
                mass_solar: mass.0,
                radius_au: radius.0,
                index: 0,
            });
            continue;
        }

        if is_major_body(
            &body.name,
            body.body_type,
            body.body_type.is_star_or_remnant(),
            mass.0,
        ) {
            other_worlds.push(SystemWorld {
                entity: ent,
                name: body.name.clone(),
                body_type: body.body_type,
                is_central_star: false,
                distance_au: dist,
                mass_solar: mass.0,
                radius_au: radius.0,
                index: 0,
            });
        }
    }

    if central_star.is_none() && !other_worlds.is_empty() {
        let mut best_idx = 0;
        let mut best_dist = f64::MAX;
        for (i, w) in other_worlds.iter().enumerate() {
            let score = if w.body_type.is_star_or_remnant() {
                w.distance_au
            } else {
                w.distance_au + 1000.0
            };
            if score < best_dist {
                best_dist = score;
                best_idx = i;
            }
        }
        let mut cs = other_worlds.remove(best_idx);
        cs.is_central_star = true;
        central_star = Some(cs);
    }

    other_worlds.sort_by(|a, b| {
        a.distance_au
            .partial_cmp(&b.distance_au)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut result = Vec::with_capacity(1 + other_worlds.len());
    if let Some(mut cs) = central_star {
        cs.index = 0;
        result.push(cs);
    }
    for (i, mut w) in other_worlds.into_iter().enumerate() {
        w.index = i + 1;
        result.push(w);
    }

    result
}
