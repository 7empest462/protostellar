use bevy::prelude::*;

use crate::simulation::components::*;
use crate::utils::constants::*;

/// Maps body types to photosphere shader stellar subtype indices.
pub fn star_subtype_from_body_type(body_type: BodyType) -> f32 {
    match body_type {
        BodyType::RedDwarf => 1.0,
        BodyType::BrownDwarf => 2.0,
        BodyType::RedGiant | BodyType::RedSupergiant => 3.0,
        BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => 4.0,
        BodyType::NeutronStar => 5.0,
        BodyType::Pulsar => 6.0,
        BodyType::Magnetar => 7.0,
        BodyType::WhiteDwarf => 8.0,
        BodyType::Protostar => 9.0,
        BodyType::WolfRayet => 10.0,
        _ => 0.0,
    }
}

/// Computes realistic astrophysical color palette for Stars and Stellar Remnants.
pub fn compute_stellar_palette(body_type: BodyType, temp_k: f64) -> Color {
    match body_type {
        BodyType::BlackHole => Color::srgb(0.01, 0.01, 0.01),
        BodyType::QuasiStar => Color::srgb(1.0, 0.10, 0.02),
        BodyType::BrownDwarf => Color::srgb(0.48, 0.12, 0.32),
        BodyType::NeutronStar => Color::srgb(0.72, 0.86, 1.0),
        BodyType::Pulsar => Color::srgb(0.65, 0.78, 1.0),
        BodyType::Magnetar => Color::srgb(0.72, 0.45, 1.0),
        BodyType::WhiteDwarf => Color::srgb(0.85, 0.92, 1.0),
        BodyType::WolfRayet => Color::srgb(0.95, 0.45, 0.85),
        BodyType::BlueGiant | BodyType::BlueSupergiant | BodyType::Hypergiant => {
            Color::srgb(0.68, 0.82, 1.0)
        }
        BodyType::RedGiant | BodyType::RedSupergiant => Color::srgb(0.95, 0.22, 0.04),
        BodyType::RedDwarf => Color::srgb(1.0, 0.42, 0.12),
        BodyType::Protostar => Color::srgb(0.95, 0.58, 0.15),

        _ => {
            let (br, bg, bb) = blackbody_to_srgb(temp_k);
            Color::srgb(br, bg, bb)
        }
    }
}

/// Computes realistic astrophysical color palette for Gas Giants based on
/// mass tier (Jupiter vs Super-Jupiter vs Brown Dwarf) and equilibrium temperature.
/// Computes specialized atmospheric palettes for Ice Giants (Uranus, Neptune, Sub-Neptunes).
pub fn compute_ice_giant_palette(name: &str, _temp_k: f64) -> Color {
    let lower = name.to_lowercase();
    if lower.contains("uranus") {
        Color::srgb(0.42, 0.76, 0.82) // Pale aquamarine cyan/teal
    } else if lower.contains("neptune") {
        Color::srgb(0.18, 0.48, 0.88) // Deep vivid cobalt azure
    } else if lower.contains("nine") {
        Color::srgb(0.14, 0.32, 0.58) // Deep abyssal navy / indigo
    } else {
        Color::srgb(0.24, 0.58, 0.86) // Luminous celestial azure-cyan
    }
}

