//! Simulation ECS components for celestial bodies in Protostellar.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use crate::utils::constants::*;
use crate::utils::math::OrbitalElements;

/// Mass hierarchy classification for accretion and physics scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MassTier {
    /// Microscopic to meter-scale dust and pebbles (< 1e-4 Earth Masses)
    DustPebble,
    /// Kilometer-scale planetesimals (1e-4 to 0.005 Earth Masses)
    Planetesimal,
    /// Protoplanetary embryos (0.005 to 0.1 Earth Masses) - Promoted to ECS
    Embryo,
    /// Mature major planets (>= 0.1 Earth Masses) - Full N-body gravity
    MajorPlanet,
}

impl MassTier {
    pub fn from_mass(mass_solar: f64) -> Self {
        let m_earth = mass_solar / EARTH_MASS_SOLAR;
        if m_earth >= 0.1 {
            MassTier::MajorPlanet
        } else if m_earth >= 0.005 {
            MassTier::Embryo
        } else if m_earth >= 1e-4 {
            MassTier::Planetesimal
        } else {
            MassTier::DustPebble
        }
    }
}

/// Marker and classification component for all astronomical bodies.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct CelestialBody {
    pub body_type: BodyType,
    pub name: String,
}

/// Astrophysical classification of a body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyType {
    /// Microscopic to millimeter dust grains (aggregated in super-particles)
    DustGrain,
    /// Kilometer-scale rocky/icy bodies
    Planetesimal,
    /// Moon- to Mars-scale embryo in runaway/oligarchic accretion
    Protoplanet,
    /// Cleared-orbit rocky world (Mercury, Venus, Earth, Mars type)
    TerrestrialPlanet,
    /// Massive rocky / terrestrial planet (1.5 - 10 Earth masses, e.g. Kepler-10b, 55 Cancri e)
    SuperEarth,
    /// Massive hydrogen/helium envelope world (Jupiter, Saturn type)
    GasGiant,
    /// Water/ammonia/methane mantle world (Uranus, Neptune type)
    IceGiant,
    /// Sub-stellar brown dwarf (deuterium burning only, M < 0.08 M_sun)
    BrownDwarf,
    /// Low-mass, fully convective red dwarf (M < 0.5 M_sun)
    RedDwarf,
    /// Sun-like yellow dwarf star (G-type, ~1.0 M_sun)
    YellowDwarf,
    /// Hot, massive blue giant star (B-type, 8-15 M_sun)
    BlueGiant,
    /// Luminous blue supergiant star (O-type, 15-25 M_sun)
    BlueSupergiant,
    /// Expanded hydrogen-shell burning red giant
    RedGiant,
    /// Highly evolved massive red supergiant (Betelgeuse type)
    RedSupergiant,
    /// Extremely luminous hypergiant star (M > 25 M_sun)
    Hypergiant,
    /// Evolved massive star shedding envelope with intense winds
    WolfRayet,
    /// Pre-main-sequence contracting protostellar core
    Protostar,
    /// Hydrogen-burning ignited star
    MainSequenceStar,
    /// Degenerate carbon-oxygen Earth-sized stellar remnant (M <= 1.44 M_sun)
    WhiteDwarf,
    /// Ultra-dense degenerate neutron star core (M <= 2.17 M_sun)
    NeutronStar,
    /// Rapidly spinning magnetized neutron star with relativistic beams
    Pulsar,
    /// Neutron star with extreme magnetic field (10^14 - 10^15 Gauss)
    Magnetar,
    /// Gravitational singularity with event horizon and accretion disk
    BlackHole,
    /// JWST Little Red Dot / Black Hole Star: Supermassive black hole seed encased in a dense 60 AU hydrogen cocoon
    QuasiStar,
    /// Minor rocky body
    Asteroid,
    /// Volatile-rich icy body
    Comet,
    /// Debris ring or post-disruption fragment swarm
    DebrisRing,
    /// Natural satellite or moon orbiting a parent planet
    Moon,
}

