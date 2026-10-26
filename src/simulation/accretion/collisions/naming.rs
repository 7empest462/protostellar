//! Body nomenclature resolution for collision mergers, galactic collisions, and canonical identities.

use crate::simulation::components::BodyType;

/// Returns true if a celestial body has a canonical solar system, galactic center,
/// or benchmark exoplanet identity that should not be overwritten by generic merger labels.
pub fn is_canonical_named_body(name: &str) -> bool {
    const CANONICAL_KEYS: &[&str] = &[
        "earth",
        "theia",
        "moon",
        "luna",
        "mercury",
        "venus",
        "mars",
        "jupiter",
        "saturn",
        "uranus",
        "neptune",
        "pluto",
        "sun",
        "sol",
        "sagittarius",
        "sgr a",
        "cygnus",
        "m87",
        "trappist",
        "kepler",
        "pulsar",
        "lich",
        "draugr",
        "poltergeist",
        "phobetor",
        "magnetar",
        "sgr 1806",
        "pistol star",
        "quasar",
        "little red dot",
        "tde supermassive",
    ];
    let lower = name.to_lowercase();
    CANONICAL_KEYS.iter().any(|k| lower.contains(k))
}

/// Updates the name of a merged primary celestial body according to physical classification regimes,
/// ensuring genuine Andromeda-Milky Way collisions produce Milkomeda while preserving canonical names
/// (such as Sagittarius A*) and providing realistic labels for procedural mergers.
pub fn update_merged_body_name(
    name: &mut String,
    secondary_name: &str,
    updated_type: BodyType,
    total_mass: f64,
) {
    // 1. Explicit galactic collision merger between Andromeda and Milky Way SMBHs
    let is_milkomeda_merger = (name.contains("Milky Way") && secondary_name.contains("Andromeda"))
        || (name.contains("Andromeda") && secondary_name.contains("Milky Way"))
        || name.contains("Milkomeda")
        || secondary_name.contains("Milkomeda");

    if is_milkomeda_merger && updated_type == BodyType::BlackHole {
        *name = format!(
            "Merged Supermassive Black Hole (Milkomeda SMBH - {:.1}M M☉)",
            total_mass / 1_000_000.0
        );
        return;
    }

    // 2. Canonical solar system, galactic center, and exoplanet names are preserved
    if is_canonical_named_body(name) {
        return;
    }

    // 3. Exotic stellar remnants
    if updated_type == BodyType::ThorneZytkow {
        *name = format!("Thorne-Żytkow Object ({total_mass:.1} M☉)");
        return;
    }
    if updated_type == BodyType::QuarkStar {
        *name = format!("Strange Quark Star ({total_mass:.2} M☉)");
        return;
    }
    if updated_type == BodyType::BlueStraggler {
        *name = format!("Blue Straggler Star ({total_mass:.1} M☉)");
        return;
    }

    // 4. Black hole mass tier classifications for generic/procedural bodies
    if updated_type == BodyType::BlackHole {
        if total_mass >= 1_000_000.0 {
            *name = format!(
                "Galactic Nucleus (Supermassive Black Hole - {:.1}M M☉)",
                total_mass / 1_000_000.0
            );
        } else if total_mass >= 40.0 {
            *name = format!("Galactic Nucleus (SMBH Seed - {total_mass:.1} M☉)");
        } else if total_mass >= 15.0 {
            *name = format!("Intermediate-Mass Black Hole ({total_mass:.1} M☉)");
        } else {
            *name = format!("Stellar-Mass Black Hole ({total_mass:.1} M☉)");
        }
        return;
    }

    // 5. Planetary classifications
    let is_named_minor = name.contains("Comet") || name.contains("Asteroid");
    if updated_type.is_planet() && (is_named_minor || !name.starts_with("Planet")) {
        *name = match updated_type {
            BodyType::TerrestrialPlanet => "Planet (Terrestrial World)".to_string(),
            BodyType::SuperEarth => "Planet (Super-Earth)".to_string(),
            BodyType::GasGiant => "Planet (Gas Giant)".to_string(),
            BodyType::IceGiant => "Planet (Ice Giant)".to_string(),
            BodyType::Protoplanet => "Protoplanet (Embryo)".to_string(),
            _ => name.clone(),
        };
    }
}
