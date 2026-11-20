//! Celestial world sorting, classification, and hierarchy utilities for the HUD.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::disk::belts::BeltZone;
use crate::simulation::probes::{ProbeStatus, ProbeType, SpaceProbe};
use crate::simulation::scenarios::{ActiveScenarioState, ScenarioPreset};
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::types::{QuickBarState, SystemWorld};

pub(crate) use crate::utils::math::{
    contains_ignore_ascii_case, eq_any_ignore_ascii_case, starts_with_ignore_ascii_case,
};

fn is_canonical_moon_name(name: &str) -> bool {
    const CANONICAL_MOONS: &[&str] = &[
        "io",
        "europa",
        "ganymede",
        "callisto",
        "titan",
        "enceladus",
        "triton",
        "charon",
    ];
    let trimmed = name.trim();
    if eq_any_ignore_ascii_case(trimmed, &["moon", "the moon", "luna"]) {
        return true;
    }
    CANONICAL_MOONS.iter().any(|&m| {
        trimmed.eq_ignore_ascii_case(m)
            || (starts_with_ignore_ascii_case(trimmed, m)
                && trimmed
                    .get(m.len()..)
                    .is_some_and(|rest| rest.starts_with(" (")))
    })
}

/// Returns true if a celestial body name corresponds to one of the canonical major planets
/// (Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune, Pluto, Planet Nine).
pub fn is_canonical_major_planet(name: &str) -> bool {
    const MAJOR_PLANETS: &[&str] = &[
        "mercury",
        "venus",
        "earth",
        "mars",
        "jupiter",
        "saturn",
        "uranus",
        "neptune",
        "pluto",
        "planet nine",
        "planet 9",
    ];
    if contains_ignore_ascii_case(name, "(moon)")
        || contains_ignore_ascii_case(name, "satellite")
        || contains_ignore_ascii_case(name, "super-jupiter")
    {
        return false;
    }
    MAJOR_PLANETS
        .iter()
        .any(|p| contains_ignore_ascii_case(name, p))
}

/// Determines if a celestial body is a protoplanetary embryo (e.g. Theia, Callisto Embryo, Titan Embryo,
/// or procedural protoplanets coalesced in planetary feeding zones to seed moons and planetary accretion).
pub fn is_embryo_body(name: &str, body_type: BodyType) -> bool {
    if contains_ignore_ascii_case(name, "theia") {
        return false;
    }
    if is_canonical_major_planet(name) {
        return false;
    }
    if contains_ignore_ascii_case(name, "embryo") {
        return true;
    }
    body_type == BodyType::Protoplanet
}

