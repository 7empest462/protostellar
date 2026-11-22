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
    /// Uniform size exaggeration factor applied to ALL celestial bodies (stars, planets, moons).
    /// At `1.0` (default), bodies render at true 1:1 physical radii in AU relative to orbital distances.
    /// Adjustable at runtime with `,` (decrease) and `Shift+.` (increase) keys.
    pub size_exaggeration: f32,
    /// Legacy compatibility field retained for serialized configs (true 1:1 scale uses physical radii).
    pub min_body_visual_radius: f32,
    /// Particle swarm density preset tier (100k Standard, 250k High, 500k Ultra).
    pub particle_density: ParticleDensityTier,
    /// Whether GPU Compute Pipeline is active for disk particle dynamics.
    pub enable_gpu_compute: bool,
    /// Live telemetry: whether GPU compute shader is successfully dispatching frames.
    pub gpu_compute_active: bool,
}

/// Selectable particle swarm density tier for GPU Keplerian orbit mechanics and N-body simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ParticleDensityTier {
    Standard100k,
    High250k,
    #[default]
    Ultra500k,
}

impl ParticleDensityTier {
    pub const fn particle_count(&self) -> u32 {
        match self {
            Self::Standard100k => 100_000,
            Self::High250k => 250_000,
            Self::Ultra500k => 500_000,
        }
    }

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Standard100k => "100k [Standard]",
            Self::High250k => "250k [High]",
            Self::Ultra500k => "500k [Ultra]",
        }
    }

    pub const fn cycle(&self) -> Self {
        match self {
            Self::Standard100k => Self::High250k,
            Self::High250k => Self::Ultra500k,
            Self::Ultra500k => Self::Standard100k,
        }
    }
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            softening_au: 0.008, // ~1.2 million km
            barnes_hut_theta: 0.5,
            base_dt_yr: 0.0005,         // ~4.38 hours per physics step
            max_substeps_per_frame: 48, // Enhanced substep resolution for stable high-speed time warp
            enable_gas_drag: true,
            enable_accretion: true,
            enable_thermodynamics: true,
            target_particle_count: 500_000,
            accretion_rate_multiplier: 120.0,
            gas_density_scale: 1.0,
            active_particles: 500_000,
            particle_render_scale: 0.12,
            size_exaggeration: 1.0,
            min_body_visual_radius: 0.0006,
            particle_density: ParticleDensityTier::Ultra500k,
            enable_gpu_compute: true,
            gpu_compute_active: false,
        }
    }
}

impl SimulationConfig {
    /// Computes the zoom-invariant 1:1 visual render radius (in AU) from a body's physical
    /// radius in AU, scaled linearly by `size_exaggeration` (default `1.0`).
    ///
    /// Example outputs at `1.0x` true physical scale:
    /// - Sun         (`0.00465 AU`) -> `0.00465 AU`
    /// - Jupiter     (`0.000477 AU`) -> `0.000477 AU`
    /// - Earth       (`0.0000426 AU`) -> `0.0000426 AU`
    /// - Moon        (`0.0000116 AU`) -> `0.0000116 AU`
    /// - TRAPPIST-1  (`0.00056 AU`) -> `0.00056 AU`
    pub fn calc_visual_radius(&self, physical_radius_au: f64) -> f32 {
        let r = if physical_radius_au.is_finite() {
            physical_radius_au.max(1e-9) as f32
        } else {
            1e-6
        };
        let exag = if self.size_exaggeration.is_finite() {
            self.size_exaggeration.clamp(0.1, 100.0)
        } else {
            1.0
        };
        r * exag
    }

    /// Computes the zoom-invariant 1:1 visual render radius for any celestial body classification.
    /// Delegates linearly to [`SimulationConfig::calc_visual_radius`] so stars, planets, moons,
    /// compact remnants, and minor bodies always maintain exact true proportions.
    pub fn calc_visual_radius_for_type(
        &self,
        physical_radius_au: f64,
        _body_type: crate::simulation::components::BodyType,
    ) -> f32 {
        self.calc_visual_radius(physical_radius_au)
    }

