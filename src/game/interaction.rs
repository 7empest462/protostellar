//! Player interaction tools: gravitational impulse, tractor, and mass injection.

use bevy::math::DVec3;
use bevy::prelude::*;
use rand::prelude::*;
use std::f64::consts::PI;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Handles player tool activation and direct live editing of celestial bodies.
pub fn handle_player_tools(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    disk_params: Res<DiskParameters>,
    mut config: ResMut<SimulationConfig>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    mut scenario_events: MessageWriter<crate::simulation::scenarios::LoadScenarioEvent>,
    mut builder_state: ResMut<crate::game::ui::PlanetBuilderState>,
    mut toast: ResMut<crate::game::ui::NotificationToast>,
    mut camera_query: Query<(&Transform, &mut PanOrbitCamera)>,
    mut selected_query: Query<
        (
            Entity,
            &mut Mass,
            &mut Radius,
            &mut Temperature,
            &mut SimPosition,
            &mut SimVelocity,
            &mut Composition,
            &mut CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&mut Transform>,
            Option<&mut IgnitionState>,
            Option<&mut BlackHoleStarState>,
        ),
        Without<PanOrbitCamera>,
    >,
) {
    let mut rng = rand::rng();
    let star_mass = disk_params.central_star_mass;

    // Hotkey [P]: Toggle Planet Builder Panel
    if keyboard.just_pressed(KeyCode::KeyP) {
        builder_state.is_open = !builder_state.is_open;
        toast.message = if builder_state.is_open {
            "🛠️ Planet Builder & Spawner Opened [P]".to_string()
        } else {
            "🛠️ Planet Builder Closed [P]".to_string()
        };
        toast.timer = 2.5;
    }

    // 0. Tab Key: Deterministic Numerical Cycling Through All Celestial Bodies & The Star
    // Sorted strictly from the Central Star (0) outward by orbital distance (1..N)
    if keyboard.just_pressed(KeyCode::Tab) {
        let mut stars: Vec<(Entity, f64)> = Vec::new();
        let mut planets: Vec<(Entity, f64)> = Vec::new();

        for (e, _, _, _, pos, _, _, body, ..) in selected_query.iter() {
            if body.body_type.is_star_or_remnant() {
                stars.push((e, pos.0.length()));
            } else {
                planets.push((e, pos.0.length()));
            }
        }

        // Sort stars and planets from innermost to outermost
        stars.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        planets.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut all_entities: Vec<Entity> = Vec::new();
        for (s_ent, _) in stars {
            all_entities.push(s_ent);
        }
        for (p_ent, _) in planets {
            all_entities.push(p_ent);
        }

        if !all_entities.is_empty() {
            let shift =
                keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
            let len = all_entities.len();
            let next_idx = if let Some(curr) = player_state.selected_entity {
                if let Some(curr_idx) = all_entities.iter().position(|&e| e == curr) {
                    if shift {
                        (curr_idx + len - 1) % len
                    } else {
                        (curr_idx + 1) % len
                    }
                } else {
                    0
                }
            } else {
                if shift {
                    len - 1
                } else {
                    0
                }
            };

            let target = all_entities[next_idx];
            player_state.selected_entity = Some(target);
            if let Ok((_, mut cam)) = camera_query.single_mut() {
                cam.target_entity = Some(target);
            }
        }
    }

    // 0A. Size Exaggeration Slider (Comma = decrease, Period = increase)
    if keyboard.just_pressed(KeyCode::Comma) {
        config.size_exaggeration = (config.size_exaggeration * 0.7).max(0.1);
        info!("🔬 Size Scale: {:.2}×", config.size_exaggeration);
    }
    if keyboard.just_pressed(KeyCode::Period) {
        config.size_exaggeration = (config.size_exaggeration * 1.4).min(20.0);
        info!("🔬 Size Scale: {:.2}×", config.size_exaggeration);
    }

    // 0B. Escape Key: Deselect Current Target
    if keyboard.just_pressed(KeyCode::Escape) {
        player_state.selected_entity = None;
    }

    // 0B. Key V: Cycle Diagnostic Overlay Modes (Realistic -> Spectral Composition -> Hill Spheres & Gaps)
    if keyboard.just_pressed(KeyCode::KeyV) {
        player_state.overlay_mode = player_state.overlay_mode.cycle();
    }

    // 1. Toggle Gravitational Tractor Tool (Key T)
    if keyboard.just_pressed(KeyCode::KeyT) {
        if player_state.active_tool == PlayerTool::GravitationalTractor {
            player_state.active_tool = PlayerTool::Inspect;
            player_state.tractor_position = None;
            player_state.tractor_mass = 0.0;
        } else {
            player_state.active_tool = PlayerTool::GravitationalTractor;
            if let Ok((cam_trans, _)) = camera_query.single() {
                // Place tractor 15 AU in front of camera
                let tractor_pt = cam_trans.translation + cam_trans.forward() * 15.0;
                player_state.tractor_position = Some(DVec3::new(
                    tractor_pt.x as f64,
                    tractor_pt.y as f64,
                    tractor_pt.z as f64,
                ));
                player_state.tractor_mass = EARTH_MASS_SOLAR * 5.0;
            }
        }
    }

    // 2. LIVE CELESTIAL BODY EDITOR (When a body is selected)
    if let Some(selected_ent) = player_state.selected_entity {
        if let Ok((
            entity,
            mut mass,
            mut radius,
            mut _temp,
            mut pos,
            mut vel,
            mut comp,
            mut body,
            mut diff_opt,
            mut trans_opt,
            mut ignition_opt,
            mut opt_quasi,
        )) = selected_query.get_mut(selected_ent)
        {
            // A. Increase Mass (Key U or Key + / =)
            if keyboard.just_pressed(KeyCode::KeyU)
                || keyboard.just_pressed(KeyCode::Equal)
                || keyboard.just_pressed(KeyCode::NumpadAdd)
            {
                mass.0 *= 1.25; // +25% mass
                if !body.body_type.is_star_or_remnant() {
                    let avg_density = comp.average_density();
                    radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.1);
                }

                if let Some(ref mut diff) = diff_opt {
                    diff.recalculate(mass.0, radius.0, &comp);
                }
                if let Some(ref mut trans) = trans_opt {
                    let scale = (radius.0 as f32 * 50.0).clamp(0.02, 1.5);
                    trans.scale = Vec3::splat(scale);
                }
            }

            // B. Decrease Mass (Key J or Key - / _)
            if keyboard.just_pressed(KeyCode::KeyJ)
                || keyboard.just_pressed(KeyCode::Minus)
                || keyboard.just_pressed(KeyCode::NumpadSubtract)
            {
                mass.0 = (mass.0 * 0.8).max(1e-7 * EARTH_MASS_SOLAR);
                if !body.body_type.is_star_or_remnant() {
                    let avg_density = comp.average_density();
                    radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.1);
                }

                if let Some(ref mut diff) = diff_opt {
                    diff.recalculate(mass.0, radius.0, &comp);
                }
                if let Some(ref mut trans) = trans_opt {
                    let scale = (radius.0 as f32 * 50.0).clamp(0.02, 1.5);
                    trans.scale = Vec3::splat(scale);
                }
            }

            // C. Expand Orbit (Key O)
            if keyboard.just_pressed(KeyCode::KeyO) && !body.body_type.is_star_or_remnant() {
                pos.0 *= 1.10;
                let r = pos.0.length().max(0.1);
                let v_circ = (G_ASTRO * star_mass / r).sqrt();
                let phi = pos.0.z.atan2(pos.0.x);
                vel.0 = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
            }

            // D. Contract Orbit (Key L)
            if keyboard.just_pressed(KeyCode::KeyL) && !body.body_type.is_star_or_remnant() {
                pos.0 = (pos.0 * 0.90).clamp_length_min(0.25);
                let r = pos.0.length().max(0.1);
                let v_circ = (G_ASTRO * star_mass / r).sqrt();
                let phi = pos.0.z.atan2(pos.0.x);
                vel.0 = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
            }

            // E. Cycle Composition (Key C)
            if keyboard.just_pressed(KeyCode::KeyC) {
                *comp = comp.cycle_next_composition();
                if !body.body_type.is_star_or_remnant() {
                    let avg_density = comp.average_density();
                    radius.0 = ((3.0 * mass.0 / avg_density) / (4.0 * PI))
                        .cbrt()
                        .max(EARTH_RADIUS_AU * 0.1);
                }
            }

            // F. Stellar Core Ignition (Key I on Star) or Prograde Delta-V Boost (Key I or B on Planets)
            if keyboard.just_pressed(KeyCode::KeyI) {
                if body.body_type.is_star_or_remnant() {
                    if let Some(ref mut ignition) = ignition_opt {
                        if !ignition.is_ignited {
                            ignition.core_temperature = 1.0e7; // Trigger instant fusion!
                        } else {
                            ignition.shockwave_radius = 1.6; // Trigger new coronal solar blast!
                        }
                    }
                } else {
                    let speed = vel.0.length();
                    if speed > 0.0 {
                        let dir = vel.0 / speed;
                        vel.0 += dir * (speed * 0.15);
                    }
                }
            }

            // F2. Prograde Delta-V Boost alternative (Key B on Planets)
            if keyboard.just_pressed(KeyCode::KeyB) && !body.body_type.is_star_or_remnant() {
                let speed = vel.0.length();
                if speed > 0.0 {
                    let dir = vel.0 / speed;
                    vel.0 += dir * (speed * 0.15);
                }
            }

            // G. Retrograde Delta-V Brake (Key K)
            if keyboard.just_pressed(KeyCode::KeyK) && !body.body_type.is_star_or_remnant() {
                let speed = vel.0.length();
                if speed > 0.0 {
                    let dir = vel.0 / speed;
                    vel.0 -= dir * (speed * 0.15);
                }
            }

            // H. Circularize / Fix Orbit (Key Z)
            if keyboard.just_pressed(KeyCode::KeyZ) && !body.body_type.is_star_or_remnant() {
                let r_cyl = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt().max(0.1);
                let v_circ = (G_ASTRO * star_mass / r_cyl).sqrt();
                let phi = pos.0.z.atan2(pos.0.x);
                vel.0 = DVec3::new(-v_circ * phi.sin(), 0.0, v_circ * phi.cos());
                pos.0.y = 0.0;
            }

            // I. Delete / Vaporize Selected Body (Key Delete or Backspace)
            if (keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace))
                && !body.body_type.is_star_or_remnant()
            {
                if let Ok(mut cmd) = commands.get_entity(entity) {
                    cmd.despawn();
                }
                player_state.selected_entity = None;
            }

            // J. Shatter / Form Planetary Rings (Key X)
            if keyboard.just_pressed(KeyCode::KeyX) && !body.body_type.is_star_or_remnant() {
                if let Ok(mut cmd) = commands.get_entity(entity) {
                    cmd.insert(PlanetaryRingSystem {
                        inner_radius_au: 0.0008,
                        outer_radius_au: 0.0028,
                        ring_mass_earth: 0.0002,
                        optical_depth: 0.88,
                        ice_fraction: 0.95,
                        silicate_fraction: 0.05,
                    });
                }
            }

            // K. Seed Photosynthetic Biosphere & Oceans (Key E)
            if keyboard.just_pressed(KeyCode::KeyE) && !body.body_type.is_star_or_remnant() {
                if let Ok(mut cmd) = commands.get_entity(entity) {
                    cmd.insert((
                        VolatileInventory {
                            delivered_water_m_earth: 0.002,
                            ocean_coverage_frac: 0.70,
                            atmospheric_pressure_bar: 1.0,
                            cometary_impact_count: 12,
                        },
                        BiosphereState {
                            habitability_score: 0.95,
                            biomass_coverage_frac: 0.65,
                            oxygen_fraction: 0.21,
                            emergence_year: Some(0.0),
                        },
                        PlanetaryClimate {
                            surface_temperature_k: 288.0,
                            equilibrium_temperature_k: 255.0,
                            greenhouse_delta_k: 33.0,
                            albedo: 0.30,
                            ice_coverage_frac: 0.10,
                            cloud_coverage_frac: 0.55,
                            climate_regime: ClimateRegime::TemperateHabitable,
                        },
                    ));
                    comp.ice_frac = 0.08;
                    comp.gas_frac = 0.02;
                }
            }

            // L. Stellar Evolution Metamorphosis (Key N on Star)
            if keyboard.just_pressed(KeyCode::KeyN) && body.body_type.is_star_or_remnant() {
                if let Some(ref mut ignition) = ignition_opt {
                    if !ignition.is_ignited {
                        ignition.core_temperature = 1.0e7;
                        ignition.is_ignited = true;
                        ignition.fusion_fraction = 1.0;
                        ignition.shockwave_radius = 1.6;
                        body.body_type = BodyType::MainSequenceStar;
                        body.name = "The Star (Main Sequence)".to_string();
                    }
                }
            }

            // M. JWST Little Red Dot / Black Hole Star Experiments (Keys X, B)
            if let Some(ref mut qs) = opt_quasi {
                if keyboard.just_pressed(KeyCode::KeyX) {
                    qs.toggle_super_eddington();
                    let mode = if qs.super_eddington_active {
                        "4.5x Eddington (Hyper-Accretion Active)"
                    } else {
                        "0.9x Eddington (Sub-Eddington Normal)"
                    };
                    toast.message = format!("⚡ Inflow Rate: {} on {}", mode, body.name);
                    toast.timer = 4.5;
                }
                if keyboard.just_pressed(KeyCode::KeyB) {
                    qs.trigger_blowout();
                    toast.message = "💥 COCOON BLOWOUT: Radiation pressure shedding hydrogen envelope into Quasar!".to_string();
                    toast.timer = 6.0;
                }
            }
        }
    }

    // 3. Mass Injection / Seed Planetesimal (Key M)
    if keyboard.just_pressed(KeyCode::KeyM) {
        let spawn_radius = rng.random_range(0.8..12.0);
        let phi = rng.random_range(0.0..2.0 * PI);
        let pos = DVec3::new(spawn_radius * phi.cos(), 0.0, spawn_radius * phi.sin());

        let v_k = (G_ASTRO * star_mass / spawn_radius).sqrt();
        let vel = DVec3::new(-v_k * phi.sin(), 0.0, v_k * phi.cos());

        let mass = EARTH_MASS_SOLAR * 0.20;
        let comp = if spawn_radius < 2.7 {
            Composition::rocky()
        } else {
            Composition::icy()
        };

        commands.spawn((
            CelestialBody {
                body_type: BodyType::Protoplanet,
                name: format!("Injected Embryo @ {:.1} AU", spawn_radius),
            },
            Mass(mass),
            SimPosition(pos),
            SimVelocity(vel),
            SimAcceleration::default(),
            Radius(EARTH_RADIUS_AU * 0.8),
            Temperature(280.0 * spawn_radius.powf(-0.5)),
            Luminosity(0.0),
            AngularMomentum(pos.cross(vel) * mass),
            comp,
        ));
    }

    // 4. Trigger Late Heavy Bombardment & Giant Planet Migration (Key G)
    if keyboard.just_pressed(KeyCode::KeyG) {
        lhb_state.is_active = true;
        lhb_state.manual_trigger_requested = true;
    }

    // 5. Exoplanet System Generator Scenario Hotkeys (F1 - F5)
    if keyboard.just_pressed(KeyCode::F1) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::SolarNebulaMmsn,
        ));
    } else if keyboard.just_pressed(KeyCode::F2) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::Trappist1System,
        ));
    } else if keyboard.just_pressed(KeyCode::F3) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::Kepler16Circumbinary,
        ));
    } else if keyboard.just_pressed(KeyCode::F4) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::HotJupiterMigration,
        ));
    } else if keyboard.just_pressed(KeyCode::F5) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::RoguePlanetFlyby,
        ));
    } else if keyboard.just_pressed(KeyCode::F6) {
        scenario_events.write(crate::simulation::scenarios::LoadScenarioEvent(
            crate::simulation::scenarios::ScenarioPreset::LittleRedDot,
        ));
    }

    // 6. GPU Compute Pipeline Toggle (F8)
    if keyboard.just_pressed(KeyCode::F8) {
        config.enable_gpu_compute = !config.enable_gpu_compute;
        if !config.enable_gpu_compute {
            config.gpu_compute_active = false;
        }
        let status = if config.enable_gpu_compute {
            "⚡ GPU COMPUTE: Enabled (WGPU Compute Dispatch @ 100,000 Particles)"
        } else {
            "🖥️ CPU FALLBACK: Active (Rayon Multithreading)"
        };
        toast.message = status.to_string();
        toast.timer = 4.0;
    }
}