/// Determines if a celestial body is a major world (Star, Canonical Planet, Mature Planet, Named Moon)
/// or a generic procedural minor planetesimal / asteroid / embryo.
pub fn is_major_body(name: &str, body_type: BodyType, is_star: bool, mass_solar: f64) -> bool {
    const EXOTIC_MAJOR: &[&str] = &["trappist", "kepler", "nemesis", "rogue", "host star"];
    if is_star || body_type.is_star_or_remnant() {
        return true;
    }
    if body_type == BodyType::Moon {
        return is_canonical_moon_name(name) || mass_solar >= (EARTH_MASS_SOLAR * 0.005);
    }
    if contains_ignore_ascii_case(name, "theia") {
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
    if starts_with_ignore_ascii_case(name, "asteroid-")
        || starts_with_ignore_ascii_case(name, "dust-")
        || starts_with_ignore_ascii_case(name, "debris-")
        || starts_with_ignore_ascii_case(name, "comet-")
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
    if EXOTIC_MAJOR
        .iter()
        .any(|tag| contains_ignore_ascii_case(name, tag))
    {
        return true;
    }
    mass_solar >= (EARTH_MASS_SOLAR * 0.0001)
}

/// Collects and deterministically sorts all active celestial bodies into a continuous 0..N numerical sequence:
/// - [0] is ALWAYS the Central Star.
/// - [1..N] are all orbiting worlds (companions, black holes, super-earths, gas giants, embryos),
///   sorted strictly from innermost to outermost by distance from the star.
pub fn collect_sorted_system_worlds<'a, I>(items: I, sort_by_mass: bool) -> Vec<SystemWorld>
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
            let entry = SystemWorld {
                entity: ent,
                name: body.name.clone(),
                body_type: body.body_type,
                is_central_star: central_star.is_none(),
                distance_au: dist,
                mass_solar: mass.0,
                radius_au: radius.0,
                index: 0,
            };
            if central_star.is_none() {
                central_star = Some(entry);
            } else {
                other_worlds.push(entry);
            }
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

    if sort_by_mass {
        other_worlds.sort_by(|a, b| {
            b.mass_solar
                .partial_cmp(&a.mass_solar)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    } else {
        other_worlds.sort_by(|a, b| {
            a.distance_au
                .partial_cmp(&b.distance_au)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

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

/// Query signature used to snapshot celestial bodies for quick-bar change detection.
pub type QuickBarBodiesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CelestialBody,
        &'static SimPosition,
        &'static Mass,
        &'static Radius,
        Option<&'static CentralStar>,
    ),
    With<CelestialBody>,
>;

/// Fingerprint of the celestial bodies, probes, and filter settings driving the quick-bar UI.
#[derive(Clone, PartialEq, Eq)]
pub struct QuickBarFingerprint {
    body_count: usize,
    body_xor: u64,
    body_sum: u64,
    probe_count: usize,
    probe_xor: u64,
    probe_sum: u64,
    is_minimized: bool,
    show_embryos: bool,
    show_minor_bodies: bool,
    sort_by_mass: bool,
    expanded_belts: Vec<BeltZone>,
    filter_text: String,
    scenario: Option<ScenarioPreset>,
}

fn fnv1a_bytes(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

pub(crate) fn compute_quick_bar_fingerprint(
    bodies_query: &QuickBarBodiesQuery,
    probes_query: &Query<(Entity, &SpaceProbe, &CelestialBody)>,
    quick_bar_state: &QuickBarState,
    opt_scenario: Option<&ActiveScenarioState>,
) -> QuickBarFingerprint {
    let mut body_count = 0usize;
    let mut body_xor = 0u64;
    let mut body_sum = 0u64;
    for (ent, body, _pos, mass, _rad, opt_star) in bodies_query.iter() {
        let name_hash = fnv1a_bytes(body.name.as_bytes());
        let mass_hash = if quick_bar_state.sort_by_mass {
            u64::from((mass.0.max(0.0) as f32).to_bits() >> 10)
        } else {
            0
        };
        let tag = ent.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ ((body.body_type as u64) << 32)
            ^ ((u64::from(opt_star.is_some())) << 48)
            ^ mass_hash.wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ name_hash;
        body_count += 1;
        body_xor ^= tag;
        body_sum = body_sum.wrapping_add(tag);
    }

    let mut probe_count = 0usize;
    let mut probe_xor = 0u64;
    let mut probe_sum = 0u64;
    for (ent, probe, body) in probes_query.iter() {
        let tag = ent.to_bits().wrapping_mul(0x517C_C1B7_2722_0A95)
            ^ ((probe.probe_type as u64) << 16)
            ^ ((probe.status as u64) << 24)
            ^ fnv1a_bytes(body.name.as_bytes());
        probe_count += 1;
        probe_xor ^= tag;
        probe_sum = probe_sum.wrapping_add(tag);
    }

    QuickBarFingerprint {
        body_count,
        body_xor,
        body_sum,
        probe_count,
        probe_xor,
        probe_sum,
        is_minimized: quick_bar_state.is_minimized,
        show_embryos: quick_bar_state.show_embryos,
        show_minor_bodies: quick_bar_state.show_minor_bodies,
        sort_by_mass: quick_bar_state.sort_by_mass,
        expanded_belts: quick_bar_state.expanded_belts.iter().copied().collect(),
        filter_text: quick_bar_state.filter_text.clone(),
        scenario: opt_scenario.map(|s| s.current_preset),
    }
}

/// Tuple entry describing an embryo or minor body in the quick selector bar.
pub type QuickBarMinorEntry = (Entity, String, BodyType, bool, f64, f64);

/// Tuple entry describing an active space probe in the quick selector bar.
pub type QuickBarProbeEntry = (Entity, ProbeType, ProbeStatus, String);

pub(crate) fn retain_filtered_quick_bar_items(
    filter: &str,
    major_worlds: &mut Vec<SystemWorld>,
    embryo_bodies: &mut Vec<QuickBarMinorEntry>,
    minor_bodies: &mut Vec<QuickBarMinorEntry>,
    probes: &mut Vec<QuickBarProbeEntry>,
) {
    if filter.is_empty() {
        return;
    }
    major_worlds.retain(|w| contains_ignore_ascii_case(&w.name, filter));
    embryo_bodies.retain(|b| contains_ignore_ascii_case(&b.1, filter));
    minor_bodies.retain(|b| contains_ignore_ascii_case(&b.1, filter));
    probes.retain(|(_, _, _, name)| contains_ignore_ascii_case(name, filter));
}

pub(crate) fn should_defer_quick_bar_hover_rebuild(
    pointer_on_quick_bar: bool,
    quick_bar_changed: bool,
    toggled_hotkey: bool,
    prev_entities: &[Entity],
    current_entities: &[Entity],
) -> bool {
    if !pointer_on_quick_bar || quick_bar_changed || toggled_hotkey || prev_entities.is_empty() {
        return false;
    }
    let mut prev_set = prev_entities.to_vec();
    let mut curr_set = current_entities.to_vec();
    prev_set.sort_unstable();
    curr_set.sort_unstable();
    prev_set == curr_set
}