    /// Computes the visual render radius without orbital compression or distance distortion.
    /// Even compact systems such as TRAPPIST-1 retain ~20–90 planetary radii of open separation
    /// at true 1:1 scale, so bodies remain strictly linear with physical radius.
    pub fn calc_visual_radius_with_orbit(
        &self,
        physical_radius_au: f64,
        _body_type: crate::simulation::components::BodyType,
        _r_orb: f32,
        _min_planetary_orbit_au: f32,
    ) -> f32 {
        self.calc_visual_radius(physical_radius_au)
    }

    /// Computes the safe minimum camera zoom radius for a celestial body with given visual radius (AU).
    /// Allows close orbital inspection right down to true-scale terrestrial planets, moons, and comets
    /// while preserving clearance outside the body's surface.
    pub fn calc_camera_min_zoom_radius(&self, visual_radius: f32) -> f32 {
        let r = if visual_radius.is_finite() {
            visual_radius.max(1e-8)
        } else {
            1e-6
        };
        (r * 1.40).max(1e-8)
    }

    /// Computes the optimal camera framing distance when focusing or locking onto a celestial body.
    /// Nicely frames the body in view across all true-scale regimes (stars, planets, moons, comets).
    pub fn calc_camera_framing_radius(&self, visual_radius: f32) -> f32 {
        let min_r = self.calc_camera_min_zoom_radius(visual_radius);
        let r = if visual_radius.is_finite() {
            visual_radius.max(1e-8)
        } else {
            1e-6
        };
        (r * 3.5).clamp(min_r, 250.0)
    }