impl BodyType {
    /// Returns true if this body is an active star or degenerate stellar remnant.
    pub fn is_star_or_remnant(&self) -> bool {
        matches!(
            self,
            BodyType::Protostar
                | BodyType::MainSequenceStar
                | BodyType::BrownDwarf
                | BodyType::RedDwarf
                | BodyType::YellowDwarf
                | BodyType::BlueGiant
                | BodyType::BlueSupergiant
                | BodyType::RedGiant
                | BodyType::RedSupergiant
                | BodyType::Hypergiant
                | BodyType::WolfRayet
                | BodyType::WhiteDwarf
                | BodyType::NeutronStar
                | BodyType::Pulsar
                | BodyType::Magnetar
                | BodyType::BlackHole
                | BodyType::QuasiStar
        )
    }

    /// Returns true if this body is a degenerate compact remnant.
    pub fn is_remnant(&self) -> bool {
        matches!(
            self,
            BodyType::WhiteDwarf
                | BodyType::NeutronStar
                | BodyType::Pulsar
                | BodyType::Magnetar
                | BodyType::BlackHole
                | BodyType::QuasiStar
        )
    }

    /// Returns true if this body is a major planet.
    pub fn is_planet(&self) -> bool {
        matches!(
            self,
            BodyType::TerrestrialPlanet
                | BodyType::SuperEarth
                | BodyType::GasGiant
                | BodyType::IceGiant
                | BodyType::Protoplanet
        )
    }
}

/// Centralized hydrostatic equilibrium and mass-dependent classification function.
pub fn classify_body_by_mass_and_comp(
    mass_solar: f64,
    comp: &Composition,
    is_central_star: bool,
) -> BodyType {
    let mass_earth = mass_solar / crate::utils::constants::EARTH_MASS_SOLAR;
    let mass_jupiter = mass_solar / crate::utils::constants::JUPITER_MASS_SOLAR;

    if is_central_star || mass_solar >= 0.08 {
        if mass_solar < 0.08 {
            BodyType::BrownDwarf
        } else if mass_solar < 0.50 {
            BodyType::RedDwarf
        } else if mass_solar < 1.4 {
            BodyType::YellowDwarf
        } else if mass_solar < 8.0 {
            BodyType::BlueGiant
        } else if mass_solar < 25.0 {
            BodyType::BlueSupergiant
        } else if mass_solar < 500.0 {
            BodyType::Hypergiant
        } else {
            // Direct collapse beyond 500 M_sun into Intermediate-Mass Black Hole
            BodyType::BlackHole
        }
    } else {
        // Planetary / Minor body classification
        if mass_solar >= 0.0125 - 1e-7 {
            // ~13 Jupiter Masses
            BodyType::BrownDwarf
        } else if mass_jupiter >= 0.05 - 1e-6 || mass_earth >= 12.0 - 1e-5 {
            let norm = comp.normalized();
            let refractory_frac = norm.silicate_frac + norm.metal_frac + norm.organics_frac;
            // Giant Planet Regime (>= 12 Earth Masses / >= 16 M_earth):
            // 1. Gas Giant: Dominant hydrogen/helium envelope (gas > 20%).
            // 2. Ice Giant: MUST have a hefty volatile ice/water percentage (ice >= 25%).
            // 3. Mega-Earth / Massive Terrestrial: Dominantly rocky/metallic (refractory >= 55%, gas <= 12%, ice < 25%).
            if norm.gas_frac > 0.20 {
                BodyType::GasGiant
            } else if norm.ice_frac >= 0.25 {
                BodyType::IceGiant
            } else if refractory_frac >= 0.55 && norm.gas_frac <= 0.12 {
                BodyType::SuperEarth // Mega-Earth / massive rocky terrestrial world
            } else if norm.gas_frac > 0.12 {
                BodyType::GasGiant
            } else {
                BodyType::SuperEarth
            }
        } else if mass_earth >= 1.75 - 1e-5 {
            let norm = comp.normalized();
            let refractory_frac = norm.silicate_frac + norm.metal_frac + norm.organics_frac;
            // Physical & Material Limits for Super-Earth Classification:
            // 1. Gas Envelope Limit: True Super-Earths have solid rocky/iron surfaces with only a thin
            //    secondary atmosphere (gas <= 0.08, or <= 8% by mass). If gas > 0.15, it is a Gas Dwarf / Mini-Neptune.
            // 2. Refractory Limit: Must be dominantly rock and iron core (silicates + metal + organics >= 0.55).
            // 3. Volatile Ice Limit: If ice >= 0.25, it is a volatile-rich Water World / Sub-Neptune (Ice Giant).
            // 4. Bulk Density Limit: Must have high rocky/metallic density (rho >= 0.80 * DENSITY_ROCK_ASTRO, ~2640 kg/m^3).
            if norm.gas_frac > 0.15 {
                BodyType::GasGiant // Gas Dwarf / Mini-Neptune
            } else if norm.ice_frac >= 0.25 || (norm.gas_frac > 0.08 && norm.ice_frac > 0.15) {
                BodyType::IceGiant // Sub-Neptune / Water World (MUST have hefty ice!)
            } else if norm.gas_frac <= 0.08
                && refractory_frac >= 0.55
                && norm.average_density() >= 0.80 * crate::utils::constants::DENSITY_ROCK_ASTRO
            {
                BodyType::SuperEarth // True Rocky Super-Earth!
            } else if norm.gas_frac > 0.08 {
                BodyType::GasGiant // Thick volatile envelope
            } else {
                BodyType::TerrestrialPlanet
            }
        } else if mass_earth >= 0.40 - 1e-5 {
            let norm = comp.normalized();
            if norm.gas_frac > 0.20 {
                BodyType::GasGiant
            } else if norm.ice_frac >= 0.25 {
                BodyType::IceGiant
            } else {
                BodyType::TerrestrialPlanet
            }
        } else if mass_earth >= 0.02 - 1e-5 {
            BodyType::Protoplanet
        } else if mass_earth >= 0.001 - 1e-6 {
            BodyType::Planetesimal
        } else if comp.ice_frac > 0.35 {
            BodyType::Comet
        } else {
            BodyType::Asteroid
        }
    }
}