/// System that handles direct 3D plane click-to-place spawning when builder click mode is active.
pub fn handle_planet_builder_click_spawn(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
    mut pan_orbit_query: Query<&mut PanOrbitCamera>,
    mut builder_state: ResMut<crate::game::ui::PlanetBuilderState>,
    disk_params: Res<DiskParameters>,
    mut player_state: ResMut<PlayerInteractionState>,
    mut toast: ResMut<crate::game::ui::NotificationToast>,
    ui_interaction_query: Query<&Interaction, With<Button>>,
) {
    if !builder_state.is_open || !builder_state.click_to_spawn_mode {
        return;
    }

    // Don't spawn if cursor is clicking/hovering any UI button
    let clicking_ui = ui_interaction_query
        .iter()
        .any(|i| *i == Interaction::Pressed || *i == Interaction::Hovered);
    if clicking_ui {
        return;
    }

    if mouse_buttons.just_pressed(MouseButton::Left) {
        let Ok(window) = window_query.single() else {
            return;
        };
        let Ok((camera, camera_transform)) = camera_query.single() else {
            return;
        };

        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
                // Intersect ray with the orbital disk plane Y = 0
                if ray.direction.y.abs() > 1e-5 {
                    let t = -ray.origin.y / ray.direction.y;
                    if t > 0.0 {
                        let hit = ray.origin + *ray.direction * t;
                        let spawn_coords = DVec3::new(hit.x as f64, 0.0, hit.z as f64);
                        crate::game::ui::spawn_custom_builder_world(
                            &mut commands,
                            &builder_state,
                            disk_params.central_star_mass,
                            Some(spawn_coords),
                            &mut player_state,
                            &mut pan_orbit_query,
                            &mut toast,
                        );
                        // Exit click-to-spawn mode after placement
                        builder_state.click_to_spawn_mode = false;
                    }
                }
            }
        }
    }
}