    /// Legacy mass-based render radius used only for accretion collision cross-sections.
    /// NOT used for visual rendering (use `calc_visual_radius` instead).
    pub fn calc_collision_radius(
        mass_solar: f64,
        body_type: crate::simulation::components::BodyType,
    ) -> f32 {
        use crate::simulation::components::BodyType;
        use crate::utils::constants::*;

        match body_type {
            BodyType::Protostar
            | BodyType::TTauriStar
            | BodyType::HerbigAeBeStar
            | BodyType::MainSequenceStar
            | BodyType::BrownDwarf
            | BodyType::RedDwarf
            | BodyType::OrangeDwarf
            | BodyType::YellowDwarf
            | BodyType::YellowWhiteDwarf
            | BodyType::WhiteMainSequence
            | BodyType::BlueMainSequence
            | BodyType::BlueGiant
            | BodyType::BlueSupergiant
            | BodyType::RedGiant
            | BodyType::RedSupergiant
            | BodyType::YellowHypergiant
            | BodyType::Hypergiant
            | BodyType::CarbonStar
            | BodyType::LuminousBlueVariable
            | BodyType::WolfRayet
            | BodyType::PopulationIIIStar
            | BodyType::BlueStraggler
            | BodyType::ThorneZytkow
            | BodyType::DarkStar => {
                let m_ratio = (mass_solar.max(0.01)).powf(0.25) as f32;
                (0.055 * m_ratio).clamp(0.025, 0.150)
            }
            BodyType::WhiteDwarf | BodyType::BlackDwarf | BodyType::IronStar => 0.012,
            BodyType::NeutronStar
            | BodyType::Pulsar
            | BodyType::Magnetar
            | BodyType::QuarkStar
            | BodyType::BosonStar => 0.006,
            BodyType::BlackHole => 0.010,
            BodyType::QuasiStar => 2.50,
            BodyType::GasGiant => {
                let m_ratio = (mass_solar / JUPITER_MASS_SOLAR).cbrt() as f32;
                (0.045 * m_ratio).clamp(0.030, 0.065)
            }
            BodyType::IceGiant => {
                let m_ratio = (mass_solar / (15.0 * EARTH_MASS_SOLAR)).cbrt() as f32;
                (0.032 * m_ratio).clamp(0.022, 0.045)
            }
            BodyType::SuperEarth => {
                let m_ratio = (mass_solar / (3.5 * EARTH_MASS_SOLAR)).cbrt() as f32;
                (0.026 * m_ratio).clamp(0.018, 0.036)
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
    /// Julian year length in seconds: 365.25 * 86,400 = 31,557,600 s.
    pub const SECONDS_PER_YEAR: f64 = 31_557_600.0;
    /// Baseline simulation rate at 60 fps with base dt = 0.0005 yr: 60 * 0.0005 = 0.03 yr/s.
    pub const BASE_YEARS_PER_SEC: f64 = 0.03;
    /// Multiplier corresponding to authentic Real-Time progression (1 wall-clock second = 1 simulation second).
    /// 1 / (0.03 * 31,557,600) = 1 / 946,728 ≈ 1.05626958e-6.
    pub const SPEED_REAL_TIME: f64 = 1.0 / (Self::BASE_YEARS_PER_SEC * Self::SECONDS_PER_YEAR);
    /// Multiplier corresponding to 1 wall-clock second = 1 simulation minute (60x real-time).
    pub const SPEED_1_MINUTE: f64 = 60.0 * Self::SPEED_REAL_TIME;
    /// Multiplier corresponding to 1 wall-clock second = 5 simulation minutes (300x real-time).
    pub const SPEED_5_MINUTES: f64 = 300.0 * Self::SPEED_REAL_TIME;
    /// Multiplier corresponding to 1 wall-clock second = 6 simulation hours (21,600x real-time).
    pub const SPEED_6_HOURS: f64 = 21_600.0 * Self::SPEED_REAL_TIME;
    /// Multiplier corresponding to 1 wall-clock second = 12 simulation hours (43,200x real-time).
    pub const SPEED_12_HOURS: f64 = 43_200.0 * Self::SPEED_REAL_TIME;
    /// Multiplier corresponding to 1 wall-clock second = 24 simulation hours / 1 day (86,400x real-time).
    pub const SPEED_24_HOURS: f64 = 86_400.0 * Self::SPEED_REAL_TIME;
    /// Multiplier corresponding to 1 wall-clock second = 72 simulation hours / 3 days (259,200x real-time).
    pub const SPEED_72_HOURS: f64 = 259_200.0 * Self::SPEED_REAL_TIME;

    /// Minimum allowable time multiplier (allows real-time and sub-second slow motion).
    pub const MIN_SPEED: f64 = 1.0e-7;
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

        let years_per_sec = Self::BASE_YEARS_PER_SEC * self.multiplier;
        let sec_per_sec = years_per_sec * Self::SECONDS_PER_YEAR;

        if (sec_per_sec - 1.0).abs() < 0.05 {
            "Real-Time (1s = 1.0s)".to_string()
        } else if sec_per_sec < 60.0 {
            format!("1s = {sec_per_sec:.1}s")
        } else if sec_per_sec < 3600.0 {
            let minutes = sec_per_sec / 60.0;
            if (minutes - minutes.round()).abs() < 0.05 {
                format!("1s = {:.0} min", minutes.round())
            } else {
                format!("1s = {minutes:.1} min")
            }
        } else if sec_per_sec <= 260_000.0 {
            let hours = sec_per_sec / 3600.0;
            if (hours - hours.round()).abs() < 0.05 {
                format!("1s = {:.0} hr", hours.round())
            } else {
                format!("1s = {hours:.1} hr")
            }
        } else if years_per_sec < 0.0833 {
            let days = years_per_sec * 365.25;
            format!("{:.1}x (1s = {:.1} days)", self.multiplier, days)
        } else if years_per_sec < 1.0 {
            let months = years_per_sec * 12.0;
            format!("{:.0}x (1s = {:.1} months)", self.multiplier, months)
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
    /// Accumulated visual simulation time in seconds for shaders and rendering animations.
    /// Pauses when time_warp is paused, freezing planetary rotation, clouds, and stellar boiling.
    #[serde(default)]
    pub visual_time_secs: f32,
}

impl SimTime {
    /// Decomposes `elapsed_years` into calendar components: `(years, months, days, hours, minutes, seconds)`.
    /// Uses the standard Julian year (`31,557,600` seconds = `12` months of `2,629,800` seconds).
    pub fn decompose_calendar_clock(&self) -> (u64, u32, u32, u32, u32, u32) {
        let total_secs = if self.elapsed_years.is_finite() && self.elapsed_years > 0.0 {
            (self.elapsed_years * TimeWarp::SECONDS_PER_YEAR).floor() as u128
        } else {
            0
        };
        let sec_per_year: u128 = 31_557_600;
        let sec_per_month: u128 = 2_629_800; // 31,557,600 / 12 (30.4375 days)
        let years = (total_secs / sec_per_year).min(u128::from(u64::MAX)) as u64;
        let rem_yr = total_secs % sec_per_year;
        let months = (rem_yr / sec_per_month) as u32;
        let rem_mo = rem_yr % sec_per_month;
        let days = (rem_mo / 86_400) as u32;
        let rem_day = rem_mo % 86_400;
        let hours = (rem_day / 3_600) as u32;
        let rem_hr = rem_day % 3_600;
        let minutes = (rem_hr / 60) as u32;
        let seconds = (rem_hr % 60) as u32;
        (years, months, days, hours, minutes, seconds)
    }

    /// Formats the live visual chronometer showing years, months, days, hours, minutes, and seconds.
    pub fn format_visual_clock(&self) -> String {
        let (years, months, days, hours, minutes, seconds) = self.decompose_calendar_clock();
        format!("{years}y {months:02}m {days:02}d | {hours:02}h {minutes:02}m {seconds:02}s")
    }
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
    /// Number of active celestial bodies in the previous step.
    pub last_body_count: usize,
    /// Sum of masses across all bodies in the previous step.
    pub last_total_mass: f64,
    /// Flag requesting immediate re-baselining of $E_0$ (e.g. from editor or player actions).
    pub needs_rebaseline: bool,
}

impl EnergyMonitor {
    /// Re-baselines initial total energy to `current_total_energy` and resets relative drift.
    pub fn rebaseline(&mut self, current_total_energy: f64, body_count: usize, total_mass: f64) {
        self.initial_total_energy = current_total_energy;
        self.last_body_count = body_count;
        self.last_total_mass = total_mass;
        self.relative_energy_drift = 0.0;
        self.needs_rebaseline = false;
        self.initialized = true;
    }

    /// Requests that the next physics step re-baselines initial energy.
    pub fn request_rebaseline(&mut self) {
        self.needs_rebaseline = true;
    }
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
            disk_mass: 0.00010, // ~33 Earth masses of solid dust (authentic Hayashi MMSN)
            inner_radius_au: 0.06, // Protostellar accretion boundary layer (~0.06 AU)
            outer_radius_au: 45.0, // Authentic Kuiper Belt boundary (~40-45 AU)
            density_power_law: 1.5,
            reference_temp_1au: 280.0,
            snow_line_au: 2.70, // Authentic Ice/Snow Line (Asteroid Belt transition)
            gas_disk_lifetime_yr: 60_000.0, // 60,000 years before complete photo-evaporative clearance
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
    /// 3D Internal planetary cutaway view showing core, mantle, crust, and plumes
    InternalCutaway,
    /// GPU-accelerated 3D volumetric magnetic field loops and magnetospheres
    MagneticFields,
    /// Dark matter halos (NFW potential profile & gravitational contours)
    DarkMatterHalos,
    /// Hide all diagnostic overlays for a clean view (no extra lines/gizmos)
    Hidden,
}

impl DiagnosticOverlayMode {
    pub fn cycle(&self) -> Self {
        match self {
            Self::Realistic => Self::SpectralComposition,
            Self::SpectralComposition => Self::HillSpheresAndGaps,
            Self::HillSpheresAndGaps => Self::InternalCutaway,
            Self::InternalCutaway => Self::MagneticFields,
            Self::MagneticFields => Self::DarkMatterHalos,
            Self::DarkMatterHalos => Self::Hidden,
            Self::Hidden => Self::Realistic,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Realistic => "Realistic PBR",
            Self::SpectralComposition => "Spectral Composition Map",
            Self::HillSpheresAndGaps => "Hill Spheres & Annular Gaps",
            Self::InternalCutaway => "Internal Cutaway & Core Geodynamics",
            Self::MagneticFields => "Magnetic Fields (GPU)",
            Self::DarkMatterHalos => "Dark Matter Halos (NFW Potential)",
            Self::Hidden => "No Overlay",
        }
    }
}

/// Configuration for circumstellar Habitable Zone and volatile frost lines.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct HabitableZoneConfig {
    /// Whether circumstellar habitable zone and frost line bands are rendered.
    pub is_enabled: bool,
}

impl Default for HabitableZoneConfig {
    fn default() -> Self {
        Self { is_enabled: true }
    }
}

/// Configuration for rendering gravitational Lagrange libration points (L1-L5) and Hill spheres.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct LagrangeOverlayConfig {
    /// When true, renders L1-L5 libration points, equilateral triangle lines, and Hill sphere.
    pub show_lagrange_points: bool,
    /// When true, also renders the Hill sphere wireframe around the selected body.
    pub show_hill_sphere: bool,
}

impl Default for LagrangeOverlayConfig {
    fn default() -> Self {
        Self {
            show_lagrange_points: false,
            show_hill_sphere: true,
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
    pub primary_entity: Option<Entity>,
    pub local_position: Vec3,
}

/// Pool of active visual impact shockwaves.
#[derive(Resource, Debug, Clone, Default)]
pub struct ImpactShockwavePool {
    pub shockwaves: Vec<ImpactShockwave>,
}

/// Active visual debris stream created during tidal Roche disruption of a moon or comet.
#[derive(Debug, Clone)]
pub struct RocheDebrisStream {
    pub primary_entity: Entity,
    pub primary_pos: Vec3,
    pub disruption_pos: Vec3,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub timer: f32,
    pub max_timer: f32,
    pub ice_fraction: f32,
    pub debris_mass_earth: f64,
    /// (radius_au, current_phase_rad, orbital_speed_rad_per_s, z_offset)
    pub fragments: Vec<(f32, f32, f32, f32)>,
}

/// Pool of active visual tidal disruption debris streams.
#[derive(Resource, Debug, Clone, Default)]
pub struct RocheDebrisPool {
    pub streams: Vec<RocheDebrisStream>,
}

/// Active visual relativistic tidal disruption event (TDE) plasma stream funneled into a black hole.
#[derive(Debug, Clone)]
pub struct TidalDisruptionStream {
    pub bh_entity: Entity,
    pub bh_pos: Vec3,
    pub disruption_pos: Vec3,
    pub r_tidal: f32,
    pub r_isco: f32,
    pub timer: f32,
    pub max_timer: f32,
    pub initial_mass_solar: f64,
    pub initial_velocity: Vec3,
    /// Plasma stream ribbons: (progress [0..1], radius, azimuthal_angle, vertical_thickness, intensity)
    pub stream_nodes: Vec<(f32, f32, f32, f32, f32)>,
}

/// Pool of active relativistic tidal disruption event (TDE) streams.
#[derive(Resource, Debug, Clone, Default)]
pub struct TidalDisruptionPool {
    pub streams: Vec<TidalDisruptionStream>,
}

/// Available player tools for interacting with and shaping the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlayerTool {
    #[default]
    Inspect,
    /// Interactive orbital slingshot launcher with real-time trajectory forecast
    Slingshot,
    /// Apply a velocity impulse ($\Delta \vec{v}$) vector to a selected body
    GravitationalImpulse,
    /// Position a virtual gravitational tractor to gently redirect bodies
    GravitationalTractor,
    /// Inject a clump of mass/particles into an orbital ring
    MassInjection,
    /// Trigger a spiral density wave perturbation
    DensityWave,
}

/// Archetypes for launching new celestial bodies via the Interactive Slingshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SlingshotArchetype {
    #[default]
    Asteroid,
    Comet,
    TerrestrialPlanet,
    WaterWorld,
    GasGiant,
    RoguePlanet,
}

impl SlingshotArchetype {
    pub fn cycle(self) -> Self {
        match self {
            Self::Asteroid => Self::Comet,
            Self::Comet => Self::TerrestrialPlanet,
            Self::TerrestrialPlanet => Self::WaterWorld,
            Self::WaterWorld => Self::GasGiant,
            Self::GasGiant => Self::RoguePlanet,
            Self::RoguePlanet => Self::Asteroid,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Asteroid => "Asteroid",
            Self::Comet => "Comet",
            Self::TerrestrialPlanet => "Terrestrial Planet",
            Self::WaterWorld => "Water World",
            Self::GasGiant => "Gas Giant",
            Self::RoguePlanet => "Rogue Planet",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Asteroid => "🪨",
            Self::Comet => "☄️",
            Self::TerrestrialPlanet => "🌍",
            Self::WaterWorld => "🌊",
            Self::GasGiant => "🪐",
            Self::RoguePlanet => "🚀",
        }
    }
}

/// Global resource tracking interactive orbital slingshot state.
#[derive(Resource, Debug, Clone)]
pub struct SlingshotState {
    pub is_active: bool,
    /// Launch origin point on the orbital plane ($Y = 0$) or locked entity position
    pub drag_origin: Option<DVec3>,
    /// Current mouse drag position on the orbital plane ($Y = 0$)
    pub drag_current: Option<DVec3>,
    /// Optional target entity if applying a gravitational slingshot kick to an existing body
    pub target_entity: Option<Entity>,
    /// Archetype of body to spawn when releasing
    pub archetype: SlingshotArchetype,
    /// Velocity scaling factor: converts drag distance in AU into launch speed in AU/yr
    pub velocity_scale: f64,
}

impl Default for SlingshotState {
    fn default() -> Self {
        Self {
            is_active: false,
            drag_origin: None,
            drag_current: None,
            target_entity: None,
            archetype: SlingshotArchetype::Asteroid,
            velocity_scale: 8.0,
        }
    }
}

/// Controls which orbital trails and conic overlays are drawn in the viewport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OrbitVisualizationMode {
    /// Render orbital ribbons and conics strictly for the currently selected body.
    #[default]
    SelectedOnly,
    /// Render orbital ribbons and conics for all planets, companions, and embryos.
    All,
    /// Completely hide all orbital trails for a clean cinematic view.
    Off,
}

impl OrbitVisualizationMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::SelectedOnly => Self::All,
            Self::All => Self::Off,
            Self::Off => Self::SelectedOnly,
        }
    }

    pub fn display_label(self) -> &'static str {
        match self {
            Self::SelectedOnly => "Selected",
            Self::All => "All",
            Self::Off => "Hidden",
        }
    }
}

