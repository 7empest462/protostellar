use bevy::prelude::*;

use crate::simulation::components::*;
use crate::utils::constants::*;

/// Maps body types to photosphere shader stellar subtype indices.
pub fn star_subtype_from_body_type(body_type: BodyType) -> f32 {
    match body_type {
        BodyType::RedDwarf => 1.0,
        BodyType::BrownDwarf => 2.0,
        BodyType::RedGiant | BodyType::RedSupergiant => 3.0,
        BodyType::BlueMainSequence
        | BodyType::BlueGiant
        | BodyType::BlueSupergiant
        | BodyType::Hypergiant => 4.0,
        BodyType::NeutronStar => 5.0,
        BodyType::Pulsar => 6.0,
        BodyType::Magnetar => 7.0,
        BodyType::WhiteDwarf => 8.0,
        BodyType::Protostar | BodyType::TTauriStar => 9.0,
        BodyType::WolfRayet => 10.0,
        BodyType::DarkStar => 11.0,
        BodyType::BosonStar => 12.0,
        BodyType::OrangeDwarf => 13.0,
        BodyType::YellowWhiteDwarf => 14.0,
        BodyType::WhiteMainSequence => 15.0,
        BodyType::HerbigAeBeStar => 16.0,
        BodyType::YellowHypergiant => 17.0,
        BodyType::CarbonStar => 18.0,
        BodyType::LuminousBlueVariable => 19.0,
        BodyType::PopulationIIIStar => 20.0,
        BodyType::BlueStraggler => 21.0,
        BodyType::QuarkStar => 22.0,
        BodyType::ThorneZytkow => 23.0,
        BodyType::BlackDwarf => 24.0,
        BodyType::IronStar => 25.0,
        _ => 0.0,
    }
}

