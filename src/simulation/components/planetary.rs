//! Planetary and stellar state ECS components in Protostellar.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::celestial::BodyType;

/// Ignition progress state for a forming star (0.0 = cold cloud, 1.0 = fully ignited).
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IgnitionState {
    pub core_temperature: f64,
    pub fusion_fraction: f32,
    pub is_ignited: bool,
    pub shockwave_radius: f64,
}

impl Default for IgnitionState {
    fn default() -> Self {
        Self {
            core_temperature: 1e5,
            fusion_fraction: 0.0,
            is_ignited: false,
            shockwave_radius: 0.0,
        }
    }
}

/// Tracks delivered volatiles (water, nitrogen, organics) delivered via cometary impacts.
#[derive(Component, Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct VolatileInventory {
    /// Total water ice mass delivered in Earth masses (M_earth)
    pub delivered_water_m_earth: f64,
    /// Current ocean coverage fraction (0.0 to 1.0)
    pub ocean_coverage_frac: f32,
    /// Atmospheric surface pressure in bars
    pub atmospheric_pressure_bar: f32,
    /// Number of major cometary impacts absorbed
    pub cometary_impact_count: u32,
}

fn default_elongation() -> f32 {
    1.0
}

/// A dynamic impact basin formed by a major cometary / asteroidal impact or grazing flyby.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImpactBasin {
    /// Normal vector on the unit sphere
    pub surface_normal: Vec3,
    /// Basin angular radius in radians
    pub angular_radius: f32,
    /// Timestamp when impact occurred (sim_yr)
    pub formation_time_yr: f64,
    /// Current cooling state: 1.0 = glowing magma melt pool, 0.0 = solidified dark basalt mare
    pub melt_glow_fraction: f32,
    /// Impact elongation (1.0 = circular crater, >1.0 = elongated grazing trench)
    #[serde(default = "default_elongation")]
    pub elongation: f32,
}

/// Tracks recent impact basins on a planetary surface.
#[derive(Component, Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanetaryBasins {
    pub basins: Vec<ImpactBasin>,
}

/// Circumplanetary planetary ring system formed by tidal Roche disruption.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlanetaryRingSystem {
    pub inner_radius_au: f32,
    pub outer_radius_au: f32,
    pub ring_mass_earth: f64,
    pub optical_depth: f32,
    pub ice_fraction: f32,
    pub silicate_fraction: f32,
}

impl Default for PlanetaryRingSystem {
    fn default() -> Self {
        Self {
            inner_radius_au: 0.0008,
            outer_radius_au: 0.0022,
            ring_mass_earth: 0.0001,
            optical_depth: 0.85,
            ice_fraction: 0.95,
            silicate_fraction: 0.05,
        }
    }
}

/// Major thermodynamic climate regimes for terrestrial and giant planets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ClimateRegime {
    /// Frozen global ice sheets with high albedo (T < 260 K)
    SnowballIceAge,
    /// Liquid surface water oceans, moderate greenhouse balance, dynamic clouds (273 - 340 K)
    #[default]
    TemperateHabitable,
    /// Evaporated oceans, dense CO2/steam greenhouse runaway (T > 400 K, P > 20 bar)
    RunawayVenusian,
    /// Massive hydrogen/helium envelope (Gas/Ice Giants)
    GasGiantEnvelope,
    /// Airless frozen or baked rock (Mercury / Moon)
    AirlessVacuum,
}

/// Atmospheric radiative greenhouse climate equilibrium state.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PlanetaryClimate {
    /// Actual surface temperature in Kelvin (T_eq + dT_GH)
    pub surface_temperature_k: f32,
    /// Pure radiative equilibrium blackbody temperature from stellar flux (Kelvin)
    pub equilibrium_temperature_k: f32,
    /// Radiative atmospheric greenhouse temperature elevation (Kelvin)
    pub greenhouse_delta_k: f32,
    /// Bond albedo (0.0 to 1.0, e.g. 0.30 for Earth, 0.65 for Snowball Earth)
    pub albedo: f32,
    /// Surface fractional ice coverage (0.0 to 1.0)
    pub ice_coverage_frac: f32,
    /// Atmospheric cloud coverage fraction (0.0 to 1.0)
    pub cloud_coverage_frac: f32,
    /// Dominant climate regime classification
    pub climate_regime: ClimateRegime,
}

impl Default for PlanetaryClimate {
    fn default() -> Self {
        Self {
            surface_temperature_k: 288.0,
            equilibrium_temperature_k: 255.0,
            greenhouse_delta_k: 33.0,
            albedo: 0.30,
            ice_coverage_frac: 0.10,
            cloud_coverage_frac: 0.50,
            climate_regime: ClimateRegime::TemperateHabitable,
        }
    }
}

/// Dynamic habitability and living biosphere on terrestrial worlds.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BiosphereState {
    /// Habitability score (0.0 = sterile/hostile, 1.0 = ideal Eden)
    pub habitability_score: f32,
    /// Fractional surface coverage by photosynthetic vegetation / biomass (0.0 to 1.0)
    pub biomass_coverage_frac: f32,
    /// Atmospheric oxygen gas fraction (0.0 to 0.21)
    pub oxygen_fraction: f32,
    /// Timestamp when primordial life emerged (sim_yr)
    pub emergence_year: Option<f64>,
}

impl Default for BiosphereState {
    fn default() -> Self {
        Self {
            habitability_score: 0.0,
            biomass_coverage_frac: 0.0,
            oxygen_fraction: 0.0,
            emergence_year: None,
        }
    }
}

