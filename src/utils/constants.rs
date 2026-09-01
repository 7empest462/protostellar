//! Physical and astrophysical constants for Protostellar.
//!
//! # Unit System
//! The simulation uses **Normalized Astrophysical Units**:
//! - Distance: Astronomical Units ($\text{AU}$) ($1 \text{ AU} \approx 1.495978707 \times 10^{11} \text{ m}$)
//! - Mass: Solar Masses ($M_\odot$) ($1 M_\odot \approx 1.98847 \times 10^{30} \text{ kg}$)
//! - Time: Years ($\text{yr}$) ($1 \text{ yr} \approx 31557600 \text{ s}$)
//!
//! In this system:
//! $$G = 4\pi^2 \approx 39.47841760435743 \text{ AU}^3 / (M_\odot \cdot \text{yr}^2)$$
//! Orbital velocity of Earth around Sun: $v \approx 2\pi \text{ AU/yr} \approx 29.78 \text{ km/s}$.

use std::f64::consts::PI;

/// Gravitational Constant in Normalized Astrophysical Units: $\text{AU}^3 / (M_\odot \cdot \text{yr}^2)$
/// $G = 4\pi^2$
pub const G_ASTRO: f64 = 4.0 * PI * PI;

/// Gravitational Constant in SI units: $\text{m}^3 / (\text{kg} \cdot \text{s}^2)$
pub const G_SI: f64 = 6.67430e-11;

/// $1 \text{ AU}$ in meters
pub const AU_TO_METERS: f64 = 1.495978707e11;

/// $1 \text{ AU}$ in kilometers
pub const AU_TO_KM: f64 = 1.495978707e8;

/// $1 M_\odot$ (Solar Mass) in kilograms
pub const SOLAR_MASS_KG: f64 = 1.98847e30;

/// $1 M_\oplus$ (Earth Mass) in Solar Masses
pub const EARTH_MASS_SOLAR: f64 = 5.9722e24 / SOLAR_MASS_KG; // ~3.0034896e-6

/// $1 M_\oplus$ in kilograms
pub const EARTH_MASS_KG: f64 = 5.9722e24;

/// $1 M_J$ (Jupiter Mass) in Solar Masses
pub const JUPITER_MASS_SOLAR: f64 = 1.89813e27 / SOLAR_MASS_KG; // ~9.5458e-4

/// $1 M_J$ in kilograms
pub const JUPITER_MASS_KG: f64 = 1.89813e27;

/// 1 Solar Radius in AU
pub const SOLAR_RADIUS_AU: f64 = 6.957e8 / AU_TO_METERS; // ~0.00465047 AU

/// 1 Solar Radius in kilometers
pub const SOLAR_RADIUS_KM: f64 = 6.957e5;

/// 1 Earth Radius in AU
pub const EARTH_RADIUS_AU: f64 = 6.371e6 / AU_TO_METERS; // ~4.25875e-5 AU

/// 1 Earth Radius in kilometers
pub const EARTH_RADIUS_KM: f64 = 6371.0;

/// 1 Year in seconds
pub const YEAR_SECONDS: f64 = 31557600.0; // Julian year: 365.25 days

/// $1 \text{ AU/yr}$ in $\text{km/s}$
pub const AU_PER_YR_TO_KM_PER_S: f64 = AU_TO_KM / YEAR_SECONDS; // ~4.74047 km/s

/// Chandrasekhar mass limit for electron degeneracy pressure ($1.44\text{ M}_\odot$)
pub const CHANDRASEKHAR_LIMIT_SOLAR: f64 = 1.44;

/// Tolman-Oppenheimer-Volkoff (TOV) mass limit for neutron degeneracy pressure ($2.17\text{ M}_\odot$)
pub const TOV_LIMIT_SOLAR: f64 = 2.17;

/// Speed of light in AU/yr
pub const SPEED_OF_LIGHT_AU_YR: f64 = (299792458.0 * YEAR_SECONDS) / AU_TO_METERS; // ~63241.077 AU/yr

/// Stefan-Boltzmann Constant in $\text{W} / (\text{m}^2 \cdot \text{K}^4)$
pub const STEFAN_BOLTZMANN_SI: f64 = 5.670374419e-8;

/// Solar Luminosity $L_\odot$ in Watts
pub const SOLAR_LUMINOSITY_WATTS: f64 = 3.828e26;

/// Standard density of rocky silicates in $M_\odot / \text{AU}^3$
/// Silicate rock $\rho \approx 3300 \text{ kg/m}^3$
pub const DENSITY_ROCK_ASTRO: f64 =
    (3300.0 * AU_TO_METERS * AU_TO_METERS * AU_TO_METERS) / SOLAR_MASS_KG;

/// Standard density of water ice in $M_\odot / \text{AU}^3$
/// Ice $\rho \approx 930 \text{ kg/m}^3$
pub const DENSITY_ICE_ASTRO: f64 =
    (930.0 * AU_TO_METERS * AU_TO_METERS * AU_TO_METERS) / SOLAR_MASS_KG;

/// Standard density of iron/metal core in $M_\odot / \text{AU}^3$
/// Iron/Nickel $\rho \approx 7870 \text{ kg/m}^3$
pub const DENSITY_IRON_ASTRO: f64 =
    (7870.0 * AU_TO_METERS * AU_TO_METERS * AU_TO_METERS) / SOLAR_MASS_KG;

/// Standard density of carbonaceous organics/tar in $M_\odot / \text{AU}^3$
/// Organics $\rho \approx 1400 \text{ kg/m}^3$
pub const DENSITY_ORGANICS_ASTRO: f64 =
    (1400.0 * AU_TO_METERS * AU_TO_METERS * AU_TO_METERS) / SOLAR_MASS_KG;

/// Standard mean density of the Sun in $M_\odot / \text{AU}^3$
pub const DENSITY_SUN_ASTRO: f64 =
    1.0 / ((4.0 / 3.0) * PI * SOLAR_RADIUS_AU * SOLAR_RADIUS_AU * SOLAR_RADIUS_AU);

/// Converts a Blackbody Temperature (in Kelvin) to an sRGB color tuple `(r, g, b)`
/// using Planck's law / Tanner Helland's empirical curve for blackbody radiation.
pub fn blackbody_to_srgb(temp_kelvin: f64) -> (f32, f32, f32) {
    let t = (temp_kelvin / 100.0).clamp(10.0, 400.0);

    // Red component
    let r = if t <= 66.0 {
        255.0
    } else {
        let x = t - 60.0;
        (329.698727446 * x.powf(-0.1332047592)).clamp(0.0, 255.0)
    };

    // Green component
    let g = if t <= 66.0 {
        let x = t;
        (99.4708025861 * x.ln() - 161.1195681661).clamp(0.0, 255.0)
    } else {
        let x = t - 60.0;
        (288.1221695283 * x.powf(-0.0755148492)).clamp(0.0, 255.0)
    };

    // Blue component
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        let x = t - 10.0;
        (138.5177312231 * x.ln() - 305.0447927307).clamp(0.0, 255.0)
    };

    ((r / 255.0) as f32, (g / 255.0) as f32, (b / 255.0) as f32)
}