/// Identifies a natural moon / satellite orbiting a parent celestial body.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SatelliteOf {
    /// Entity of the parent world (e.g. Earth, Jupiter, Protoplanet)
    pub parent: Entity,
    /// Semi-major axis in AU relative to parent body
    pub semi_major_axis_au: f64,
    /// Orbital period in years around parent body
    pub orbital_period_years: f64,
    /// Current true anomaly / orbital angle in radians
    pub true_anomaly: f64,
}

/// Mass of the body in Solar Masses ($M_\odot$).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct Mass(pub f64);

/// Simulation position in Astronomical Units ($\text{AU}$) (Double Precision).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct SimPosition(pub DVec3);

/// Simulation velocity in $\text{AU / yr}$ (Double Precision).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct SimVelocity(pub DVec3);

/// Gravitational acceleration in $\text{AU / yr}^2$ (Double Precision).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct SimAcceleration(pub DVec3);

impl Default for SimAcceleration {
    fn default() -> Self {
        Self(DVec3::ZERO)
    }
}

/// Physical radius of the body in $\text{AU}$.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct Radius(pub f64);

/// Surface temperature of the body in Kelvin ($\text{K}$).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct Temperature(pub f64);

/// Bolometric luminosity of the body in Solar Luminosities ($L_\odot$).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct Luminosity(pub f64);

impl Default for Luminosity {
    fn default() -> Self {
        Self(0.0)
    }
}

/// Spin and orbital angular momentum vector in $M_\odot \cdot \text{AU}^2 / \text{yr}$.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut)]
pub struct AngularMomentum(pub DVec3);

impl Default for AngularMomentum {
    fn default() -> Self {
        Self(DVec3::ZERO)
    }
}

/// Planetary rotation period, spin vector, and axial tilt.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpinState {
    /// Spin angular momentum vector $\vec{S}$ in $M_\odot \cdot \text{AU}^2 / \text{yr}$.
    pub spin_vector: DVec3,
    /// Rotation period in Earth hours (day length).
    pub rotation_period_hours: f64,
    /// Axial tilt (obliquity) relative to orbital plane in degrees.
    pub axial_tilt_degrees: f64,
}