pub fn compute_gas_giant_palette(mass_solar: f64, temp_k: f64, name: &str) -> Color {
    let mass_jup = mass_solar / crate::utils::constants::JUPITER_MASS_SOLAR;
    let lower = name.to_lowercase();

    // 0. Ice Giant Fallback
    if lower.contains("neptune") {
        return Color::srgb(0.18, 0.48, 0.88); // Deep vivid cobalt azure
    }
    if lower.contains("uranus") {
        return Color::srgb(0.42, 0.76, 0.82); // Pale aquamarine cyan/teal
    }

    // 1. Saturn Preset
    if lower.contains("saturn") {
        return Color::srgb(0.92, 0.82, 0.58); // Butterscotch golden-sand
    }

    // 2. Hot Jupiter (Sudarsky Class IV/V: Alkali / Silicate cloud hazes)
    if temp_k > 800.0 || lower.contains("hot jupiter") {
        return Color::srgb(0.38, 0.16, 0.10); // Fiery carbonaceous bronze/amber
    }

    // 3. Named Jupiter preset or standard 1.0 M_jup
    if (lower.contains("jupiter") && !lower.contains("super") && !lower.contains("hot"))
        || (0.7..=1.8).contains(&mass_jup)
    {
        return Color::srgb(0.86, 0.65, 0.42); // Iconic Jovian amber-ochre
    }

    // 4. Super-Jupiters by mass variations:
    if mass_jup > 12.0 {
        // Brown Dwarf Transition: Incandescent plum-maroon & dark violet
        Color::srgb(0.45, 0.12, 0.32)
    } else if mass_jup > 6.0 {
        // Heavy Super-Jupiter (6-12 M_jup): Royal Plum-Purple with Midnight Navy belts
        Color::srgb(0.32, 0.20, 0.48)
    } else if mass_jup > 3.5 {
        // Massive Super-Jupiter (3.5-6 M_jup): Deep Lapis-Indigo and Sapphire-Cyan
        Color::srgb(0.16, 0.36, 0.62)
    } else if mass_jup > 1.8 {
        // Super-Jupiter (1.8-3.5 M_jup): Exotic Emerald-Teal & Aquamarine
        Color::srgb(0.18, 0.52, 0.50)
    } else if mass_jup < 0.6 {
        // Sub-Saturn / Warm Gas Dwarf: Pale Cream-Straw
        Color::srgb(0.85, 0.78, 0.56)
    } else {
        // Standard Jupiter size: Classic Jovian ochre-amber
        Color::srgb(0.86, 0.65, 0.42)
    }
}

/// Calculates ring albedo and RGB color based on volatile composition.
pub fn calc_ring_color(ice_fraction: f32) -> Vec4 {
    if ice_fraction >= 0.70 {
        // High ice fraction (>= 70%): brilliant silver-white (Saturn-like)
        Vec4::new(0.96, 0.97, 1.0, 0.95)
    } else if ice_fraction >= 0.35 {
        // Mixed ice & dust (35-70%): warm sand-cream tone
        Vec4::new(0.85, 0.78, 0.68, 0.85)
    } else {
        // Silicate / carbonaceous (< 35%): dark anthracite / charcoal (Uranus / Jovian-like)
        Vec4::new(0.38, 0.35, 0.32, 0.65)
    }
}

pub fn compute_asteroid_spectral_palette(name: &str, comp: &Composition) -> Color {
    let lower = name.to_lowercase();
    if lower.contains("psyche") {
        return Color::srgb(0.48, 0.47, 0.49);
    } else if lower.contains("vesta") {
        return Color::srgb(0.44, 0.42, 0.36);
    } else if lower.contains("ceres") {
        return Color::srgb(0.12, 0.12, 0.13);
    } else if lower.contains("pallas")
        || lower.contains("hygiea")
        || lower.contains("mathilde")
        || lower.contains("bennu")
        || lower.contains("ryugu")
    {
        return Color::srgb(0.09, 0.095, 0.105);
    } else if lower.contains("ida") || lower.contains("gaspra") || lower.contains("eros") {
        return Color::srgb(0.34, 0.28, 0.20);
    }

    let norm = comp.normalized();
    if norm.metal_frac > 0.40 {
        Color::srgb(0.45, 0.44, 0.46)
    } else if norm.organics_frac > 0.18 {
        Color::srgb(0.24, 0.15, 0.11)
    } else if norm.silicate_frac > 0.60 {
        let hash = name.bytes().fold(0usize, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as usize)
        });
        if hash.is_multiple_of(3) {
            Color::srgb(0.40, 0.38, 0.34)
        } else {
            Color::srgb(0.32, 0.26, 0.19)
        }
    } else if norm.ice_frac > 0.15 {
        Color::srgb(0.16, 0.19, 0.23)
    } else {
        Color::srgb(0.10, 0.10, 0.11)
    }
}

