//! Scenario presets loading and Little Red Dot experimental actions.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::scenarios::{LoadScenarioEvent, ScenarioPreset};

use super::super::types::*;

pub fn handle_scenario_action(
    action: &UiButtonAction,
    scenario_events: &mut MessageWriter<LoadScenarioEvent>,
    toast: &mut NotificationToast,
    quasi_star_query: &mut Query<&mut BlackHoleStarState>,
    commands: &mut Commands,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
) -> bool {
    if handle_scenario_preset_load(action, scenario_events, toast) {
        return true;
    }
    handle_little_red_dot_action(
        action,
        quasi_star_query,
        toast,
        commands,
        player_state,
        camera_query,
    )
}

fn handle_scenario_preset_load(
    action: &UiButtonAction,
    scenario_events: &mut MessageWriter<LoadScenarioEvent>,
    toast: &mut NotificationToast,
) -> bool {
    match action {
        UiButtonAction::LoadScenarioSolar => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::SolarNebulaMmsn));
            toast.message = "🪐 Loaded Scenario: Hayashi Solar Nebula (MMSN)".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioGenesis => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::AccretionDiskGenesis));
            toast.message =
                "🌌 Loaded Scenario: Protoplanetary Disk Genesis (Zero Planets, SPH Gas Drag & Organic Accretion)"
                    .to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioTrappist => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::Trappist1System));
            toast.message =
                "🔴 Loaded Scenario: TRAPPIST-1 (7 Resonant Earths, 3 Habitable)".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioKepler16 => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::Kepler16Circumbinary));
            toast.message =
                "☀️ Loaded Scenario: Kepler-16 'Tatooine' Circumbinary System".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioHotJupiter => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::HotJupiterMigration));
            toast.message = "🌀 Loaded Scenario: Hot Jupiter Type II Inward Migration".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioRoguePlanet => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::RoguePlanetFlyby));
            toast.message =
                "☄️ Loaded Scenario: Interstellar Rogue Planet Flyby Perturbation".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioLittleRedDot => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::LittleRedDot));
            toast.message =
                "🔴 Loaded Scenario: JWST Little Red Dot (100,000 M☉ Black Hole Star)".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioPulsar => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::PulsarSystem));
            toast.message =
                "⚡ Loaded Scenario: PSR B1257+12 (Lich & 3 Zombie Exoplanets)".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioMagnetar => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::MagnetarOutburst));
            toast.message =
                "🧲 Loaded Scenario: SGR 1806-20 (10¹⁵ G Magnetar & Giant Flare)".to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioRelativisticBinary => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::RelativisticBinary));
            toast.message =
                "⚡ Loaded Scenario: PSR B1913+16 (Relativistic Binary & Gravitational Waves)"
                    .to_string();
            toast.timer = 5.0;
            true
        }
        UiButtonAction::LoadScenarioKozaiTriple => {
            scenario_events.write(LoadScenarioEvent(ScenarioPreset::KozaiLidovTriple));
            toast.message =
                "🪐 Loaded Scenario: HD 80606 (Kozai-Lidov Resonance & Secular Migration)"
                    .to_string();
            toast.timer = 5.0;
            true
        }
        _ => false,
    }
}

fn handle_little_red_dot_action(
    action: &UiButtonAction,
    quasi_star_query: &mut Query<&mut BlackHoleStarState>,
    toast: &mut NotificationToast,
    commands: &mut Commands,
    player_state: &mut PlayerInteractionState,
    camera_query: &mut Query<&mut PanOrbitCamera>,
) -> bool {
    match action {
        UiButtonAction::ToggleSuperEddington => {
            let mut toggled = false;
            for mut state in quasi_star_query.iter_mut() {
                state.toggle_super_eddington();
                let mode = if state.super_eddington_active {
                    "4.5x Eddington (Hyper-Accretion Active)"
                } else {
                    "0.9x Eddington (Sub-Eddington Normal)"
                };
                toast.message = format!("⚡ Inflow Rate: {mode} on JWST Little Red Dot");
                toast.timer = 4.5;
                toggled = true;
            }
            if !toggled {
                toast.message = "ℹ️ No Quasi-Star present in active simulation.".to_string();
                toast.timer = 3.0;
            }
            true
        }
        UiButtonAction::TriggerBlowoutCocoon => {
            let mut triggered = false;
            for mut state in quasi_star_query.iter_mut() {
                state.trigger_blowout();
                toast.message = "💥 COCOON BLOWOUT: Radiation pressure stripping hydrogen envelope to unveil Supermassive Quasar!".to_string();
                toast.timer = 6.0;
                triggered = true;
            }
            if !triggered {
                toast.message = "ℹ️ No Quasi-Star present in active simulation.".to_string();
                toast.timer = 3.0;
            }
            true
        }
        UiButtonAction::SpawnInfallPop3Star => {
            let r_au = 120.0;
            let total_m = 150_000.0;
            let v_circ = (crate::utils::constants::G_ASTRO * total_m / r_au).sqrt();
            let v_mag = v_circ * 0.38;
            let pos = DVec3::new(r_au, 0.0, 15.0);
            let vel = DVec3::new(-v_mag * 0.75, 0.0, -v_mag * 0.65);

            let new_star = commands
                .spawn((
                    CelestialBody {
                        name: "Infalling Pop-III Hypergiant (TDE Target)".to_string(),
                        body_type: BodyType::BlueSupergiant,
                    },
                    Mass(120.0),
                    SimPosition(pos),
                    SimVelocity(vel),
                    SimAcceleration(DVec3::ZERO),
                    Radius(0.012),
                    Temperature(45_000.0),
                    Composition::solar_gas(),
                    VolatileInventory::default(),
                    SpinState {
                        spin_vector: DVec3::new(0.0, 1e-10, 0.0),
                        rotation_period_hours: 24.0,
                        axial_tilt_degrees: 15.0,
                    },
                ))
                .id();

            player_state.selected_entity = Some(new_star);
            if let Ok(mut cam) = camera_query.single_mut() {
                cam.target_entity = Some(new_star);
            }
            toast.message = "🌟 Spawned 120 M☉ Pop-III Hypergiant plunging toward the 100,000 M☉ Black Hole Seed!".to_string();
            toast.timer = 5.5;
            true
        }
        _ => false,
    }
}