impl Default for SpinState {
    fn default() -> Self {
        Self {
            spin_vector: DVec3::new(0.0, 1e-12, 0.0),
            rotation_period_hours: 24.0,
            axial_tilt_degrees: 5.0,
        }
    }
}

impl SpinState {
    /// Recalculates day length and obliquity from spin angular momentum, mass, and radius.
    pub fn update_from_spin(&mut self, spin_vec: DVec3, mass_solar: f64, radius_au: f64) {
        self.spin_vector = spin_vec;
        let spin_mag = spin_vec.length();

        if spin_mag > 1e-20 && mass_solar > 1e-12 && radius_au > 1e-10 {
            // Moment of Inertia I = 0.33 * M * R^2 (differentiated planetary interior)
            let i_moment = 0.33 * mass_solar * radius_au * radius_au;
            let omega_rad_yr = spin_mag / i_moment;

            // Convert rad/yr to hours/rotation
            let period_yr = (2.0 * PI) / omega_rad_yr.max(1e-10);
            let period_hours = period_yr * YEAR_SECONDS / 3600.0;
            self.rotation_period_hours = period_hours.clamp(1.5, 50000.0);

            // Axial tilt relative to disk normal (Y-axis)
            let cos_theta = (spin_vec.y / spin_mag).clamp(-1.0, 1.0);
            self.axial_tilt_degrees = cos_theta.acos().to_degrees();
        } else {
            self.rotation_period_hours = 24.0;
            self.axial_tilt_degrees = 0.0;
        }
    }
}

/// State tracking the internal dynamics of a JWST Little Red Dot / Black Hole Star (Quasi-Star).
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct BlackHoleStarState {
    /// Mass of the central supermassive black hole seed in $M_\odot$ (~100,000 M_sun).
    pub black_hole_mass_solar: f64,
    /// Mass of the surrounding pristine hydrogen gas cocoon in $M_\odot$ (~50,000 M_sun).
    pub cocoon_mass_solar: f64,
    /// Radius of the outer hydrogen photosphere in AU (~60 AU).
    pub cocoon_radius_au: f64,
    /// Current accretion rate onto the central black hole relative to the Eddington limit ($L / L_{\text{Edd}}$).
    pub eddington_ratio: f64,
    /// Progress of radiative envelope blow-off (0.0 = intact cocoon, 1.0 = fully blown-off Quasar).
    pub blowout_progress: f32,
    /// Whether super-Eddington hyper-accretion mode is currently active.
    pub super_eddington_active: bool,
    /// Whether the envelope has been blown off into interstellar space.
    pub is_blown_out: bool,
    /// Cumulative mass swallowed by the black hole seed in $M_\odot$.
    pub accreted_envelope_mass: f64,
    /// Cumulative propagation distance of the relativistic polar laser light beams in AU (advances at speed of light c).
    pub jet_travel_distance_au: f64,
}

impl Default for BlackHoleStarState {
    fn default() -> Self {
        Self {
            black_hole_mass_solar: 400_000.0,
            cocoon_mass_solar: 50_000.0,
            cocoon_radius_au: 60.0,
            eddington_ratio: 3.5,
            blowout_progress: 0.0,
            super_eddington_active: true,
            is_blown_out: false,
            accreted_envelope_mass: 0.0,
            jet_travel_distance_au: 0.0,
        }
    }
}

impl BlackHoleStarState {
    /// Returns the total combined mass of the black hole seed plus remaining cocoon.
    pub fn total_mass_solar(&self) -> f64 {
        self.black_hole_mass_solar + self.cocoon_mass_solar
    }

    /// Triggers envelope blowout into a naked active Quasar.
    pub fn trigger_blowout(&mut self) {
        self.is_blown_out = true;
    }

    /// Toggles super-Eddington hyper-accretion on/off.
    pub fn toggle_super_eddington(&mut self) {
        self.super_eddington_active = !self.super_eddington_active;
        self.eddington_ratio = if self.super_eddington_active {
            4.5
        } else {
            0.9
        };
    }
}