/// Global state tracking player interaction and tool selection.
#[derive(Resource, Debug, Clone, Default)]
pub struct PlayerInteractionState {
    pub selected_entity: Option<Entity>,
    pub hovered_entity: Option<Entity>,
    pub active_tool: PlayerTool,
    pub overlay_mode: DiagnosticOverlayMode,
    pub orbit_mode: OrbitVisualizationMode,
    pub tractor_position: Option<DVec3>,
    pub tractor_mass: f64,
    pub impulse_delta_v: Option<DVec3>,
    pub impulse_target_entity: Option<Entity>,
    /// Set true when celestial body selection was initiated by a UI interaction (e.g. Quick Bar),
    /// preventing 3D mouse picking from accidentally overriding the target in the same frame.
    pub just_selected_via_ui: bool,
    /// Whether 3D internal planetary cutaway / cross-section mode is active for selected or inspected bodies.
    pub cutaway_mode: bool,
    /// Visual view toggle: strips planetary atmospheres and primary gas envelopes to reveal solid/molten cores & terrain underneath.
    pub hide_atmospheres: bool,
    /// Visual view toggle: hides cloud decks and opaque haze shrouds while retaining underlying atmospheres and Rayleigh/Chapman scattering.
    pub hide_clouds: bool,
}

impl PlayerInteractionState {
    /// Returns true when planetary atmospheres and gas envelopes should be rendered.
    #[must_use]
    pub const fn show_atmospheres(&self) -> bool {
        !self.hide_atmospheres
    }

