//! Simulation resources, global settings, time warp controls, and diagnostics.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Global configuration for the astrophysical physics engine.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Gravitational softening parameter ($\epsilon$) in AU to prevent singularities.
    pub softening_au: f64,
    /// Barnes-Hut opening angle threshold ($\theta$). $\theta = 0$ is exact direct $O(N^2)$.
    pub barnes_hut_theta: f32,
    /// Base fixed physics step in years ($\text{yr}$) (default: $\sim 0.0005\text{ yr} \approx 4.38\text{ hr}$).
    pub base_dt_yr: f64,
    /// Maximum sub-steps per frame during time warp.
    pub max_substeps_per_frame: usize,
    /// Whether gas aerodynamic drag is actively decelerating solid particles.
    pub enable_gas_drag: bool,
    /// Whether particle mergers and collisions are actively calculated.
    pub enable_accretion: bool,
    /// Whether radiative thermodynamic heating/cooling is active.
    pub enable_thermodynamics: bool,
    /// Target number of particles in the disk.
    pub target_particle_count: usize,
    /// Accelerated accretion cross-section multiplier.
    pub accretion_rate_multiplier: f32,
    /// Gas disk density scale (decreases towards 0 as star blows away gas).
    pub gas_density_scale: f32,
    /// Number of active non-absorbed particles.
    pub active_particles: u32,
    /// Global render size multiplier for microscopic dust particles (0.1x - 0.3x for fine dusty haze).
    pub particle_render_scale: f32,
    /// Global render size multiplier for celestial bodies.
    pub body_render_scale: f32,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            softening_au: 0.008, // ~1.2 million km
            barnes_hut_theta: 0.5,
            base_dt_yr: 0.0005,         // ~4.38 hours per physics step
            max_substeps_per_frame: 24, // Optimized for 120 FPS high-speed time warp
            enable_gas_drag: true,
            enable_accretion: true,
            enable_thermodynamics: true,
            target_particle_count: 50000,
            accretion_rate_multiplier: 120.0,
            gas_density_scale: 1.0,
            active_particles: 50000,
            particle_render_scale: 0.3,
            body_render_scale: 0.15, // Scale down planets relative to distances for realism
        }
    }
}

impl SimulationConfig {
    /// Computes the visual render radius of a celestial body using a smooth cube-root mass relationship.
    /// Keeps pure dust very small and scales embryos and mature planets clearly relative to the star.
    pub fn calc_render_radius(
        mass_solar: f64,
        body_type: crate::simulation::components::BodyType,
    ) -> f32 {
        use crate::simulation::components::BodyType;
        use crate::utils::constants::*;

        match body_type {
            BodyType::Protostar | BodyType::MainSequenceStar => 0.08,
            BodyType::GasGiant => {
                let m_ratio = (mass_solar / JUPITER_MASS_SOLAR).cbrt() as f32;
                (0.045 * m_ratio).clamp(0.030, 0.065)
            }
            BodyType::IceGiant => {
                let m_ratio = (mass_solar / (15.0 * EARTH_MASS_SOLAR)).cbrt() as f32;
                (0.032 * m_ratio).clamp(0.022, 0.045)
            }
            BodyType::TerrestrialPlanet => {
                let m_ratio = (mass_solar / EARTH_MASS_SOLAR).cbrt() as f32;
                (0.020 * m_ratio).clamp(0.014, 0.030)
            }
            BodyType::Protoplanet => {
                let m_ratio = (mass_solar / (0.05 * EARTH_MASS_SOLAR)).cbrt() as f32;
                (0.014 * m_ratio).clamp(0.010, 0.020)
            }
            BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet => {
                let m_ratio = (mass_solar / (0.001 * EARTH_MASS_SOLAR)).cbrt() as f32;
                (0.008 * m_ratio).clamp(0.005, 0.012)
            }
            BodyType::Moon => {
                let m_ratio = (mass_solar / (0.012 * EARTH_MASS_SOLAR)).cbrt() as f32;
                (0.006 * m_ratio).clamp(0.004, 0.010)
            }
            _ => 0.005,
        }
    }
}

