//! Exoplanet System Generator and Sandbox Scenarios.
//!
//! Provides multi-system presets:
//! - TRAPPIST-1 Resonant Ultracool Red Dwarf System (7 Earths)
//! - Kepler-16 Circumbinary System (Tatooine-like binary star pair with circumbinary planet)
//! - Hot Jupiter Inward Migration Scenario (Type II disk migration)
//! - Rogue Planet Flyby Perturbation Scenario (Hyperbolic interstellar interloper)
//! - Hayashi Minimum Mass Solar Nebula (Default Solar System)

pub mod exotic;
pub mod kepler;
pub mod solar;
pub mod trappist;

pub use exotic::*;
pub use kepler::*;
pub use solar::*;
pub use trappist::*;

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;

/// Supported Sandbox Scenario Presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
pub enum ScenarioPreset {
    #[default]
    SolarNebulaMmsn,
    Trappist1System,
    Kepler16Circumbinary,
    HotJupiterMigration,
    RoguePlanetFlyby,
    LittleRedDot,
    PulsarSystem,
    MagnetarOutburst,
}

impl ScenarioPreset {
    pub fn display_name(&self) -> &'static str {
        match self {
            ScenarioPreset::SolarNebulaMmsn => "Hayashi Solar Nebula",
            ScenarioPreset::Trappist1System => "TRAPPIST-1 (7 Resonant Earths)",
            ScenarioPreset::Kepler16Circumbinary => "Kepler-16 (Circumbinary Binary)",
            ScenarioPreset::HotJupiterMigration => "Hot Jupiter Migration",
            ScenarioPreset::RoguePlanetFlyby => "Rogue Planet Flyby",
            ScenarioPreset::LittleRedDot => "JWST Little Red Dot (Black Hole Star)",
            ScenarioPreset::PulsarSystem => "PSR B1257+12 (Pulsar & Zombie Planets)",
            ScenarioPreset::MagnetarOutburst => "SGR 1806-20 (Magnetar Giant Flare)",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ScenarioPreset::SolarNebulaMmsn => {
                "Default 4.5 Gyr Hayashi Minimum Mass Solar Nebula (MMSN) with central protostar and 10 protoplanetary embryos."
            }
            ScenarioPreset::Trappist1System => {
                "Ultracool M-dwarf (0.09 M☉) with 7 Earth-sized terrestrial worlds in a compact resonant Laplace chain (3 habitable)."
            }
            ScenarioPreset::Kepler16Circumbinary => {
                "Tatooine-like circumbinary system with K/M-dwarf binary pair and a Saturn-mass circumbinary giant at 0.70 AU."
            }
            ScenarioPreset::HotJupiterMigration => {
                "Massive gas giant (1.4 M_Jup) undergoing Type II disk torque inward migration from 5.2 AU down to 0.045 AU."
            }
            ScenarioPreset::RoguePlanetFlyby => {
                "A 3.5 M_Jup interstellar rogue planet screaming through the solar system at 38 km/s, scattering orbits."
            }
            ScenarioPreset::LittleRedDot => {
                "Cosmic Dawn (z ~ 8.5): A 100,000 M☉ supermassive black hole seed encased in a dense, dust-free primordial hydrogen gas cocoon spanning 60 AU."
            }
            ScenarioPreset::PulsarSystem => {
                "Millisecond pulsar (1.40 M☉, 6.22 ms relativistic spin) with 3 confirmed zombie exoplanets (Draugr, Poltergeist, Phobetor) and an irradiated post-supernova fallback disk."
            }
            ScenarioPreset::MagnetarOutburst => {
                "Ultra-magnetized 10¹⁵ Gauss magnetar (SGR 1806-20) with starquake crustal fractures, glowing magnetic arches, an LBV hypergiant companion, and relativistic ejecta."
            }
        }
    }
}

/// Message event to request loading a new sandbox scenario.
#[derive(Event, Message, Debug, Clone, Copy)]
pub struct LoadScenarioEvent(pub ScenarioPreset);

/// Active state tracking for scenarios with ongoing events (e.g. Migration, Flyby).
#[derive(Resource, Debug, Clone, Default)]
pub struct ActiveScenarioState {
    pub current_preset: ScenarioPreset,
    pub scenario_time_years: f64,
    pub migration_active: bool,
    pub migration_target_au: f64,
    pub rogue_planet_entity: Option<Entity>,
}