    /// Returns true when cloud decks should be rendered atop planets.
    #[must_use]
    pub const fn show_clouds(&self) -> bool {
        !self.hide_clouds
    }
}

impl PlayerInteractionState {
    /// Returns true if 3D internal cutaway mode is enabled via hotkey toggle or overlay mode.
    pub fn is_cutaway_active(&self) -> bool {
        self.cutaway_mode || self.overlay_mode == DiagnosticOverlayMode::InternalCutaway
    }
}

#[derive(Debug, Clone)]
pub struct ImpactEjectaRing {
    pub primary_entity: Entity,
    pub primary_pos: Vec3,
    pub impact_pos: Vec3,
    pub timer: f32,
    pub max_timer: f32,
    pub ring_mass: f64,
    pub color: Color,
    pub fragments: Vec<(f32, f32, f32, f32)>, // r, phi, omega, z
}

/// Individual 3D molten collision ejecta fragment exhibiting Keplerian orbital motion,
/// radiative blackbody cooling, trailing plasma ribbons, and re-impact mechanics.
#[derive(Debug, Clone)]
pub struct MoltenDebrisFragment {
    /// World position in AU.
    pub position: Vec3,
    /// World velocity in AU/yr.
    pub velocity: Vec3,
    /// Initial peak temperature in Kelvin (e.g. 2200 - 3800 K).
    pub initial_temperature: f32,
    /// Current cooling temperature in Kelvin.
    pub temperature: f32,
    /// Radiative cooling rate in s^-1.
    pub cooling_rate: f32,
    /// Visual fragment radius in AU.
    pub radius: f32,
    /// Estimated mass in Solar Masses.
    pub mass: f64,
    /// Primary parent body being orbited.
    pub primary_entity: Entity,
    /// Time elapsed since ejection in visual seconds.
    pub age: f32,
    /// Maximum duration before complete cooling/condensation in visual seconds.
    pub max_age: f32,
    /// Recent past positions for trailing ribbon streamer.
    pub trail: [Vec3; 4],
    /// Local position relative to primary entity center in AU.
    pub local_position: Vec3,
    /// Local ballistic velocity relative to primary entity in AU/s.
    pub local_velocity: Vec3,
    /// Local past positions relative to primary entity center.
    pub local_trail: [Vec3; 4],
    /// Optional color tint for icy/volatile ablation plumes.
    pub color_tint: Option<Color>,
}

