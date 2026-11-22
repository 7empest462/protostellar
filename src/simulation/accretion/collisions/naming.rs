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
    if crate::utils::math::contains_any_ignore_ascii_case(name, &["super-jupiter", "(moon)"]) {
        return false;
    }
    crate::utils::math::contains_any_ignore_ascii_case(name, CANONICAL_KEYS)
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

    // 5. Sub-stellar Brown Dwarf & Stellar class upgrades (replaces planetary placeholder names)
    if updated_type == BodyType::BrownDwarf {
        let mass_mj = total_mass / crate::utils::constants::JUPITER_MASS_SOLAR;
        if !name.contains("Substellar") && !name.contains("Embryo") {
            *name = format!("Brown Dwarf ({mass_mj:.1} M_J)");
        }
        return;
    }
    if updated_type.is_star_or_remnant() && !updated_type.is_remnant() {
        if updated_type.is_evolved_giant() {
            if updated_type == BodyType::RedSupergiant && !name.contains("Supergiant") {
                *name = "The Star (Red Supergiant)".to_string();
            } else if matches!(
                updated_type,
                BodyType::Hypergiant | BodyType::YellowHypergiant
            ) && !name.contains("Hypergiant")
            {
                *name = "The Star (Luminous Yellow Hypergiant)".to_string();
            }
            return;
        }
        let (_, label) = crate::simulation::components::celestial::classify_stellar_type_and_label(
            total_mass,
            &crate::simulation::components::Composition::solar_gas(),
        );
        crate::simulation::thermodynamics::stellar_lifecycle::update_promoted_stellar_name(
            name,
            updated_type,
            total_mass,
            label,
        );
        return;
    }

    // 6. Planetary classifications
    let is_named_minor = name.contains("Comet") || name.contains("Asteroid");
    let is_giant_reclass = name.starts_with("Super-Jupiter") && updated_type != BodyType::GasGiant;
    if updated_type.is_planet()
        && (is_named_minor || is_giant_reclass || !name.starts_with("Planet"))
    {
        *name = match updated_type {
            BodyType::TerrestrialPlanet => "Planet (Terrestrial World)".to_string(),
            BodyType::SuperEarth => {
                let mass_mj = total_mass / crate::utils::constants::JUPITER_MASS_SOLAR;
                if mass_mj >= 0.2 {
                    format!("Mega-Earth ({mass_mj:.1} M_J)")
                } else {
                    "Planet (Super-Earth)".to_string()
                }
            }
            BodyType::GasGiant => {
                let mass_mj = total_mass / crate::utils::constants::JUPITER_MASS_SOLAR;
                if mass_mj >= 1.0 {
                    format!("Super-Jupiter ({mass_mj:.1} M_J)")
                } else {
                    "Planet (Gas Giant)".to_string()
                }
            }
            BodyType::IceGiant => "Planet (Ice Giant)".to_string(),
            BodyType::Protoplanet => "Protoplanet (Embryo)".to_string(),
            _ => name.clone(),
        };
    }
}

/// Strips trailing mass or classification parenthetical tags (such as `" (1.5 M_J)"`
/// or `" (Gas Giant)"`) from a parent planet's name when deriving satellite names.
pub fn clean_moon_parent_name(parent_name: &str) -> String {
    let mut trimmed = parent_name.trim();
    while trimmed.ends_with(')') {
        let Some(open_idx) = trimmed.rfind(" (") else {
            break;
        };
        let Some(prefix) = trimmed.get(..open_idx) else {
            break;
        };
        let candidate = prefix.trim();
        if candidate.is_empty() {
            break;
        }
        trimmed = candidate;
    }
    trimmed.to_string()
}

/// Formats a positive integer as a Roman numeral for sequential moon designations (`I`, `II`, ...).
pub fn format_roman_numeral(num: usize) -> String {
    const NUMERALS: &[(usize, &str)] = &[
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut remaining = num.max(1);
    let mut out = String::new();
    for &(value, symbol) in NUMERALS {
        while remaining >= value {
            out.push_str(symbol);
            remaining -= value;
        }
    }
    out
}

/// Resolves a clean satellite designation (`"The Moon"` for Earth+Theia or `"{Parent} {Roman} (Moon)"`).
pub fn resolve_satellite_moon_name(
    parent_name: &str,
    secondary_name: &str,
    existing_satellites: usize,
) -> String {
    let is_canonical_earth_theia = (parent_name.contains("Earth")
        && !parent_name.contains("Super-Earth")
        && !parent_name.contains("Planet Nine")
        && !parent_name.contains("Venus"))
        && (secondary_name.contains("Theia") || secondary_name.contains("Moon"));
    if is_canonical_earth_theia && existing_satellites == 0 {
        return "The Moon".to_string();
    }
    let base = clean_moon_parent_name(parent_name);
    let roman = format_roman_numeral(existing_satellites + 1);
    format!("{base} {roman} (Moon)")
}
