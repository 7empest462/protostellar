//! Visual effects, orbit path gizmos, impact shockwaves, and diagnostic overlays.

pub mod orbits;
pub mod planetary;
pub mod relativistic;
pub mod shockwaves;
pub mod tools;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::EARTH_MASS_SOLAR;
use crate::utils::math::*;

pub use orbits::{
    draw_ambient_and_trailing_orbit_ribbon, draw_conic_apsides_and_nodes,
    draw_hyperbolic_trajectory,
};
pub use planetary::{
    draw_body_selection_and_beacons, draw_cometary_escape_tails, draw_diagnostic_overlays,
    draw_planetary_magnetospheres,
};
pub use relativistic::{
    draw_black_hole_spacetime_curvature, draw_ignition_shockwave,
    draw_magnetar_reconnection_gizmos, draw_neutron_star_gizmos, draw_pulsar_lighthouse_gizmos,
    draw_quasar_relativistic_jets, draw_quasi_star_photosphere_and_magnetosphere,
    draw_standard_black_hole_jets, draw_stellar_evolution_nebula, draw_white_dwarf_gizmos,
};
pub use shockwaves::{
    draw_au_guide_rings, draw_impact_shockwaves, draw_roche_debris_streamers,
    update_impact_shockwaves, update_roche_debris_streams,
};
pub use tools::{draw_planet_builder_preview, draw_tractor_beam_gizmo};

fn draw_star_effects(
    gizmos: &mut Gizmos,
    star_vec: Vec3,
    star_radius: &Radius,
    star_mass: &Mass,
    star_body: &CelestialBody,
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
    opt_quasi: Option<&BlackHoleStarState>,
    config: &SimulationConfig,
    elapsed: f32,
) {
    if let Some(evo) = opt_evo {
        draw_stellar_evolution_nebula(gizmos, star_vec, evo);
    }

    if star_body.body_type == BodyType::WhiteDwarf {
        draw_white_dwarf_gizmos(gizmos, star_vec, elapsed);
    } else if star_body.body_type == BodyType::Pulsar {
        draw_pulsar_lighthouse_gizmos(gizmos, star_vec, elapsed);
    } else if star_body.body_type == BodyType::Magnetar {
        draw_magnetar_reconnection_gizmos(gizmos, star_vec, elapsed);
    } else if star_body.body_type == BodyType::NeutronStar {
        draw_neutron_star_gizmos(gizmos, star_vec, elapsed);
    } else if opt_quasi.is_some()
        || star_body.body_type == BodyType::QuasiStar
        || star_body.name.contains("Quasar")
        || (star_body.body_type == BodyType::BlackHole && star_mass.0 > 500.0)
    {
        let is_blown_out =
            opt_quasi.is_some_and(|qs| qs.is_blown_out) || star_body.name.contains("Quasar");
        let blowout_p = opt_quasi.map_or(if is_blown_out { 1.0 } else { 0.0 }, |qs| {
            qs.blowout_progress
        });
        let light_dist = opt_quasi.map_or(0.0, |qs| qs.jet_travel_distance_au as f32);

        draw_quasar_relativistic_jets(
            gizmos,
            star_vec,
            star_radius.0,
            star_body.body_type,
            config,
            is_blown_out,
            blowout_p,
            light_dist,
            elapsed,
        );

        if !is_blown_out && star_body.body_type == BodyType::QuasiStar {
            draw_quasi_star_photosphere_and_magnetosphere(gizmos, star_vec, elapsed);
        }

        let current_visual_r =
            config.calc_visual_radius_for_type(star_radius.0, star_body.body_type);
        let base_lens_r = (current_visual_r * 1.50).max(3.6);
        draw_black_hole_spacetime_curvature(gizmos, star_vec, base_lens_r, is_blown_out, elapsed);
    } else if star_body.body_type == BodyType::BlackHole {
        draw_standard_black_hole_jets(gizmos, star_vec);
    }

    if let Some(ignition) = opt_ignition {
        draw_ignition_shockwave(gizmos, star_vec, ignition);
    }
}