impl MoltenDebrisFragment {
    /// Evaluates blackbody thermal emission color based on current cooling temperature and age.
    pub fn blackbody_color(&self) -> Color {
        let t = self.temperature;
        let p = (self.age / self.max_age.max(0.01)).clamp(0.0, 1.0);
        let alpha = ((1.0 - p * p) * 0.95).clamp(0.0, 1.0);

        if t > 2800.0 {
            // White-hot plasma core with high emissivity
            let blend = ((t - 2800.0) / 1000.0).clamp(0.0, 1.0);
            Color::srgba(1.0, 0.95 + blend * 0.05, 0.80 + blend * 0.20, alpha)
        } else if t > 2000.0 {
            // Molten gold-yellow silicate magma
            let blend = ((t - 2000.0) / 800.0).clamp(0.0, 1.0);
            Color::srgba(1.0, 0.65 + blend * 0.30, 0.15 + blend * 0.65, alpha)
        } else if t > 1200.0 {
            // Blazing incandescent orange-red lava
            let blend = ((t - 1200.0) / 800.0).clamp(0.0, 1.0);
            Color::srgba(1.0, 0.25 + blend * 0.40, 0.06 + blend * 0.09, alpha)
        } else if t > 600.0 {
            // Deep crimson cherry lava
            let blend = ((t - 600.0) / 600.0).clamp(0.0, 1.0);
            Color::srgba(
                0.55 + blend * 0.45,
                0.08 + blend * 0.17,
                0.02 + blend * 0.04,
                alpha * 0.9,
            )
        } else {
            // Dark cooling basalt silicate rock
            let blend = (t / 600.0).clamp(0.0, 1.0);
            Color::srgba(
                0.20 + blend * 0.35,
                0.18 + blend * 0.05,
                0.16 + blend * 0.02,
                alpha * 0.75,
            )
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ImpactEjectaPool {
    pub rings: Vec<ImpactEjectaRing>,
    pub fragments: Vec<MoltenDebrisFragment>,
}
