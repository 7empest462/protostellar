//! Multi-epoch planetary telemetry recording, habitability tracking, and CSV export engine.

use bevy::math::DVec3;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::Write as IoWrite;
use std::path::Path;

const SPARKLINE_BLOCKS: &[char] = &[' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// A single time-stamped snapshot of a celestial body's physical, orbital, and climate state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanetaryTelemetrySample {
    pub sim_time_yr: f64,
    pub body_name: String,
    pub mass_earth: f64,
    pub radius_km: f64,
    pub semi_major_axis_au: f64,
    pub eccentricity: f64,
    pub surface_temp_k: f64,
    pub atmospheric_pressure_bar: f32,
    pub ocean_coverage_frac: f32,
    pub habitability_score: f32,
    pub biomass_coverage_frac: f32,
}

/// Active metric selected for graphing in the Telemetry HUD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TelemetryMetric {
    #[default]
    Habitability,
    Temperature,
    OceanCoverage,
    AtmosphericPressure,
    Orbit,
}

impl TelemetryMetric {
    pub fn display_title(&self) -> &'static str {
        match self {
            Self::Habitability => "Biosphere Habitability Index",
            Self::Temperature => "Surface Equilibrium Temperature",
            Self::OceanCoverage => "Liquid Water Ocean Coverage",
            Self::AtmosphericPressure => "Atmospheric Surface Pressure",
            Self::Orbit => "Orbital Eccentricity & Distance",
        }
    }

    pub fn unit_label(&self) -> &'static str {
        match self {
            Self::Habitability => "% (0% Sterile - 100% Eden)",
            Self::Temperature => "K / °C",
            Self::OceanCoverage => "% Surface Liquid Water",
            Self::AtmosphericPressure => "bar (Earth = 1.00 bar)",
            Self::Orbit => "AU / Eccentricity",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Habitability => "Habitability",
            Self::Temperature => "Temperature",
            Self::OceanCoverage => "Ocean Coverage",
            Self::AtmosphericPressure => "Atm Pressure",
            Self::Orbit => "Orbit (a, e)",
        }
    }
}

/// Ring-buffer historical telemetry for the currently tracked planetary world.
#[derive(Resource, Debug, Clone)]
pub struct SimulationTelemetryHistory {
    /// History samples stored chronologically (oldest at front, newest at back).
    pub samples: VecDeque<PlanetaryTelemetrySample>,
    /// Maximum ring buffer capacity (default: 128 samples).
    pub max_samples: usize,
    /// Simulation time (in years) when the last sample was captured.
    pub last_sample_yr: f64,
    /// Downsampled sampling interval in simulation years.
    pub sample_interval_yr: f64,
    /// Entity ID currently being tracked.
    pub tracked_entity: Option<Entity>,
    /// Name of currently tracked world.
    pub tracked_name: String,
    /// Last export status message for UI display.
    pub last_export_status: Option<String>,
}

impl Default for SimulationTelemetryHistory {
    fn default() -> Self {
        Self {
            samples: VecDeque::with_capacity(128),
            max_samples: 128,
            last_sample_yr: 0.0,
            sample_interval_yr: 25.0,
            tracked_entity: None,
            tracked_name: "Proto-Earth".to_string(),
            last_export_status: None,
        }
    }
}