fn determine_orbit_color(
    body_type: BodyType,
    is_selected: bool,
    is_satellite: bool,
    eccentricity: f64,
) -> Color {
    if is_selected {
        Color::srgba(0.15, 0.95, 1.0, 1.0)
    } else if is_satellite {
        Color::srgba(0.80, 0.85, 1.0, 0.85)
    } else if eccentricity >= 1.0 {
        Color::srgba(1.0, 0.25, 0.45, 0.95)
    } else {
        match body_type {
            BodyType::TerrestrialPlanet => Color::srgba(0.25, 1.0, 0.55, 0.90),
            BodyType::SuperEarth => Color::srgba(0.15, 0.90, 0.85, 0.90),
            BodyType::GasGiant => Color::srgba(1.0, 0.75, 0.20, 0.90),
            BodyType::IceGiant => Color::srgba(0.30, 0.80, 1.0, 0.90),
            BodyType::Protoplanet => Color::srgba(1.0, 0.55, 0.20, 0.85),
            BodyType::Planetesimal | BodyType::Asteroid => Color::srgba(0.70, 0.75, 0.85, 0.55),
            BodyType::Comet => Color::srgba(0.45, 1.0, 0.95, 0.90),
            _ => Color::srgba(0.75, 0.75, 0.85, 0.65),
        }
    }
}

fn draw_body_orbit(
    gizmos: &mut Gizmos,
    pos: &SimPosition,
    vel: &SimVelocity,
    mass: &Mass,
    body_type: BodyType,
    is_selected: bool,
    opt_satellite: Option<&SatelliteOf>,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
    star_pos_dvec: DVec3,
    star_mass_val: f64,
    star_vec: Vec3,
) {
    let (rel_pos, rel_vel, primary_mass, anchor_vec, is_satellite) =
        if let Some(sat) = opt_satellite {
            if let Ok((p_pos, p_vel, p_mass)) = parent_query.get(sat.parent) {
                (
                    pos.0 - p_pos.0,
                    vel.0 - p_vel.0,
                    p_mass.0,
                    Vec3::new(p_pos.x as f32, p_pos.y as f32, p_pos.z as f32),
                    true,
                )
            } else {
                (pos.0 - star_pos_dvec, vel.0, star_mass_val, star_vec, false)
            }
        } else {
            (pos.0 - star_pos_dvec, vel.0, star_mass_val, star_vec, false)
        };

    if let Some(elements) =
        state_vectors_to_orbital_elements(rel_pos, rel_vel, primary_mass, mass.0)
    {
        let base_color =
            determine_orbit_color(body_type, is_selected, is_satellite, elements.eccentricity);
        let base_rgba = base_color.to_srgba();

        if elements.eccentricity < 1.0 && elements.semi_major_axis > 0.0 {
            draw_ambient_and_trailing_orbit_ribbon(
                gizmos,
                &elements,
                anchor_vec,
                base_rgba,
                is_selected,
            );

            if is_selected || elements.eccentricity >= 0.03 {
                draw_conic_apsides_and_nodes(gizmos, &elements, anchor_vec);
            }
        } else if elements.eccentricity >= 1.0 {
            draw_hyperbolic_trajectory(gizmos, &elements, anchor_vec, base_rgba);
        }
    }
}

struct BodyEffectParams {
    star_vec: Vec3,
    star_pos_dvec: DVec3,
    star_mass_val: f64,
    elapsed: f32,
    cam_pos: Option<Vec3>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Per-body rendering gizmo requires multiple body components and contextual parameters"
)]
fn draw_single_body_gizmos(
    gizmos: &mut Gizmos,
    entity: Entity,
    pos: &SimPosition,
    vel: &SimVelocity,
    mass: &Mass,
    comp: &Composition,
    body: &CelestialBody,
    opt_diff: Option<&InternalDifferentiation>,
    opt_spin: Option<&SpinState>,
    opt_rad: Option<&Radius>,
    opt_tail: Option<&AtmosphericEscapeTail>,
    opt_satellite: Option<&SatelliteOf>,
    params: &BodyEffectParams,
    player_state: &PlayerInteractionState,
    config: &SimulationConfig,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
) {
    let is_selected = player_state.selected_entity == Some(entity);
    let is_planet = matches!(
        body.body_type,
        BodyType::Protoplanet
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::GasGiant
            | BodyType::IceGiant
    );
    let body_vec = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let r_orbit = pos.0.length() as f32;

    let should_draw_tail = match player_state.orbit_mode {
        OrbitVisualizationMode::Off => false,
        // SelectedOnly: only the selected body — no comet/tail exceptions.
        OrbitVisualizationMode::SelectedOnly => is_selected,
        OrbitVisualizationMode::All => true,
    };

    if should_draw_tail {
        draw_cometary_escape_tails(
            gizmos,
            body_vec,
            params.star_vec,
            vel,
            comp,
            opt_tail,
            opt_rad,
            config,
            params.elapsed,
        );
    }

    if let Some(diff) = opt_diff {
        draw_planetary_magnetospheres(gizmos, body_vec, params.star_vec, diff, opt_spin);
    }

    let should_draw_orbit = match player_state.orbit_mode {
        OrbitVisualizationMode::Off => false,
        OrbitVisualizationMode::SelectedOnly => is_selected,
        OrbitVisualizationMode::All => {
            is_selected
                || is_planet
                || opt_satellite.is_some()
                || (mass.0 / EARTH_MASS_SOLAR) >= 0.05
                || body.body_type == BodyType::Comet
        }
    };

    if should_draw_orbit {
        draw_body_orbit(
            gizmos,
            pos,
            vel,
            mass,
            body.body_type,
            is_selected,
            opt_satellite,
            parent_query,
            params.star_pos_dvec,
            params.star_mass_val,
            params.star_vec,
        );
    }

    draw_diagnostic_overlays(
        gizmos,
        body_vec,
        params.star_vec,
        r_orbit,
        mass,
        comp,
        body,
        params.star_mass_val,
        player_state.overlay_mode,
    );

    if is_selected {
        draw_body_selection_and_beacons(gizmos, body_vec, body, opt_rad, config, params.cam_pos);
    }
}