/// Fractional chemical/bulk composition of the body (sums to 1.0).
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Composition {
    /// Metallic core elements ($\text{Fe}$, $\text{Ni}$, $\rho \approx 7870\text{ kg/m}^3$)
    pub metal_frac: f64,
    /// Rocky silicates ($\text{MgSiO}_3$, $\text{Fe}_2\text{SiO}_4$, $\rho \approx 3300\text{ kg/m}^3$)
    pub silicate_frac: f64,
    /// Volatile ices ($\text{H}_2\text{O}$, $\text{CO}_2$, $\text{CH}_4$, $\text{NH}_3$, $\rho \approx 930\text{ kg/m}^3$)
    pub ice_frac: f64,
    /// Complex carbonaceous organics / tholins ($\rho \approx 1400\text{ kg/m}^3$)
    pub organics_frac: f64,
    /// Primordial hydrogen/helium gas ($\rho \approx 100\text{ kg/m}^3$)
    pub gas_frac: f64,
}

impl Default for Composition {
    fn default() -> Self {
        Self {
            metal_frac: 0.15,
            silicate_frac: 0.65,
            ice_frac: 0.10,
            organics_frac: 0.05,
            gas_frac: 0.05,
        }
    }
}

impl Composition {
    /// Refractory metal-rich composition (Inner disk / Mercury-type)
    pub fn metal_rich() -> Self {
        Self {
            metal_frac: 0.65,
            silicate_frac: 0.30,
            ice_frac: 0.00,
            organics_frac: 0.05,
            gas_frac: 0.00,
        }
    }

    /// Rocky terrestrial composition (Earth, Venus, Mars)
    pub fn rocky() -> Self {
        Self {
            metal_frac: 0.32,
            silicate_frac: 0.63,
            ice_frac: 0.00,
            organics_frac: 0.05,
            gas_frac: 0.00,
        }
    }

    /// Silicate-rich mantle debris composition (The Moon, Galilean satellites)
    pub fn silicate_rich() -> Self {
        Self {
            metal_frac: 0.08,
            silicate_frac: 0.88,
            ice_frac: 0.02,
            organics_frac: 0.02,
            gas_frac: 0.00,
        }
    }

    /// Carbonaceous chondrite composition (Asteroid belt / Ceres)
    pub fn carbonaceous() -> Self {
        Self {
            metal_frac: 0.10,
            silicate_frac: 0.50,
            ice_frac: 0.15,
            organics_frac: 0.25,
            gas_frac: 0.00,
        }
    }

    /// Outer disk volatile ice-rich composition (Outer belt, Comets, Kuiper worlds)
    pub fn icy() -> Self {
        Self {
            metal_frac: 0.05,
            silicate_frac: 0.25,
            ice_frac: 0.55,
            organics_frac: 0.15,
            gas_frac: 0.00,
        }
    }

    /// Solar primordial gas composition (Jupiter, Saturn, Protostar)
    pub fn solar_gas() -> Self {
        Self {
            metal_frac: 0.001,
            silicate_frac: 0.004,
            ice_frac: 0.015,
            organics_frac: 0.000,
            gas_frac: 0.980,
        }
    }

    /// Pristine dust-free primordial hydrogen gas (JWST Little Red Dot / Population III environment)
    pub fn pure_hydrogen() -> Self {
        Self {
            metal_frac: 0.0,
            silicate_frac: 0.0,
            ice_frac: 0.0,
            organics_frac: 0.0,
            gas_frac: 1.0,
        }
    }

    /// Solar nebula composition alias
    pub fn solar_nebula() -> Self {
        Self::solar_gas()
    }

    /// Cycles to next major chemical archetype
    pub fn cycle_next_composition(&self) -> Self {
        if self.silicate_frac > 0.6 {
            Self::metal_rich()
        } else if self.metal_frac > 0.6 {
            Self::icy()
        } else if self.ice_frac > 0.4 {
            Self::solar_gas()
        } else {
            Self::rocky()
        }
    }

    /// Normalizes fractions so that their exact sum equals 1.000000.
    pub fn normalized(&self) -> Self {
        let sum = self.metal_frac
            + self.silicate_frac
            + self.ice_frac
            + self.organics_frac
            + self.gas_frac;
        if sum > 1e-12 {
            Self {
                metal_frac: self.metal_frac / sum,
                silicate_frac: self.silicate_frac / sum,
                ice_frac: self.ice_frac / sum,
                organics_frac: self.organics_frac / sum,
                gas_frac: self.gas_frac / sum,
            }
        } else {
            Self::rocky()
        }
    }