/// Major evolutionary epochs across the multi-billion-year lifecycle of a star.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StellarEvolutionPhase {
    /// Gravitational Kelvin-Helmholtz contraction before core hydrogen fusion
    #[default]
    ProtostarContraction,
    /// Stable core hydrogen fusion (e.g. Current Sun, ~10 Billion Year Lifespan)
    MainSequence,
    /// Core hydrogen exhausted; hydrogen shell burning swells star into a Red Giant (R ~ 1.25 AU, L ~ 2500 L_sun)
    RedGiantBranch,
    /// Advanced shell-burning and helium flash pulsations
    HeliumFlashAgb,
    /// Massive star expanded into a luminous Red Supergiant (Betelgeuse type)
    RedSupergiantBranch,
    /// Stellar envelope pulsation and mass shedding into multi-layer ionized planetary nebulae
    PlanetaryNebulaEjection,
    /// Cataclysmic core-collapse explosion of a massive star (>= 8 M_sun)
    SupernovaExplosion,
    /// Degenerate carbon-oxygen Earth-sized core remnant (R ~ 0.009 AU, T ~ 30,000 K)
    WhiteDwarf,
    /// Ultra-dense spinning magnetized neutron star with relativistic beams
    NeutronStarPulsar,
    /// Neutron star with ultra-intense magnetic field (10^14 - 10^15 Gauss)
    MagnetarRemnant,
    /// Gravitational singularity with event horizon, photon ring, and accretion disk
    BlackHoleRemnant,
}

/// Far-future stellar evolution, fuel consumption, and planetary nebula state.
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StellarEvolutionState {
    /// Current evolutionary phase of the central star
    pub phase: StellarEvolutionPhase,
    /// Core hydrogen nuclear fuel fraction remaining (1.0 = pristine, 0.0 = exhausted)
    pub hydrogen_core_fraction: f32,
    /// Core helium nuclear fuel fraction (0.0 to 1.0)
    pub helium_core_fraction: f32,
    /// Ongoing stellar wind mass loss rate in solar masses per year (M_sun / yr)
    pub envelope_mass_loss_rate: f64,
    /// Time spent in the current evolutionary phase in simulation years
    pub phase_timer_years: f64,
    /// Expanding planetary nebula / supernova ionized shell radius in AU
    pub nebula_expansion_radius_au: f32,
    /// Optical opacity of the ejected nebula (0.0 = clear, 1.0 = opaque)
    pub nebula_opacity: f32,
}

impl Default for StellarEvolutionState {
    fn default() -> Self {
        Self {
            phase: StellarEvolutionPhase::ProtostarContraction,
            hydrogen_core_fraction: 1.0,
            helium_core_fraction: 0.0,
            envelope_mass_loss_rate: 0.0,
            phase_timer_years: 0.0,
            nebula_expansion_radius_au: 0.0,
            nebula_opacity: 0.0,
        }
    }
}

/// Extreme electromagnetic properties for stars and stellar remnants (White Dwarfs, Pulsars, Magnetars, Black Holes).
#[derive(Component, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ElectromagneticFieldState {
    /// Magnetic surface dipole field strength in Gauss
    pub magnetic_field_gauss: f64,
    /// Rotational period in seconds
    pub rotation_period_sec: f64,
    /// Magnetic dipole inclination angle relative to rotational axis (radians)
    pub magnetic_inclination_rad: f32,
    /// Relativistic beam jet length in AU
    pub jet_length_au: f32,
    /// Synchrotron luminosity / radiance factor
    pub synchrotron_intensity: f32,
}

impl Default for ElectromagneticFieldState {
    fn default() -> Self {
        Self {
            magnetic_field_gauss: 1.0,
            rotation_period_sec: 25.0 * 86400.0,
            magnetic_inclination_rad: 0.15,
            jet_length_au: 0.0,
            synchrotron_intensity: 0.0,
        }
    }
}

/// Event triggered when an inner planet enters the expanding Red Giant envelope and is vaporized.
#[derive(Message, Debug, Clone)]
pub struct PlanetaryEngulfmentEvent {
    pub planet_entity: Entity,
    pub planet_name: String,
    pub distance_au: f64,
    pub planet_mass_earth: f64,
}

/// Event triggered when a massive star (>= 8 M_sun) or over-mass White Dwarf (> 1.44 M_sun) explodes.
#[derive(Message, Debug, Clone)]
pub struct SupernovaEvent {
    pub star_entity: Entity,
    pub star_name: String,
    pub initial_mass_solar: f64,
    pub remnant_mass_solar: f64,
    pub remnant_type: BodyType,
    pub shockwave_velocity_km_s: f64,
}

/// Atmospheric escape and photoevaporative cometary tail component.
#[derive(Component, Debug, Clone, Reflect)]
pub struct AtmosphericEscapeTail {
    /// Mass loss rate in Earth masses per million years
    pub loss_rate_m_earth_per_myr: f32,
    /// Dynamic tail length in AU (driven by XUV flux and solar wind)
    pub tail_length_au: f32,
    /// Ionization color based on atmospheric composition (cyan for H/He, amber for mineral vapor)
    pub ion_color: Color,
    /// True if planet is actively undergoing hydrodynamic escape
    pub is_active: bool,
}

impl Default for AtmosphericEscapeTail {
    fn default() -> Self {
        Self {
            loss_rate_m_earth_per_myr: 0.0,
            tail_length_au: 0.0,
            ion_color: Color::srgba(0.35, 0.85, 1.0, 0.75),
            is_active: false,
        }
    }
}