/// System that listens for `LoadScenarioEvent` and reinitializes the entire simulation.
pub fn handle_load_scenario_events(
    mut commands: Commands,
    mut events: MessageReader<LoadScenarioEvent>,
    mut disk_params: ResMut<DiskParameters>,
    mut sim_time: ResMut<SimTime>,
    mut energy_monitor: ResMut<EnergyMonitor>,
    mut time_warp: ResMut<TimeWarp>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut scenario_state: ResMut<ActiveScenarioState>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    bodies_query: Query<Entity, With<CelestialBody>>,
    mut camera_query: Query<&mut crate::rendering::camera::PanOrbitCamera>,
    mut swarm: Option<ResMut<crate::rendering::particle_swarm::ParticleSwarmData>>,
    mut config: ResMut<SimulationConfig>,
    mut swarm_mesh_query: Query<
        &mut Visibility,
        With<crate::rendering::particle_swarm::ParticleSwarmMesh>,
    >,
) {
    for event in events.read() {
        let preset = event.0;
        info!("🌟 Loading Scenario Preset: {:?}", preset);

        for ent in bodies_query.iter() {
            if let Ok(mut cmd) = commands.get_entity(ent) {
                cmd.despawn();
            }
        }

        sim_time.elapsed_years = 0.0;
        sim_time.current_dt_yr = 0.001;
        time_warp.multiplier = 1.0;
        time_warp.is_paused = false;
        energy_monitor.initial_total_energy = 0.0;
        energy_monitor.kinetic_energy = 0.0;
        energy_monitor.potential_energy = 0.0;
        energy_monitor.total_energy = 0.0;
        energy_monitor.relative_energy_drift = 0.0;
        energy_monitor.initialized = false;
        lhb_state.is_active = false;
        lhb_state.migration_progress = 0.0;
        lhb_state.resonance_crossed = false;

        scenario_state.current_preset = preset;
        scenario_state.scenario_time_years = 0.0;
        scenario_state.migration_active = false;
        match preset {
            ScenarioPreset::Trappist1System
            | ScenarioPreset::PulsarSystem
            | ScenarioPreset::MagnetarOutburst => {
                config.gas_density_scale = 0.0;
                disk_params.gas_disk_lifetime_yr = 0.0;
            }
            ScenarioPreset::SolarNebulaMmsn => {
                config.gas_density_scale = 1.0;
                disk_params.gas_disk_lifetime_yr = 5.0e6;
            }
            _ => {}
        }

        let central_star_ent = match preset {
            ScenarioPreset::SolarNebulaMmsn => {
                spawn_solar_nebula_mmsn(&mut commands, &mut disk_params)
            }
            ScenarioPreset::Trappist1System => {
                spawn_trappist_1_system(&mut commands, &mut disk_params)
            }
            ScenarioPreset::Kepler16Circumbinary => {
                spawn_kepler_16_system(&mut commands, &mut disk_params)
            }
            ScenarioPreset::HotJupiterMigration => {
                scenario_state.migration_active = true;
                scenario_state.migration_target_au = 0.045;
                spawn_hot_jupiter_scenario(&mut commands, &mut disk_params)
            }
            ScenarioPreset::RoguePlanetFlyby => {
                let (star, rogue) = spawn_rogue_planet_scenario(&mut commands, &mut disk_params);
                scenario_state.rogue_planet_entity = Some(rogue);
                star
            }
            ScenarioPreset::LittleRedDot => {
                spawn_little_red_dot_scenario(&mut commands, &mut disk_params)
            }
            ScenarioPreset::PulsarSystem => {
                spawn_pulsar_system_scenario(&mut commands, &mut disk_params)
            }
            ScenarioPreset::MagnetarOutburst => {
                spawn_magnetar_outburst_scenario(&mut commands, &mut disk_params)
            }
        };

        player_state.selected_entity = Some(central_star_ent);

        let is_empty_swarm = matches!(
            preset,
            ScenarioPreset::PulsarSystem | ScenarioPreset::MagnetarOutburst
        ) || disk_params.disk_mass <= 0.0;

        config.active_particles = if is_empty_swarm {
            0
        } else {
            config.target_particle_count as u32
        };

        for mut vis in swarm_mesh_query.iter_mut() {
            *vis = if is_empty_swarm {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }

        if let Some(ref mut swarm_data) = swarm {
            crate::rendering::particle_swarm::reseed_particle_swarm(
                swarm_data,
                &disk_params,
                &config,
            );
        }

        if let Some(mut cam) = camera_query.iter_mut().next() {
            cam.focus = Vec3::ZERO;
            cam.target_focus = Vec3::ZERO;
            cam.target_entity = Some(central_star_ent);
            let (target_r, target_yaw, target_pitch) = match preset {
                ScenarioPreset::Trappist1System => (0.12, 0.785, 0.75),
                ScenarioPreset::Kepler16Circumbinary => (2.2, 0.785, 0.65),
                ScenarioPreset::SolarNebulaMmsn => (16.0, 0.785, 0.62),
                ScenarioPreset::HotJupiterMigration => (10.0, 0.785, 0.62),
                ScenarioPreset::RoguePlanetFlyby => (35.0, 0.785, 0.62),
                ScenarioPreset::LittleRedDot => (160.0, 0.785, 0.62),
                ScenarioPreset::PulsarSystem => (1.2, 0.785, 0.65),
                ScenarioPreset::MagnetarOutburst => (6.5, 0.785, 0.62),
            };
            cam.radius = target_r;
            cam.target_radius = target_r;
            cam.yaw = target_yaw;
            cam.target_yaw = target_yaw;
            cam.pitch = target_pitch;
            cam.target_pitch = target_pitch;
        }
    }
}

/// System to execute continuous scenario dynamics (e.g. Type II migration drag).
pub fn update_active_scenarios(
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    mut scenario_state: ResMut<ActiveScenarioState>,
    mut bodies_query: Query<(&mut SimVelocity, &SimPosition, &mut CelestialBody, &Mass)>,
) {
    if time_warp.is_paused {
        return;
    }

    let dt = sim_time.current_dt_yr;
    scenario_state.scenario_time_years += dt;

    if scenario_state.migration_active
        && scenario_state.current_preset == ScenarioPreset::HotJupiterMigration
    {
        for (mut vel, pos, mut body, _mass) in bodies_query.iter_mut() {
            if body.name.contains("Hot Jupiter") {
                let r = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt();
                if r > scenario_state.migration_target_au {
                    let v_dir = vel.0.normalize_or_zero();
                    let inward_thrust = 0.0025 * (r / 5.2).clamp(0.2, 1.0);
                    vel.0 -= v_dir * (inward_thrust * dt);
                } else if !body.name.contains("Parked") {
                    body.name = "Ultra-Short Period Hot Jupiter (Parked)".to_string();
                }
            }
        }
    }
}