    /// Strict mass-weighted deterministic merger (conserves total mass of all 5 components).
    pub fn mass_weighted_merge(
        &self,
        m_self: f64,
        other: &Composition,
        m_other: f64,
    ) -> Composition {
        let total_mass = (m_self + m_other).max(1e-12);
        let raw = Composition {
            metal_frac: (self.metal_frac * m_self + other.metal_frac * m_other) / total_mass,
            silicate_frac: (self.silicate_frac * m_self + other.silicate_frac * m_other)
                / total_mass,
            ice_frac: (self.ice_frac * m_self + other.ice_frac * m_other) / total_mass,
            organics_frac: (self.organics_frac * m_self + other.organics_frac * m_other)
                / total_mass,
            gas_frac: (self.gas_frac * m_self + other.gas_frac * m_other) / total_mass,
        };
        raw.normalized()
    }

    /// Exact harmonic bulk density mixing in $M_\odot / \text{AU}^3$.
    pub fn average_density(&self) -> f64 {
        let inv_density = (self.metal_frac / DENSITY_IRON_ASTRO)
            + (self.silicate_frac / DENSITY_ROCK_ASTRO)
            + (self.ice_frac / DENSITY_ICE_ASTRO)
            + (self.organics_frac / DENSITY_ORGANICS_ASTRO)
            + (self.gas_frac / (0.02 * DENSITY_ROCK_ASTRO));

        if inv_density > 0.0 {
            1.0 / inv_density
        } else {
            DENSITY_ROCK_ASTRO
        }
    }

    /// Critical impact velocity ($v_{\text{crit}}$ in $\text{km/s}$) above which collisions bounce/fragment.
    pub fn stickiness_critical_velocity_km_s(&self) -> f64 {
        (self.ice_frac * 15.0)
            + (self.organics_frac * 8.0)
            + (self.silicate_frac * 4.0)
            + (self.metal_frac * 1.5)
    }

    /// Computes subtle RGB visual color tint from composition.
    pub fn visual_color_tint(&self) -> (f32, f32, f32) {
        let r = (self.metal_frac * 0.85
            + self.silicate_frac * 0.80
            + self.ice_frac * 0.70
            + self.organics_frac * 0.35) as f32;
        let g = (self.metal_frac * 0.70
            + self.silicate_frac * 0.55
            + self.ice_frac * 0.85
            + self.organics_frac * 0.30) as f32;
        let b = (self.metal_frac * 0.60
            + self.silicate_frac * 0.40
            + self.ice_frac * 0.95
            + self.organics_frac * 0.25) as f32;
        (r.clamp(0.1, 1.0), g.clamp(0.1, 1.0), b.clamp(0.1, 1.0))
    }
}

/// Internal geophysical differentiation of a growing planet into core, mantle, and crust.
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InternalDifferentiation {
    /// Whether the interior has melted and differentiated into layers (Iron Catastrophe).
    pub is_differentiated: bool,
    /// Fractional core differentiation progress (0.0 = undifferentiated chondritic mix, 1.0 = fully segregated core).
    pub differentiation_fraction: f32,
    /// Metallic core radius in AU.
    pub core_radius_au: f64,
    /// Silicate mantle outer boundary radius in AU.
    pub mantle_radius_au: f64,
    /// Silicate/Basaltic crust thickness in AU.
    pub crust_thickness_au: f64,
    /// Volatile hydrosphere / ice shell thickness in AU.
    pub ocean_ice_thickness_au: f64,
    /// Internal core temperature in Kelvin.
    pub core_temp_k: f64,
    /// Generated magnetic geodynamo field strength in Gauss.
    pub magnetic_field_gauss: f64,
    /// Whether the mantle contains sunken iron-rich protoplanetary remnants from Theia (LLSVPs / "the blobs").
    pub has_theia_llsvp: bool,
    /// Density contrast anomaly of the LLSVPs relative to surrounding mantle (+1.5% to +3.5%, Nature 2023).
    pub llsvp_density_contrast: f32,
}