pub fn draw_orbital_effects_and_gizmos(
    mut gizmos: Gizmos,
    config: Res<SimulationConfig>,
    player_state: Res<PlayerInteractionState>,
    shockwave_pool: Res<ImpactShockwavePool>,
    debris_pool: Res<RocheDebrisPool>,
    time: Res<Time>,
    camera_query: Query<&Transform, With<Camera3d>>,
    star_query: Query<
        (
            &SimPosition,
            &Mass,
            &Radius,
            Option<&IgnitionState>,
            &CelestialBody,
            Option<&StellarEvolutionState>,
            Option<&ElectromagneticFieldState>,
            Option<&BlackHoleStarState>,
        ),
        With<CentralStar>,
    >,
    bodies_query: Query<(
        Entity,
        &SimPosition,
        &SimVelocity,
        &Mass,
        &Composition,
        &CelestialBody,
        Option<&InternalDifferentiation>,
        Option<&SpinState>,
        Option<&Radius>,
        Option<&AtmosphericEscapeTail>,
        Option<&SatelliteOf>,
    )>,
    parent_query: Query<(&SimPosition, &SimVelocity, &Mass)>,
    opt_builder: Option<Res<crate::game::ui::PlanetBuilderState>>,
) {
    let opt_star = star_query.iter().next();
    let (star_pos_dvec, star_mass_val, star_vec) = if let Some((s_pos, s_mass, ..)) = opt_star {
        (
            s_pos.0,
            s_mass.0,
            Vec3::new(s_pos.x as f32, s_pos.y as f32, s_pos.z as f32),
        )
    } else {
        (DVec3::ZERO, 1.0, Vec3::ZERO)
    };

    let elapsed = time.elapsed_secs();
    let cam_pos = camera_query.iter().next().map(|tf| tf.translation);

    if let Some((_, s_mass, s_rad, opt_ign, s_body, opt_evo, _, opt_quasi)) = opt_star {
        draw_star_effects(
            &mut gizmos,
            star_vec,
            s_rad,
            s_mass,
            s_body,
            opt_ign,
            opt_evo,
            opt_quasi,
            &config,
            elapsed,
        );
    }

    draw_impact_shockwaves(&mut gizmos, &shockwave_pool);
    draw_roche_debris_streamers(&mut gizmos, &debris_pool);
    // AU distance guides look like planetary orbits — hide them with orbit trails.
    if player_state.orbit_mode != OrbitVisualizationMode::Off {
        draw_au_guide_rings(&mut gizmos, star_vec);
    }

    let params = BodyEffectParams {
        star_vec,
        star_pos_dvec,
        star_mass_val,
        elapsed,
        cam_pos,
    };

    for (entity, pos, vel, mass, comp, body, opt_diff, opt_spin, opt_rad, opt_tail, opt_sat) in
        bodies_query.iter()
    {
        draw_single_body_gizmos(
            &mut gizmos,
            entity,
            pos,
            vel,
            mass,
            comp,
            body,
            opt_diff,
            opt_spin,
            opt_rad,
            opt_tail,
            opt_sat,
            &params,
            &player_state,
            &config,
            &parent_query,
        );
    }

    if player_state.active_tool == PlayerTool::GravitationalTractor {
        draw_tractor_beam_gizmo(&mut gizmos, &player_state);
    }

    if let Some(builder) = opt_builder {
        draw_planet_builder_preview(&mut gizmos, &builder, star_vec, star_mass_val, elapsed);
    }
}