impl SimulationTelemetryHistory {
    /// Pushes a new sample into the ring buffer, evicting the oldest sample if capacity is exceeded.
    pub fn push_sample(&mut self, sample: PlanetaryTelemetrySample) {
        if self.samples.len() >= self.max_samples {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Clears the history buffer (e.g. when user switches tracking to a different planet).
    pub fn clear(&mut self) {
        self.samples.clear();
        self.last_sample_yr = 0.0;
    }

    /// Returns the number of recorded telemetry samples in the ring buffer.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Returns (latest, min, max, mean) for the chosen metric across the current sample buffer.
    pub fn get_metric_stats(&self, metric: TelemetryMetric) -> (f64, f64, f64, f64) {
        if self.samples.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let extract = |s: &PlanetaryTelemetrySample| match metric {
            TelemetryMetric::Habitability => f64::from(s.habitability_score * 100.0),
            TelemetryMetric::Temperature => s.surface_temp_k,
            TelemetryMetric::OceanCoverage => f64::from(s.ocean_coverage_frac * 100.0),
            TelemetryMetric::AtmosphericPressure => f64::from(s.atmospheric_pressure_bar),
            TelemetryMetric::Orbit => s.semi_major_axis_au,
        };

        let latest = self.samples.back().map_or(0.0, extract);
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut sum = 0.0;
        for s in &self.samples {
            let val = extract(s);
            if val < min {
                min = val;
            }
            if val > max {
                max = val;
            }
            sum += val;
        }
        let mean = sum / (self.samples.len() as f64);
        (latest, min, max, mean)
    }

    /// Generates a Unicode block sparkline graph (e.g. " ▂▃▅▆▇█") from the recorded history.
    pub fn generate_sparkline(&self, metric: TelemetryMetric, max_chars: usize) -> String {
        if self.samples.is_empty() {
            return "No historical telemetry points recorded yet.".to_string();
        }

        let values: Vec<f64> = self
            .samples
            .iter()
            .map(|s| match metric {
                TelemetryMetric::Habitability => f64::from(s.habitability_score * 100.0),
                TelemetryMetric::Temperature => s.surface_temp_k,
                TelemetryMetric::OceanCoverage => f64::from(s.ocean_coverage_frac * 100.0),
                TelemetryMetric::AtmosphericPressure => f64::from(s.atmospheric_pressure_bar),
                TelemetryMetric::Orbit => s.eccentricity,
            })
            .collect();

        let n = values.len();
        let step = ((n as f64) / (max_chars as f64)).max(1.0);
        let mut sampled_values = Vec::with_capacity(max_chars);

        let mut idx = 0.0;
        while (idx as usize) < n && sampled_values.len() < max_chars {
            if let Some(&val) = values.get(idx as usize) {
                sampled_values.push(val);
            }
            idx += step;
        }

        let mut min_val = f64::INFINITY;
        let mut max_val = f64::NEG_INFINITY;
        for &v in &sampled_values {
            if v < min_val {
                min_val = v;
            }
            if v > max_val {
                max_val = v;
            }
        }

        let range = max_val - min_val;
        let mut out = String::with_capacity(sampled_values.len());

        for v in sampled_values {
            if range < 1e-6 {
                if let Some(&block) = SPARKLINE_BLOCKS.get(3) {
                    out.push(block);
                }
            } else {
                let frac = ((v - min_val) / range).clamp(0.0, 1.0);
                let block_idx = (frac * (SPARKLINE_BLOCKS.len() - 1) as f64).round() as usize;
                let clamped_idx = block_idx.min(SPARKLINE_BLOCKS.len().saturating_sub(1));
                if let Some(&block) = SPARKLINE_BLOCKS.get(clamped_idx) {
                    out.push(block);
                }
            }
        }

        out
    }

    /// Serializes the entire time-series sample buffer into standard CSV text.
    pub fn serialize_to_csv(&self) -> String {
        let mut csv = String::with_capacity(self.samples.len() * 128 + 256);
        csv.push_str("Epoch_yr,BodyName,Mass_Mearth,Radius_km,SemiMajorAxis_AU,Eccentricity,SurfaceTemp_K,AtmPressure_bar,OceanCoverage_pct,Habitability_pct,Biomass_pct\n");

        for s in &self.samples {
            let _ = writeln!(
                csv,
                "{:.2},{},{:.6},{:.1},{:.4},{:.4},{:.2},{:.4},{:.2},{:.2},{:.2}",
                s.sim_time_yr,
                s.body_name,
                s.mass_earth,
                s.radius_km,
                s.semi_major_axis_au,
                s.eccentricity,
                s.surface_temp_k,
                s.atmospheric_pressure_bar,
                s.ocean_coverage_frac * 100.0,
                s.habitability_score * 100.0,
                s.biomass_coverage_frac * 100.0
            );
        }

        csv
    }

    /// Exports the recorded telemetry samples to a CSV file on disk.
    pub fn export_to_csv_file(&mut self, custom_path: Option<&Path>) -> Result<String, String> {
        if self.samples.is_empty() {
            return Err("No telemetry data to export (buffer is empty).".to_string());
        }

        let csv_content = self.serialize_to_csv();
        let target_path = if let Some(p) = custom_path {
            p.to_path_buf()
        } else {
            let export_dir = Path::new("exports");
            if !export_dir.exists() {
                let _ = fs::create_dir_all(export_dir);
            }
            let sanitized_name = self.tracked_name.replace([' ', '/', '\\', '#'], "_");
            let epoch = self.last_sample_yr as u64;
            export_dir.join(format!("telemetry_{sanitized_name}_{epoch}yr.csv"))
        };

        match File::create(&target_path) {
            Ok(mut file) => match file.write_all(csv_content.as_bytes()) {
                Ok(()) => {
                    let path_str = target_path.display().to_string();
                    let msg = format!("Exported {} samples to {path_str}", self.samples.len());
                    self.last_export_status = Some(msg.clone());
                    Ok(path_str)
                }
                Err(e) => Err(format!("Failed to write CSV file: {e}")),
            },
            Err(e) => Err(format!("Failed to create CSV file: {e}")),
        }
    }
}

fn calculate_orbital_elements(pos: DVec3, vel: DVec3, star_mass: f64) -> (f64, f64) {
    let r = pos.length().max(0.001);
    let v_sq = vel.length_squared();
    let mu = G_ASTRO * star_mass.max(0.01);
    let specific_energy = 0.5 * v_sq - mu / r;

    if specific_energy >= 0.0 {
        // Hyperbolic or parabolic trajectory
        let a = (mu / (2.0 * specific_energy.abs())).max(0.01);
        let h_vec = pos.cross(vel);
        let h_sq = h_vec.length_squared();
        let e = (1.0 + (2.0 * specific_energy * h_sq) / (mu * mu))
            .max(1.0)
            .sqrt();
        (a, e)
    } else {
        // Elliptical bound orbit
        let a = (-mu / (2.0 * specific_energy)).max(0.01);
        let h_vec = pos.cross(vel);
        let h_sq = h_vec.length_squared();
        let e_sq = (1.0 - h_sq / (mu * a)).max(0.0);
        (a, e_sq.sqrt())
    }
}

/// Periodically samples telemetry from the currently tracked or selected body.
#[allow(
    clippy::type_complexity,
    reason = "Telemetry queries multiple planet state components"
)]
pub fn record_planetary_telemetry(
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    player_state: Res<PlayerInteractionState>,
    _disk_params: Res<DiskParameters>,
    mut telemetry: ResMut<SimulationTelemetryHistory>,
    star_query: Query<&Mass, With<CentralStar>>,
    bodies_query: Query<(
        Entity,
        &CelestialBody,
        &Mass,
        &Radius,
        &SimPosition,
        &SimVelocity,
        Option<&Temperature>,
        Option<&VolatileInventory>,
        Option<&BiosphereState>,
    )>,
) {
    let t = sim_time.elapsed_years;
    let star_mass = star_query.iter().next().map_or(1.0, |m| m.0);

    // Determine target entity: player selected planet, or previous tracked planet, or auto-fallback to terrestrial planet.
    // Stars, black holes, and non-planetary bodies are excluded from Climate & Habitability telemetry.
    let target_entity = player_state
        .selected_entity
        .filter(|&ent| {
            bodies_query
                .get(ent)
                .is_ok_and(|(_, b, _, _, _, _, _, _, _)| {
                    !b.body_type.is_star_or_remnant() && b.body_type != BodyType::BlackHole
                })
        })
        .or_else(|| {
            telemetry.tracked_entity.filter(|&ent| {
                bodies_query
                    .get(ent)
                    .is_ok_and(|(_, b, _, _, _, _, _, _, _)| {
                        !b.body_type.is_star_or_remnant() && b.body_type != BodyType::BlackHole
                    })
            })
        })
        .or_else(|| {
            bodies_query
                .iter()
                .find(|(_, b, _, _, _, _, _, _, _)| {
                    b.body_type == BodyType::TerrestrialPlanet
                        || b.name == "Earth"
                        || b.name.contains("Earth")
                })
                .map(|(e, _, _, _, _, _, _, _, _)| e)
        })
        .or_else(|| {
            bodies_query
                .iter()
                .find(|(_, b, _, _, _, _, _, _, _)| {
                    !b.body_type.is_star_or_remnant() && b.body_type != BodyType::BlackHole
                })
                .map(|(e, _, _, _, _, _, _, _, _)| e)
        });

    let Some(entity) = target_entity else {
        return;
    };

    let Ok((_, body, mass, rad, pos, vel, opt_temp, opt_vol, opt_bio)) = bodies_query.get(entity)
    else {
        return;
    };

    // If tracked entity changed, clear previous history and take an immediate baseline sample (even if paused)
    let is_target_changed = telemetry.tracked_entity != Some(entity);
    if is_target_changed {
        telemetry.clear();
        telemetry.tracked_entity = Some(entity);
        telemetry.tracked_name.clone_from(&body.name);
    } else {
        if time_warp.is_paused && !time_warp.step_once {
            return;
        }

        let effective_interval =
            telemetry.sample_interval_yr * (time_warp.multiplier / 2.0).clamp(1.0, 50.0);
        if (t - telemetry.last_sample_yr).abs() < effective_interval && telemetry.samples.len() >= 2
        {
            return;
        }
    }

    let mass_earth = mass.0 / EARTH_MASS_SOLAR;
    let radius_km = rad.0 * AU_TO_KM;
    let (semi_major_axis_au, eccentricity) = calculate_orbital_elements(pos.0, vel.0, star_mass);

    let surface_temp_k = opt_temp.map_or(288.0, |temp| temp.0);
    let atmospheric_pressure_bar = opt_vol.map_or(0.0, |vol| vol.atmospheric_pressure_bar);
    let ocean_coverage_frac = opt_vol.map_or(0.0, |vol| vol.ocean_coverage_frac);
    let habitability_score = opt_bio.map_or(0.0, |bio| bio.habitability_score);
    let biomass_coverage_frac = opt_bio.map_or(0.0, |bio| bio.biomass_coverage_frac);

    let sample = PlanetaryTelemetrySample {
        sim_time_yr: t,
        body_name: body.name.clone(),
        mass_earth,
        radius_km,
        semi_major_axis_au,
        eccentricity,
        surface_temp_k,
        atmospheric_pressure_bar,
        ocean_coverage_frac,
        habitability_score,
        biomass_coverage_frac,
    };

    telemetry.push_sample(sample);
    telemetry.last_sample_yr = t;
}
