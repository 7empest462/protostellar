use bevy::prelude::*;

use crate::simulation::components::*;
use crate::utils::constants::*;
use crate::utils::math::contains_ascii_case_insensitive;

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

/// Computes specialized atmospheric palettes for Ice Giants (Uranus, Neptune, Sub-Neptunes).
pub fn compute_ice_giant_palette(name: &str, _temp_k: f64) -> Color {
    if contains_ascii_case_insensitive(name, "uranus") {
        Color::srgb(0.42, 0.76, 0.82) // Pale aquamarine cyan/teal
    } else if contains_ascii_case_insensitive(name, "neptune") {
        Color::srgb(0.18, 0.48, 0.88) // Deep vivid cobalt azure
    } else if contains_ascii_case_insensitive(name, "nine") {
        Color::srgb(0.14, 0.32, 0.58) // Deep abyssal navy / indigo
    } else {
        Color::srgb(0.24, 0.58, 0.86) // Luminous celestial azure-cyan
    }
}

pub fn compute_gas_giant_palette(mass_solar: f64, temp_k: f64, name: &str) -> Color {
    let mass_jup = mass_solar / crate::utils::constants::JUPITER_MASS_SOLAR;

    // 0. Ice Giant Fallback
    if contains_ascii_case_insensitive(name, "neptune") {
        return Color::srgb(0.18, 0.48, 0.88); // Deep vivid cobalt azure
    }
    if contains_ascii_case_insensitive(name, "uranus") {
        return Color::srgb(0.42, 0.76, 0.82); // Pale aquamarine cyan/teal
    }

    // 1. Saturn Preset
    if contains_ascii_case_insensitive(name, "saturn") {
        return Color::srgb(0.92, 0.82, 0.58); // Butterscotch golden-sand
    }

    // 2. Hot Jupiter (Sudarsky Class IV/V: Alkali / Silicate cloud hazes)
    if temp_k > 800.0 || contains_ascii_case_insensitive(name, "hot jupiter") {
        return Color::srgb(0.38, 0.16, 0.10); // Fiery carbonaceous bronze/amber
    }

    // 3. Super-Jupiters and Brown Dwarfs by mass variations:
    if mass_jup > 12.0 {
        return Color::srgb(0.45, 0.12, 0.32);
    } else if mass_jup > 6.0 {
        return Color::srgb(0.32, 0.20, 0.48);
    } else if mass_jup > 3.5 {
        return Color::srgb(0.16, 0.36, 0.62);
    } else if mass_jup > 1.8 {
        return Color::srgb(0.18, 0.52, 0.50);
    }

    // 4. Named Jupiter preset
    if contains_ascii_case_insensitive(name, "jupiter")
        && !contains_ascii_case_insensitive(name, "super")
        && !contains_ascii_case_insensitive(name, "hot")
    {
        return Color::srgb(0.86, 0.65, 0.42); // Iconic Jovian amber-ochre
    }

    // 5. Sudarsky Classification for Standard Jovians (< 1.8 M_jup):
    if temp_k >= 360.0 {
        return Color::srgb(0.18, 0.42, 0.74);
    }
    if temp_k >= 180.0 {
        return Color::srgb(0.82, 0.85, 0.88);
    }

    if mass_jup < 0.6 {
        Color::srgb(0.88, 0.80, 0.58)
    } else {
        Color::srgb(0.86, 0.65, 0.42)
    }
}

/// Calculates ring albedo and RGB color based on volatile composition.
pub fn calc_ring_color(ice_fraction: f32) -> Vec4 {
    if ice_fraction >= 0.70 {
        Vec4::new(0.96, 0.97, 1.0, 0.95)
    } else if ice_fraction >= 0.35 {
        Vec4::new(0.85, 0.78, 0.68, 0.85)
    } else {
        Vec4::new(0.38, 0.35, 0.32, 0.65)
    }
}