pub fn compute_comet_spectral_palette(name: &str, _comp: &Composition) -> Color {
    let lower = name.to_lowercase();
    if lower.contains("67p") || lower.contains("arrokoth") {
        Color::srgb(0.062, 0.045, 0.038)
    } else if lower.contains("hale-bopp") || lower.contains("swift") {
        Color::srgb(0.045, 0.055, 0.070)
    } else if lower.contains("halley") || lower.contains("tempel") {
        Color::srgb(0.048, 0.042, 0.038)
    } else {
        let hash = name.bytes().fold(0usize, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as usize)
        });
        match hash % 4 {
            0 => Color::srgb(0.040, 0.040, 0.042),
            1 => Color::srgb(0.058, 0.044, 0.036),
            2 => Color::srgb(0.042, 0.052, 0.065),
            _ => Color::srgb(0.052, 0.046, 0.038),
        }
    }
}

pub fn compute_terrestrial_body_palette(
    name: &str,
    comp: &Composition,
    temp_k: f64,
    opt_tidal: Option<&crate::simulation::tides::TidalState>,
) -> Color {
    let lower = name.to_lowercase();
    let norm = comp.normalized();

    if lower == "the moon" || lower == "moon" || lower.contains("the moon") {
        Color::srgb(0.52, 0.52, 0.54)
    } else if lower.contains("mercury") {
        Color::srgb(0.32, 0.33, 0.35)
    } else if lower.contains("mars") {
        Color::srgb(0.66, 0.36, 0.20)
    } else if lower.contains("theia") {
        Color::srgb(0.42, 0.39, 0.35)
    } else if lower.contains("venus") {
        Color::srgb(0.46, 0.40, 0.32)
    } else if lower.contains("titan") {
        Color::srgb(0.72, 0.48, 0.22)
    } else if lower.contains("callisto") || lower.contains("ganymede") {
        Color::srgb(0.30, 0.32, 0.36)
    } else if lower.contains("europa") {
        Color::srgb(0.88, 0.86, 0.82)
    } else {
        let tidal_heating = opt_tidal.map_or(0.0, |t| t.tidal_heating_flux_w_m2);
        if tidal_heating > 0.4
            || (temp_k > 420.0 && norm.silicate_frac > 0.6 && norm.gas_frac < 0.05)
        {
            let hash = name.bytes().fold(0usize, |acc, b| {
                acc.wrapping_mul(31).wrapping_add(b as usize)
            });
            if hash.is_multiple_of(2) {
                Color::srgb(0.74, 0.64, 0.26)
            } else {
                Color::srgb(0.35, 0.28, 0.22)
            }
        } else if norm.metal_frac > 0.38 {
            Color::srgb(0.42, 0.44, 0.48)
        } else if norm.organics_frac > 0.15 {
            Color::srgb(0.26, 0.20, 0.16)
        } else if norm.ice_frac > 0.30 {
            Color::srgb(0.55, 0.62, 0.70)
        } else {
            let hash = name.bytes().fold(0usize, |acc, b| {
                acc.wrapping_mul(31).wrapping_add(b as usize)
            });
            match hash % 6 {
                0 => Color::srgb(0.52, 0.52, 0.54),
                1 => Color::srgb(0.28, 0.28, 0.30),
                2 => Color::srgb(0.62, 0.38, 0.22),
                3 => Color::srgb(0.38, 0.42, 0.34),
                4 => Color::srgb(0.54, 0.46, 0.34),
                _ => Color::srgb(0.36, 0.38, 0.42),
            }
        }
    }
}