impl Default for InternalDifferentiation {
    fn default() -> Self {
        Self {
            is_differentiated: false,
            differentiation_fraction: 0.0,
            core_radius_au: 0.0,
            mantle_radius_au: 0.0,
            crust_thickness_au: 0.0,
            ocean_ice_thickness_au: 0.0,
            core_temp_k: 300.0,
            magnetic_field_gauss: 0.0,
            has_theia_llsvp: false,
            llsvp_density_contrast: 0.0,
        }
    }
}

impl InternalDifferentiation {
    /// Computes physical layer boundaries and geodynamo magnetic field from mass, radius, and composition.
    pub fn recalculate(&mut self, mass_solar: f64, total_radius_au: f64, comp: &Composition) {
        let mass_earth = (mass_solar / EARTH_MASS_SOLAR).max(1e-6);
        let radius_earth = (total_radius_au / EARTH_RADIUS_AU).max(1e-4);

        // Accretional impact heating + radionuclide decay
        let accretional_temp_boost = 4200.0 * mass_earth.powf(0.67) / radius_earth;
        self.core_temp_k = (300.0 + accretional_temp_boost).clamp(300.0, 30000.0);

        // Differentiation occurs if mass > 0.005 Earth Masses or T_core > 1400 K (Iron Melting Point)
        if mass_earth >= 0.005 || self.core_temp_k > 1400.0 {
            self.is_differentiated = true;
            self.differentiation_fraction = (mass_earth as f32 / 0.05).clamp(0.2, 1.0);

            let avg_density = comp.average_density();

            // Iron/Nickel metallic core (sink to center via Stokes drag)
            let metal_vol_frac =
                (comp.metal_frac * avg_density / DENSITY_IRON_ASTRO).clamp(0.0, 1.0);
            self.core_radius_au =
                total_radius_au * metal_vol_frac.cbrt() * f64::from(self.differentiation_fraction);

            // Silicate rocky mantle
            let rock_vol_frac = (((comp.metal_frac + comp.silicate_frac) * avg_density)
                / DENSITY_ROCK_ASTRO)
                .clamp(0.0, 1.0);
            self.mantle_radius_au = total_radius_au * rock_vol_frac.cbrt().min(1.0);

            // Surface crust and volatile oceans/ice
            let crust_frac = (0.01 / (mass_earth + 1.0)).clamp(0.001, 0.05);
            self.crust_thickness_au = total_radius_au * crust_frac;

            let ocean_vol_frac = ((comp.ice_frac + comp.organics_frac) * avg_density
                / DENSITY_ICE_ASTRO)
                .clamp(0.0, 1.0);
            self.ocean_ice_thickness_au =
                (total_radius_au - self.mantle_radius_au).max(0.0) * ocean_vol_frac;

            // Geodynamo Magnetic Field (Gauss): B ~ 0.35 * sqrt(M/M_earth) * (R_core / R_core_earth)
            if self.core_radius_au > 0.0 && self.core_temp_k > 1200.0 {
                let r_core_earth = EARTH_RADIUS_AU * 0.55;
                self.magnetic_field_gauss = 0.35
                    * mass_earth.sqrt()
                    * (self.core_radius_au / r_core_earth)
                    * f64::from(self.differentiation_fraction);
            } else {
                self.magnetic_field_gauss = 0.0;
            }
        } else {
            self.is_differentiated = false;
            self.differentiation_fraction = 0.0;
            self.core_radius_au = 0.0;
            self.mantle_radius_au = total_radius_au;
            self.crust_thickness_au = 0.0;
            self.ocean_ice_thickness_au = 0.0;
            self.magnetic_field_gauss = 0.0;
        }
    }
}

/// Cached orbital elements computed relative to a central body.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct TrackedOrbit {
    pub central_entity: Option<Entity>,
    pub elements: OrbitalElements,
    pub last_updated_yr: f64,
}

/// Marker component for the body currently selected by the player.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct SelectedBody;

/// Marker component for the central star / protostar.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct CentralStar;