/// Computes realistic astrophysical color palette for Stars and Stellar Remnants.
pub fn compute_stellar_palette(body_type: BodyType, temp_k: f64) -> Color {
    match body_type {
        BodyType::BlackHole => Color::srgb(0.01, 0.01, 0.01),
        BodyType::QuasiStar => Color::srgb(1.0, 0.10, 0.02),
        BodyType::BrownDwarf => Color::srgb(0.48, 0.12, 0.32),
        BodyType::NeutronStar | BodyType::BlueMainSequence => Color::srgb(0.72, 0.86, 1.0),
        BodyType::Pulsar => Color::srgb(0.65, 0.78, 1.0),
        BodyType::Magnetar => Color::srgb(0.72, 0.45, 1.0),
        BodyType::QuarkStar => Color::srgb(0.85, 0.72, 1.0), // Hard X-ray violet-white
        BodyType::DarkStar => Color::srgb(1.0, 0.60, 0.18),  // Puffy golden-amber
        BodyType::BosonStar => Color::srgb(0.35, 0.85, 0.95), // Shimmering cyan soliton
        BodyType::IronStar => Color::srgb(0.35, 0.38, 0.42), // Gunmetal cold iron
        BodyType::ThorneZytkow => Color::srgb(0.95, 0.18, 0.08), // Pulsing deep crimson
        BodyType::WhiteDwarf => Color::srgb(0.85, 0.92, 1.0),
        BodyType::BlackDwarf => Color::srgb(0.06, 0.06, 0.09), // Cold diamond
        BodyType::WolfRayet => Color::srgb(0.95, 0.45, 0.85),
        BodyType::CarbonStar => Color::srgb(0.85, 0.12, 0.05), // Soot-shrouded ruby
        BodyType::YellowHypergiant => Color::srgb(1.0, 0.78, 0.25),
        BodyType::LuminousBlueVariable => Color::srgb(0.60, 0.85, 1.0),
        BodyType::PopulationIIIStar => Color::srgb(0.55, 0.75, 1.0), // Blazing UV-blue
        BodyType::BlueStraggler => Color::srgb(0.65, 0.85, 1.0),
        BodyType::OrangeDwarf => Color::srgb(1.0, 0.65, 0.22),
        BodyType::YellowWhiteDwarf => Color::srgb(1.0, 0.96, 0.82),
        BodyType::WhiteMainSequence => Color::srgb(0.96, 0.98, 1.0),
        BodyType::TTauriStar => Color::srgb(1.0, 0.52, 0.16),
        BodyType::HerbigAeBeStar => Color::srgb(0.78, 0.88, 1.0),
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

    // 3. Super-Jupiters and Brown Dwarfs by mass variations:
    if mass_jup > 12.0 {
        // Brown Dwarf Transition: Incandescent plum-maroon & dark violet
        return Color::srgb(0.45, 0.12, 0.32);
    } else if mass_jup > 6.0 {
        // Heavy Super-Jupiter (6-12 M_jup): Royal Plum-Purple with Midnight Navy belts
        return Color::srgb(0.32, 0.20, 0.48);
    } else if mass_jup > 3.5 {
        // Massive Super-Jupiter (3.5-6 M_jup): Deep Lapis-Indigo and Sapphire-Cyan
        return Color::srgb(0.16, 0.36, 0.62);
    } else if mass_jup > 1.8 {
        // Super-Jupiter (1.8-3.5 M_jup): Exotic Emerald-Teal & Aquamarine
        return Color::srgb(0.18, 0.52, 0.50);
    }

    // 4. Named Jupiter preset
    if lower.contains("jupiter") && !lower.contains("super") && !lower.contains("hot") {
        return Color::srgb(0.86, 0.65, 0.42); // Iconic Jovian amber-ochre
    }

    // 5. Sudarsky Classification for Standard Jovians (< 1.8 M_jup):
    // Class III: Cloudless Azure / Rayleigh / Methane (360 - 800 K)
    if temp_k >= 360.0 {
        return Color::srgb(0.18, 0.42, 0.74);
    }
    // Class II: Water Cloud Jovian (180 - 360 K)
    if temp_k >= 180.0 {
        return Color::srgb(0.82, 0.85, 0.88);
    }

    // Class I: Ammonia Cloud Jovian (< 180 K, e.g. Jupiter, Saturn, Planet-5AU at 107 K):
    if mass_jup < 0.6 {
        // Sub-Saturn / Warm Gas Dwarf: Pale Cream-Straw
        Color::srgb(0.88, 0.80, 0.58)
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
        Color::srgb(0.162, 0.145, 0.138)
    } else if lower.contains("hale-bopp") || lower.contains("swift") {
        Color::srgb(0.145, 0.155, 0.170)
    } else if lower.contains("halley") || lower.contains("tempel") {
        Color::srgb(0.148, 0.142, 0.138)
    } else {
        let hash = name.bytes().fold(0usize, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as usize)
        });
        match hash % 4 {
            0 => Color::srgb(0.140, 0.140, 0.142),
            1 => Color::srgb(0.158, 0.144, 0.136),
            2 => Color::srgb(0.142, 0.152, 0.165),
            _ => Color::srgb(0.152, 0.146, 0.138),
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
    } else if lower.contains("io") {
        Color::srgb(0.88, 0.78, 0.28)
    } else if lower.contains("enceladus") {
        Color::srgb(0.96, 0.98, 1.0)
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

/// Calculates the fraction of solid crust covering a magma ocean at temperature `temp_k`.
///
/// Returns 0.0 for completely molten oceans (>= 1800 K), 0.5 around 1125 K,
/// and 1.0 for fully solidified crust (<= 450 K).
pub fn compute_magma_ocean_crust_fraction(temp_k: f32) -> f32 {
    (1.0 - (temp_k - 450.0) / 1350.0).clamp(0.0, 1.0)
}

/// Computes the blackbody incandescence color and thermal emissive intensity
/// for magma at a given temperature and rift fissure intensity.
pub fn compute_magma_incandescence(temp_k: f32, fissure_intensity: f32) -> (Vec3, f32) {
    let core_incandescent = Vec3::new(1.0, 0.95, 0.65);
    let molten_orange = Vec3::new(1.0, 0.42, 0.07);
    let deep_crimson = Vec3::new(0.55, 0.09, 0.03);

    let rift_heat = (fissure_intensity * 1.6 - 0.4).clamp(0.0, 1.0);
    let high_t = ((temp_k - 1200.0) / 800.0).clamp(0.0, 1.0);
    let top_tone = molten_orange.lerp(core_incandescent, high_t);
    let lava_tone = deep_crimson.lerp(top_tone, rift_heat);

    let glow_scale = ((temp_k - 450.0) / 450.0).clamp(0.0, 6.0);
    (lava_tone, glow_scale)
}

/// Computes the base albedo/emissive color for any celestial body from its physical properties.
pub fn compute_body_color(
    body: &CelestialBody,
    mass: &Mass,
    temp: &Temperature,
    comp: &Composition,
    opt_tidal: Option<&crate::simulation::tides::TidalState>,
) -> Color {
    let lower = body.name.to_lowercase();
    let is_star_like = body.body_type.is_star_or_remnant();
    let is_ice_giant = body.body_type == BodyType::IceGiant
        || lower.contains("uranus")
        || lower.contains("neptune");
    let is_gas_giant = !is_ice_giant
        && (body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.45);
    if is_star_like {
        compute_stellar_palette(body.body_type, temp.0)
    } else if is_ice_giant {
        compute_ice_giant_palette(&body.name, temp.0)
    } else if is_gas_giant {
        compute_gas_giant_palette(mass.0, temp.0, &body.name)
    } else if matches!(body.body_type, BodyType::Asteroid | BodyType::Planetesimal) {
        compute_asteroid_spectral_palette(&body.name, comp)
    } else if body.body_type == BodyType::Comet {
        compute_comet_spectral_palette(&body.name, comp)
    } else {
        compute_terrestrial_body_palette(&body.name, comp, temp.0, opt_tidal)
    }
}