pub fn compute_asteroid_spectral_palette(name: &str, comp: &Composition) -> Color {
    if contains_ascii_case_insensitive(name, "psyche") {
        return Color::srgb(0.48, 0.47, 0.49);
    } else if contains_ascii_case_insensitive(name, "vesta") {
        return Color::srgb(0.44, 0.42, 0.36);
    } else if contains_ascii_case_insensitive(name, "ceres") {
        return Color::srgb(0.12, 0.12, 0.13);
    } else if contains_ascii_case_insensitive(name, "pallas")
        || contains_ascii_case_insensitive(name, "hygiea")
        || contains_ascii_case_insensitive(name, "mathilde")
        || contains_ascii_case_insensitive(name, "bennu")
        || contains_ascii_case_insensitive(name, "ryugu")
    {
        return Color::srgb(0.09, 0.095, 0.105);
    } else if contains_ascii_case_insensitive(name, "ida")
        || contains_ascii_case_insensitive(name, "gaspra")
        || contains_ascii_case_insensitive(name, "eros")
    {
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
    if contains_ascii_case_insensitive(name, "67p")
        || contains_ascii_case_insensitive(name, "arrokoth")
    {
        Color::srgb(0.162, 0.145, 0.138)
    } else if contains_ascii_case_insensitive(name, "hale-bopp")
        || contains_ascii_case_insensitive(name, "swift")
    {
        Color::srgb(0.145, 0.155, 0.170)
    } else if contains_ascii_case_insensitive(name, "halley")
        || contains_ascii_case_insensitive(name, "tempel")
    {
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
    let norm = comp.normalized();

    if name.eq_ignore_ascii_case("the moon")
        || name.eq_ignore_ascii_case("moon")
        || contains_ascii_case_insensitive(name, "the moon")
    {
        Color::srgb(0.52, 0.52, 0.54)
    } else if contains_ascii_case_insensitive(name, "mercury") {
        Color::srgb(0.32, 0.33, 0.35)
    } else if contains_ascii_case_insensitive(name, "mars") {
        Color::srgb(0.66, 0.36, 0.20)
    } else if contains_ascii_case_insensitive(name, "theia") {
        Color::srgb(0.42, 0.39, 0.35)
    } else if contains_ascii_case_insensitive(name, "venus") {
        Color::srgb(0.46, 0.40, 0.32)
    } else if contains_ascii_case_insensitive(name, "titan") {
        Color::srgb(0.72, 0.48, 0.22)
    } else if contains_ascii_case_insensitive(name, "callisto")
        || contains_ascii_case_insensitive(name, "ganymede")
    {
        Color::srgb(0.30, 0.32, 0.36)
    } else if contains_ascii_case_insensitive(name, "europa") {
        Color::srgb(0.88, 0.86, 0.82)
    } else if contains_ascii_case_insensitive(name, "io") {
        Color::srgb(0.88, 0.78, 0.28)
    } else if contains_ascii_case_insensitive(name, "enceladus") {
        Color::srgb(0.96, 0.98, 1.0)
    } else if contains_ascii_case_insensitive(name, "pluto") {
        // Pluto: Creamy nitrogen ivory (Sputnik Planitia) + russet tholin undertones
        Color::srgb(0.82, 0.68, 0.54)
    } else if contains_ascii_case_insensitive(name, "charon") {
        // Charon: Slate water-ice gray with faint reddish tholin cast
        Color::srgb(0.48, 0.46, 0.45)
    } else if contains_ascii_case_insensitive(name, "eris") {
        // Eris: Ultra-high albedo (0.96) pearlescent crystalline frost
        Color::srgb(0.94, 0.96, 0.98)
    } else if contains_ascii_case_insensitive(name, "haumea") {
        // Haumea: Crystalline water ice white
        Color::srgb(0.88, 0.90, 0.92)
    } else if contains_ascii_case_insensitive(name, "makemake") {
        // Makemake: Terracotta-red / burnt-orange tholin mantle
        Color::srgb(0.78, 0.45, 0.28)
    } else if contains_ascii_case_insensitive(name, "sedna") {
        // Sedna: Deep garnet / crimson hydrocarbon crust
        Color::srgb(0.68, 0.22, 0.16)
    } else if contains_ascii_case_insensitive(name, "ceres") {
        // Ceres: Dark carbonaceous clay with bright faculae
        Color::srgb(0.28, 0.28, 0.29)
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
            let hash = name.bytes().fold(0usize, |acc, b| {
                acc.wrapping_mul(31).wrapping_add(b as usize)
            });
            match hash % 4 {
                0 => Color::srgb(0.86, 0.88, 0.92), // Nitrogen/methane frost
                1 => Color::srgb(0.72, 0.52, 0.38), // Tholin-rich icy mantle
                2 => Color::srgb(0.58, 0.64, 0.72), // Dirty water ice bedrock
                _ => Color::srgb(0.78, 0.76, 0.74), // Mixed cryo-regolith
            }
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
    let is_star_like = body.body_type.is_star_or_remnant();
    let is_ice_giant = body.body_type == BodyType::IceGiant
        || contains_ascii_case_insensitive(&body.name, "uranus")
        || contains_ascii_case_insensitive(&body.name, "neptune");
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

/// Returns whether a celestial body emits or reflects enough optical starlight to produce
/// a distant point-spread Airy glint. Excludes dark compact remnants (black holes, black dwarfs,
/// cold iron stars, blown-out quasi-stars) and minor debris.
#[inline]
pub fn is_optically_luminous_or_reflective_glint_body(
    body_type: BodyType,
    is_blown_out_quasi: bool,
) -> bool {
    if matches!(
        body_type,
        BodyType::BlackHole | BodyType::BlackDwarf | BodyType::IronStar
    ) || (body_type == BodyType::QuasiStar && is_blown_out_quasi)
    {
        return false;
    }
    body_type.is_star_or_remnant()
        || body_type.is_planet()
        || body_type == BodyType::Moon
        || body_type == BodyType::Protoplanet
}

/// Computes physically grounded far-field optical photometry `(chromaticity, brightness_weight)`
/// for distant starlight/reflected-sunlight Airy glints without heap allocations.
pub fn compute_optical_glint_photometry(
    body: &CelestialBody,
    mass_solar: f64,
    temp_k: f64,
    comp: &Composition,
) -> (Color, f32) {
    if body.body_type.is_star_or_remnant() {
        let base = compute_stellar_palette(body.body_type, temp_k).to_srgba();
        let col = Color::srgb(
            (base.red * 0.85 + 0.15).min(1.0),
            (base.green * 0.85 + 0.15).min(1.0),
            (base.blue * 0.85 + 0.15).min(1.0),
        );
        return (col, 1.25);
    }

    let name = body.name.as_str();
    let is_ice_giant = body.body_type == BodyType::IceGiant
        || contains_ascii_case_insensitive(name, "uranus")
        || contains_ascii_case_insensitive(name, "neptune");
    if is_ice_giant {
        let c = compute_ice_giant_palette(name, temp_k).to_srgba();
        return (
            Color::srgb(
                (c.red * 0.8 + 0.20).min(1.0),
                (c.green * 0.8 + 0.22).min(1.0),
                (c.blue * 0.8 + 0.22).min(1.0),
            ),
            0.84,
        );
    }

    if body.body_type == BodyType::GasGiant || comp.normalized().gas_frac > 0.45 {
        let c = compute_gas_giant_palette(mass_solar, temp_k, name).to_srgba();
        return (
            Color::srgb(
                (c.red * 0.82 + 0.22).min(1.0),
                (c.green * 0.82 + 0.20).min(1.0),
                (c.blue * 0.82 + 0.18).min(1.0),
            ),
            0.96,
        );
    }

    if contains_ascii_case_insensitive(name, "earth") {
        // Carl Sagan's "Pale Blue Dot" (Rayleigh + ocean + cloud scatter)
        return (Color::srgb(0.62, 0.82, 1.0), 0.80);
    }
    if contains_ascii_case_insensitive(name, "venus") {
        // High-albedo sulfuric cloud deck ("Morning Star")
        return (Color::srgb(1.0, 0.95, 0.82), 0.90);
    }
    if contains_ascii_case_insensitive(name, "mars") {
        // Ferric iron-oxide regolith
        return (Color::srgb(0.98, 0.58, 0.36), 0.72);
    }
    if contains_ascii_case_insensitive(name, "mercury") {
        return (Color::srgb(0.80, 0.80, 0.83), 0.58);
    }
    if temp_k > 950.0 {
        return (Color::srgb(1.0, 0.56, 0.24), 0.82);
    }

    let norm = comp.normalized();
    let weight = if body.body_type == BodyType::Moon {
        0.50
    } else {
        0.70
    };
    if norm.ice_frac > 0.28 {
        (Color::srgb(0.78, 0.90, 1.0), weight + 0.06)
    } else if (240.0..=340.0).contains(&temp_k) && norm.ice_frac > 0.01 {
        (Color::srgb(0.64, 0.84, 1.0), weight + 0.08)
    } else {
        let c = compute_terrestrial_body_palette(name, comp, temp_k, None).to_srgba();
        (
            Color::srgb(
                (c.red * 0.75 + 0.32).min(1.0),
                (c.green * 0.75 + 0.30).min(1.0),
                (c.blue * 0.75 + 0.28).min(1.0),
            ),
            weight,
        )
    }
}

/// Resolves the GPU shader `planet_type` (0..=9) and `subtype` (encoded in `color_seed.w`)
/// for a celestial body based on its classification, composition, and identity.
pub fn resolve_planet_type_and_subtype(
    body_type: BodyType,
    name: &str,
    comp: &Composition,
) -> (u32, f32) {
    let norm = comp.normalized();
    let lower_pluto = contains_ascii_case_insensitive(name, "pluto");
    let lower_charon = contains_ascii_case_insensitive(name, "charon");
    let lower_eris = contains_ascii_case_insensitive(name, "eris");
    let lower_haumea = contains_ascii_case_insensitive(name, "haumea");
    let lower_makemake = contains_ascii_case_insensitive(name, "makemake");
    let lower_sedna = contains_ascii_case_insensitive(name, "sedna");
    let lower_ceres = contains_ascii_case_insensitive(name, "ceres");

    // Dwarf Planets & Cryogenic Ice Worlds (planet_type = 9)
    if lower_pluto {
        return (9, 1.0);
    }
    if lower_charon {
        return (9, 2.0);
    }
    if lower_eris {
        return (9, 3.0);
    }
    if lower_haumea {
        return (9, 4.0);
    }
    if lower_makemake {
        return (9, 5.0);
    }
    if lower_sedna {
        return (9, 6.0);
    }
    if lower_ceres {
        return (9, 7.0);
    }

    if body_type.is_star_or_remnant() {
        return (0, 0.0);
    }

    // Check for gas / ice giants:
    let is_uranus_or_neptune = contains_ascii_case_insensitive(name, "uranus")
        || contains_ascii_case_insensitive(name, "neptune");
    let is_ice_giant = body_type == BodyType::IceGiant
        || is_uranus_or_neptune
        || (norm.gas_frac > 0.20 && norm.ice_frac > 0.20);

    if is_ice_giant {
        return (2, 0.0);
    }

    if body_type == BodyType::GasGiant || norm.gas_frac > 0.40 {
        return (1, 0.0);
    }

    if body_type == BodyType::SuperEarth {
        return (6, 0.0);
    }

    // Explicit trans-Neptunian / Kuiper Belt / dwarf planet designations (planet_type = 9, generic KBO subtype = 0.0)
    let is_named_kbo_or_dwarf = contains_ascii_case_insensitive(name, "kuiper")
        || contains_ascii_case_insensitive(name, "kbo")
        || contains_ascii_case_insensitive(name, "oort")
        || contains_ascii_case_insensitive(name, "dwarf")
        || contains_ascii_case_insensitive(name, "orcus")
        || contains_ascii_case_insensitive(name, "quaoar")
        || contains_ascii_case_insensitive(name, "gonggong")
        || contains_ascii_case_insensitive(name, "varuna")
        || contains_ascii_case_insensitive(name, "ixion")
        || contains_ascii_case_insensitive(name, "salacia")
        || contains_ascii_case_insensitive(name, "arrokoth");
    if is_named_kbo_or_dwarf && !body_type.is_star_or_remnant() {
        return (9, 0.0);
    }

    // Major Jovian/Kronian moons & canonical terrestrial planets have dedicated subtypes on planet_type = 3:
    // 1.0 = Mars, 2.0 = Moon, 3.0 = Mercury, 4.0 = Earth, 5.0 = Venus,
    // 6.0 = Ganymede, 7.0 = Callisto, 8.0 = Titan, 0.0 = Generic / Europa / Enceladus / Io
    let is_europa_or_enceladus = contains_ascii_case_insensitive(name, "europa")
        || contains_ascii_case_insensitive(name, "enceladus");
    if contains_ascii_case_insensitive(name, "ganymede") {
        return (3, 6.0);
    }
    if contains_ascii_case_insensitive(name, "callisto") {
        return (3, 7.0);
    }
    if contains_ascii_case_insensitive(name, "titan")
        && !contains_ascii_case_insensitive(name, "titania")
    {
        return (3, 8.0);
    }
    if contains_ascii_case_insensitive(name, "mars") {
        return (3, 1.0);
    }
    if contains_ascii_case_insensitive(name, "moon")
        || contains_ascii_case_insensitive(name, "luna")
    {
        return (3, 2.0);
    }
    if contains_ascii_case_insensitive(name, "mercury") {
        return (3, 3.0);
    }
    if contains_ascii_case_insensitive(name, "earth")
        && !contains_ascii_case_insensitive(name, "super")
    {
        return (3, 4.0);
    }
    if contains_ascii_case_insensitive(name, "venus") {
        return (3, 5.0);
    }

    // Solid planetary bodies (TerrestrialPlanet or Protoplanet, excluding moons/asteroids)
    // with high volatile ice fraction and minimal gas envelope are cryogenic ice dwarf worlds (planet_type = 9).
    if matches!(
        body_type,
        BodyType::TerrestrialPlanet | BodyType::Protoplanet
    ) && !is_europa_or_enceladus
        && norm.ice_frac > 0.45
    {
        return (9, 0.0);
    }

    match body_type {
        BodyType::TerrestrialPlanet | BodyType::Protoplanet | BodyType::Moon => (3, 0.0),
        _ => (4, 0.0),
    }
}