/// Simulation Time and warp controls with continuous logarithmic speed scaling.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct TimeWarp {
    /// Time acceleration multiplier ($0.01\times \to 1,000,000\times$).
    pub multiplier: f64,
    /// Whether the simulation is currently paused.
    pub is_paused: bool,
    /// If true, advance the simulation by one physics step and then pause.
    pub step_once: bool,
}

impl Default for TimeWarp {
    fn default() -> Self {
        Self {
            multiplier: 50.0, // 50x speed default so orbits swirl visibly and accretion proceeds smoothly
            is_paused: false,
            step_once: false,
        }
    }
}

impl TimeWarp {
    pub const MIN_SPEED: f64 = 0.01;
    pub const MAX_SPEED: f64 = 1_000_000.0;

    /// Multiplies the speed logarithmically (e.g. by 1.25x).
    pub fn speed_up(&mut self, factor: f64) {
        self.multiplier = (self.multiplier * factor).clamp(Self::MIN_SPEED, Self::MAX_SPEED);
    }

    /// Divides the speed logarithmically (e.g. by 1.25x).
    pub fn slow_down(&mut self, factor: f64) {
        self.multiplier = (self.multiplier / factor).clamp(Self::MIN_SPEED, Self::MAX_SPEED);
    }

    /// Sets an exact preset speed multiplier.
    pub fn set_preset(&mut self, preset: f64) {
        self.multiplier = preset.clamp(Self::MIN_SPEED, Self::MAX_SPEED);
    }

    /// Formats the current time warp speed into human-comprehensible units.
    pub fn human_readable_speed(&self) -> String {
        if self.is_paused {
            return "PAUSED".to_string();
        }

        let years_per_sec = 0.03 * self.multiplier; // 60 fps * 0.0005 yr = 0.03 yr/s @ 1x
        if years_per_sec < 0.0833 {
            let days = years_per_sec * 365.25;
            format!("{:.1}x (1s = {:.1} days)", self.multiplier, days)
        } else if years_per_sec < 1.0 {
            let months = years_per_sec * 12.0;
            format!("{:.1}x (1s = {:.1} months)", self.multiplier, months)
        } else if years_per_sec < 1000.0 {
            format!("{:.0}x (1s = {:.1} yr)", self.multiplier, years_per_sec)
        } else if years_per_sec < 1_000_000.0 {
            format!(
                "{:.0}x (1s = {:.1}k yr)",
                self.multiplier,
                years_per_sec / 1000.0
            )
        } else {
            format!(
                "{:.0}x (1s = {:.2}M yr)",
                self.multiplier,
                years_per_sec / 1_000_000.0
            )
        }
    }
}

/// Accumulated simulation time.
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct SimTime {
    /// Total elapsed simulation time in years ($\text{yr}$).
    pub elapsed_years: f64,
    /// Effective $\Delta t$ used for the current frame step in years.
    pub current_dt_yr: f64,
    /// Total physics integration steps executed.
    pub step_count: u64,
}

/// Real-time diagnostic monitor for total energy conservation ($E = K + U$).
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnergyMonitor {
    /// Initial total mechanical energy ($E_0 = K_0 + U_0$).
    pub initial_total_energy: f64,
    /// Current kinetic energy ($K = \sum \frac{1}{2} m_i v_i^2$).
    pub kinetic_energy: f64,
    /// Current gravitational potential energy ($U = -\sum_{i < j} \frac{G m_i m_j}{r_{ij}}$).
    pub potential_energy: f64,
    /// Current total energy ($E = K + U$).
    pub total_energy: f64,
    /// Fractional energy conservation drift: $|\Delta E / E_0| = |(E - E_0) / E_0|$.
    pub relative_energy_drift: f64,
    /// Has initial energy been recorded.
    pub initialized: bool,
}

/// Astrophysical parameters defining the initial Hayashi Minimum Mass Solar Nebula (MMSN).
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct DiskParameters {
    /// Central protostar mass in $M_\odot$.
    pub central_star_mass: f64,
    /// Total protoplanetary solid dust mass in $M_\odot$ (~150-200 Earth masses).
    pub disk_mass: f64,
    /// Inner disk truncation radius in AU.
    pub inner_radius_au: f64,
    /// Outer disk edge radius in AU.
    pub outer_radius_au: f64,
    /// Surface density profile power-law exponent ($\Sigma(r) \propto r^{-p}$, Hayashi uses $p = 1.5$).
    pub density_power_law: f64,
    /// Temperature at $1\text{ AU}$ in Kelvin.
    pub reference_temp_1au: f64,
    /// Water ice condensation "snow line" radius in AU ($\sim 2.7\text{ AU}$).
    pub snow_line_au: f64,
    /// Gas disk lifetime in years before photo-evaporative clearance ($\sim 3 - 5\text{ Myr}$).
    pub gas_disk_lifetime_yr: f64,
}

impl Default for DiskParameters {
    fn default() -> Self {
        Self {
            central_star_mass: 1.0,
            disk_mass: 0.0006,     // ~200 Earth masses of solid dust/planetesimals
            inner_radius_au: 0.35, // Sublimation zone (inside Mercury's perihelion)
            outer_radius_au: 45.0, // Authentic Kuiper Belt boundary (~40-45 AU)
            density_power_law: 1.5,
            reference_temp_1au: 280.0,
            snow_line_au: 2.70, // Authentic Ice/Snow Line (Asteroid Belt transition)
            gas_disk_lifetime_yr: 15_000.0, // 15,000 years before complete photo-evaporative clearance
        }
    }
}

/// Diagnostic overlay visualization modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DiagnosticOverlayMode {
    #[default]
    Realistic,
    SpectralComposition,
    HillSpheresAndGaps,
}

impl DiagnosticOverlayMode {
    pub fn cycle(&self) -> Self {
        match self {
            Self::Realistic => Self::SpectralComposition,
            Self::SpectralComposition => Self::HillSpheresAndGaps,
            Self::HillSpheresAndGaps => Self::Realistic,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Realistic => "Realistic PBR",
            Self::SpectralComposition => "Spectral Composition Map",
            Self::HillSpheresAndGaps => "Hill Spheres & Annular Gaps",
        }
    }
}

/// Active impact shockwave event from physical collision mergers.
#[derive(Debug, Clone)]
pub struct ImpactShockwave {
    pub position: Vec3,
    pub radius: f32,
    pub max_radius: f32,
    pub timer: f32,
    pub max_timer: f32,
    pub color: Color,
}

/// Pool of active visual impact shockwaves.
#[derive(Resource, Debug, Clone, Default)]
pub struct ImpactShockwavePool {
    pub shockwaves: Vec<ImpactShockwave>,
}

/// Available player tools for interacting with and shaping the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlayerTool {
    #[default]
    Inspect,
    /// Apply a velocity impulse ($\Delta \vec{v}$) vector to a selected body
    GravitationalImpulse,
    /// Position a virtual gravitational tractor to gently redirect bodies
    GravitationalTractor,
    /// Inject a clump of mass/particles into an orbital ring
    MassInjection,
    /// Trigger a spiral density wave perturbation
    DensityWave,
}

/// Global state tracking player interaction and tool selection.
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerInteractionState {
    pub selected_entity: Option<Entity>,
    pub hovered_entity: Option<Entity>,
    pub active_tool: PlayerTool,
    pub overlay_mode: DiagnosticOverlayMode,
    pub tractor_position: Option<DVec3>,
    pub tractor_mass: f64,
    pub impulse_delta_v: Option<DVec3>,
    pub impulse_target_entity: Option<Entity>,
}
