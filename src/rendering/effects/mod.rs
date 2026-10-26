pub mod dark_matter;
pub mod habitable_zone;
pub mod impact_debris;
pub mod kilonova;
pub mod lagrange;
pub mod megastructures;
pub mod orbits;
pub mod planetary;
pub mod relativistic;
pub mod remnants;
pub mod roche_stream;
pub mod shockwaves;
pub mod supernova;
pub mod tde;
pub mod tools;
pub mod volcanism;

pub use dark_matter::draw_dark_matter_halos;
pub use habitable_zone::{
    compute_effective_luminosity, draw_circumstellar_habitable_zone_and_frost_lines,
    draw_habitable_zone_gizmos,
};
pub use impact_debris::*;
pub use lagrange::{
    compute_cr3bp_lagrange_points, draw_lagrange_points_gizmos,
    render_lagrange_points_and_hill_sphere, LagrangeSystemCoordinates,
};
pub use roche_stream::*;
pub use volcanism::*;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::EARTH_MASS_SOLAR;
use crate::utils::math::*;

pub use kilonova::{
    draw_kilonova_effects, update_kilonova_effects, KilonovaInstance, KilonovaPool,
};
pub use megastructures::{draw_dyson_swarms, update_megastructure_construction, DysonSwarm};
pub use orbits::{
    draw_ambient_and_trailing_orbit_ribbon, draw_conic_apsides_and_nodes,
    draw_hyperbolic_trajectory,
};
pub use planetary::{
    draw_body_selection_and_beacons, draw_cometary_escape_tails, draw_diagnostic_overlays,
    draw_distant_optical_glint, draw_hover_targeting_reticle, draw_internal_planetary_cutaway,
    draw_planetary_magnetospheres, draw_volcanic_and_cryo_plumes,
};
pub use relativistic::{
    draw_black_hole_spacetime_curvature, draw_ignition_shockwave, draw_kerr_ergosphere,
    draw_magnetar_reconnection_gizmos, draw_neutron_star_gizmos, draw_pulsar_lighthouse_gizmos,
    draw_quasar_relativistic_jets, draw_quasi_star_photosphere_and_magnetosphere,
    draw_standard_black_hole_jets, draw_stellar_evolution_nebula, draw_white_dwarf_gizmos,
};
pub use remnants::{
    draw_persistent_supernova_remnants, spawn_remnant_from_explosion, update_persistent_remnants,
    PersistentRemnantPool, PersistentSupernovaRemnant, RemnantFilament,
};
pub use shockwaves::{
    draw_au_guide_rings, draw_impact_shockwaves, draw_roche_debris_streamers,
    update_impact_shockwaves, update_roche_debris_streams,
};
pub use supernova::{
    draw_supernova_explosions, sync_supernova_visuals, update_supernova_explosions, EjectaLayer,
    SupernovaDebrisPool, SupernovaEjectaFragment, SupernovaExplosionInstance, SupernovaType,
    SupernovaVisualShell,
};
pub use tde::{draw_tidal_disruption_streams, update_tidal_disruption_streams};
pub use tools::{
    draw_bombardment_projectiles_gizmo, draw_planet_builder_preview, draw_slingshot_preview,
    draw_tractor_beam_gizmo, draw_trajectory_prediction_gizmos,
};

fn draw_star_effects(
    gizmos: &mut Gizmos,
    star_vec: Vec3,
    star_radius: &Radius,
    star_mass: &Mass,
    star_body: &CelestialBody,
    opt_ignition: Option<&IgnitionState>,
    opt_evo: Option<&StellarEvolutionState>,
    opt_quasi: Option<&BlackHoleStarState>,
    opt_kerr: Option<&crate::simulation::relativity::kerr::KerrBlackHole>,
    config: &SimulationConfig,
    overlay_mode: DiagnosticOverlayMode,
    elapsed: f32,
) {
    // When overlays are hidden or in GPU MagneticFields mode, skip star-related
    // line gizmos so the 3D visual field representations remain pristine.
    if overlay_mode == DiagnosticOverlayMode::Hidden
        || overlay_mode == DiagnosticOverlayMode::MagneticFields
    {
        return;
    }
    if let Some(evo) = opt_evo {
        draw_stellar_evolution_nebula(gizmos, star_vec, evo);
    }

    if star_body.body_type == BodyType::WhiteDwarf {
        draw_white_dwarf_gizmos(gizmos, star_vec, elapsed);
    } else if star_body.body_type == BodyType::Pulsar {
        draw_pulsar_lighthouse_gizmos(gizmos, star_vec, elapsed);
    } else if star_body.body_type == BodyType::Magnetar {
        draw_magnetar_reconnection_gizmos(gizmos, star_vec, elapsed);
    } else if matches!(
        star_body.body_type,
        BodyType::NeutronStar | BodyType::QuarkStar
    ) {
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
        let base_lens_r = if (opt_quasi.is_some() || star_body.body_type == BodyType::QuasiStar)
            && !is_blown_out
        {
            (current_visual_r * 1.50).max(3.6)
        } else {
            (current_visual_r * 1.50).max(0.12)
        };
        if let Some(kerr) = opt_kerr {
            draw_kerr_ergosphere(gizmos, star_vec, kerr, base_lens_r, elapsed);
        }
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
    cam_tf: Option<Transform>,
}

#[allow(
    clippy::too_many_arguments,
    reason = "Physical features require tail, volcano, and magnetic field components"
)]
fn draw_body_physical_features(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    vel: &SimVelocity,
    opt_tail: Option<&AtmosphericEscapeTail>,
    opt_volc: Option<&VolcanicState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_spin: Option<&SpinState>,
    opt_rad: Option<&Radius>,
    params: &BodyEffectParams,
    player_state: &PlayerInteractionState,
    config: &SimulationConfig,
    is_selected: bool,
) {
    let is_active_escape = opt_tail.is_some_and(|t| t.is_active && t.tail_length_au > 0.05);
    let should_draw_tail = if is_active_escape {
        match player_state.orbit_mode {
            OrbitVisualizationMode::Off => false,
            OrbitVisualizationMode::SelectedOnly => is_selected,
            OrbitVisualizationMode::All => true,
        }
    } else {
        false
    } && player_state.overlay_mode != DiagnosticOverlayMode::Hidden;

    if should_draw_tail {
        draw_cometary_escape_tails(
            gizmos,
            body_vec,
            params.star_vec,
            vel,
            opt_tail,
            opt_rad,
            config,
            params.elapsed,
        );
    }

    if let Some(volc) = opt_volc {
        if player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
            let visual_rad = opt_rad.map_or(0.05, |r| config.calc_visual_radius(r.0));
            draw_volcanic_and_cryo_plumes(
                gizmos,
                body_vec,
                visual_rad,
                volc,
                opt_spin,
                params.star_vec,
                params.cam_tf.map_or(Vec3::ZERO, |t| t.translation),
                params.elapsed,
            );
        }
    }

    if let Some(diff) = opt_diff {
        if player_state.overlay_mode != DiagnosticOverlayMode::Hidden
            && player_state.overlay_mode != DiagnosticOverlayMode::MagneticFields
        {
            draw_planetary_magnetospheres(gizmos, body_vec, params.star_vec, diff, opt_spin);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Body orbit and overlays require astrometric and render contextual parameters"
)]
fn draw_body_orbit_and_overlays(
    gizmos: &mut Gizmos,
    body_vec: Vec3,
    pos: &SimPosition,
    vel: &SimVelocity,
    mass: &Mass,
    comp: &Composition,
    body: &CelestialBody,
    opt_rad: Option<&Radius>,
    opt_satellite: Option<&SatelliteOf>,
    params: &BodyEffectParams,
    player_state: &PlayerInteractionState,
    config: &SimulationConfig,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
    is_selected: bool,
    is_planet: bool,
) {
    let r_orbit = pos.0.length() as f32;
    let should_draw_orbit = match player_state.orbit_mode {
        OrbitVisualizationMode::Off => false,
        OrbitVisualizationMode::SelectedOnly => is_selected,
        OrbitVisualizationMode::All => {
            is_selected
                || is_planet
                || opt_satellite.is_some()
                || (mass.0 / EARTH_MASS_SOLAR) >= 0.05
                || body.body_type == BodyType::Comet
                || body.body_type == BodyType::Asteroid
        }
    } && player_state.overlay_mode != DiagnosticOverlayMode::Hidden;

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

    if player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        if let Some(ctf) = params.cam_tf {
            let cam_dist = ctf.translation.distance(body_vec);
            let vis_r = opt_rad.map_or(0.002, |r| {
                crate::rendering::bodies::transforms::compute_realistic_visual_radius(
                    r.0,
                    body.body_type,
                    r_orbit,
                    0.4,
                    cam_dist,
                    config,
                )
            });
            let glint_col =
                determine_orbit_color(body.body_type, is_selected, opt_satellite.is_some(), 0.0);
            draw_distant_optical_glint(
                gizmos,
                body_vec,
                ctf.translation,
                *ctf.up(),
                *ctf.right(),
                vis_r,
                glint_col,
                body.body_type,
            );
        }
    }

    if is_selected && player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        draw_body_selection_and_beacons(
            gizmos,
            body_vec,
            body,
            opt_rad,
            config,
            params.cam_tf.map(|t| t.translation),
        );
    }
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
    opt_volc: Option<&VolcanicState>,
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

    draw_body_physical_features(
        gizmos,
        body_vec,
        vel,
        opt_tail,
        opt_volc,
        opt_diff,
        opt_spin,
        opt_rad,
        params,
        player_state,
        config,
        is_selected,
    );

    draw_body_orbit_and_overlays(
        gizmos,
        body_vec,
        pos,
        vel,
        mass,
        comp,
        body,
        opt_rad,
        opt_satellite,
        params,
        player_state,
        config,
        parent_query,
        is_selected,
        is_planet,
    );

    let is_hovered = player_state.hovered_entity == Some(entity);
    if is_hovered && !is_selected && player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        draw_hover_targeting_reticle(
            gizmos,
            body_vec,
            body,
            opt_rad,
            config,
            params.cam_tf.as_ref(),
            params.elapsed,
        );
    }

    let is_cutaway = player_state.is_cutaway_active()
        && (is_selected || (player_state.selected_entity.is_none() && is_hovered));
    if is_cutaway && player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        draw_internal_planetary_cutaway(
            gizmos,
            body_vec,
            body,
            opt_rad,
            opt_diff,
            opt_spin,
            config,
            params.cam_tf.as_ref(),
            params.elapsed,
        );
    }
}

fn draw_cataclysm_and_environment_effects(
    gizmos: &mut Gizmos,
    player_state: &PlayerInteractionState,
    shockwave_pool: &ImpactShockwavePool,
    debris_pool: &RocheDebrisPool,
    supernova_pool: Option<&SupernovaDebrisPool>,
    star_vec: Vec3,
) {
    if player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        draw_impact_shockwaves(gizmos, shockwave_pool);
        draw_roche_debris_streamers(gizmos, debris_pool);
    }
    if let Some(pool) = supernova_pool {
        draw_supernova_explosions(gizmos, pool);
    }
    // AU distance guides look like planetary orbits — hide them with orbit trails.
    if player_state.orbit_mode != OrbitVisualizationMode::Off
        && player_state.overlay_mode != DiagnosticOverlayMode::Hidden
    {
        draw_au_guide_rings(gizmos, star_vec);
    }
}

type CelestialBodiesQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static SimPosition,
        &'static SimVelocity,
        &'static Mass,
        &'static Composition,
        &'static CelestialBody,
        Option<&'static InternalDifferentiation>,
        Option<&'static SpinState>,
        Option<&'static Radius>,
        Option<&'static AtmosphericEscapeTail>,
        Option<&'static SatelliteOf>,
        Option<&'static VolcanicState>,
    ),
    Without<crate::simulation::probes::SpaceProbe>,
>;

fn draw_celestial_bodies_batch(
    gizmos: &mut Gizmos,
    bodies_query: &CelestialBodiesQuery<'_, '_>,
    params: &BodyEffectParams,
    player_state: &PlayerInteractionState,
    config: &SimulationConfig,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
) {
    for (
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
        opt_volc,
    ) in bodies_query.iter()
    {
        draw_single_body_gizmos(
            gizmos,
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
            opt_volc,
            params,
            player_state,
            config,
            parent_query,
        );
    }
}

type CentralStarQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static SimPosition,
        &'static Mass,
        &'static Radius,
        Option<&'static IgnitionState>,
        &'static CelestialBody,
        Option<&'static StellarEvolutionState>,
        Option<&'static ElectromagneticFieldState>,
        Option<&'static BlackHoleStarState>,
        Option<&'static crate::simulation::relativity::kerr::KerrBlackHole>,
    ),
    With<CentralStar>,
>;

#[allow(clippy::type_complexity, reason = "Orbital Effects Gizmos")]
pub fn draw_orbital_effects_and_gizmos(
    mut gizmos: Gizmos,
    config: Res<SimulationConfig>,
    player_state: Res<PlayerInteractionState>,
    shockwave_pool: Res<ImpactShockwavePool>,
    debris_pool: Res<RocheDebrisPool>,
    supernova_pool: Option<Res<SupernovaDebrisPool>>,
    sim_time: Option<Res<SimTime>>,
    camera_query: Query<&Transform, With<Camera3d>>,
    star_query: CentralStarQuery<'_, '_>,
    bodies_query: CelestialBodiesQuery<'_, '_>,
    parent_query: Query<(&SimPosition, &SimVelocity, &Mass)>,
    opt_builder: Option<Res<crate::game::ui::PlanetBuilderState>>,
    opt_slingshot: Option<Res<crate::simulation::resources::SlingshotState>>,
    opt_predictor: Option<Res<crate::simulation::predictor::TrajectoryPredictorState>>,
) {
    let has_supernova = supernova_pool
        .as_deref()
        .is_some_and(|p| !p.explosions.is_empty());
    if should_skip_orbital_gizmos(
        &player_state,
        &shockwave_pool,
        &debris_pool,
        has_supernova,
        opt_builder.as_deref(),
        opt_slingshot.as_deref(),
        opt_predictor.as_deref(),
    ) {
        return;
    }
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

    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    let cam_tf = camera_query.iter().next().copied();

    if let Some((_, s_mass, s_rad, opt_ign, s_body, opt_evo, _, opt_quasi, opt_kerr)) = opt_star {
        draw_star_effects(
            &mut gizmos,
            star_vec,
            s_rad,
            s_mass,
            s_body,
            opt_ign,
            opt_evo,
            opt_quasi,
            opt_kerr,
            &config,
            player_state.overlay_mode,
            elapsed,
        );
    }

    draw_cataclysm_and_environment_effects(
        &mut gizmos,
        &player_state,
        &shockwave_pool,
        &debris_pool,
        supernova_pool.as_deref(),
        star_vec,
    );

    let params = BodyEffectParams {
        star_vec,
        star_pos_dvec,
        star_mass_val,
        elapsed,
        cam_tf,
    };

    draw_celestial_bodies_batch(
        &mut gizmos,
        &bodies_query,
        &params,
        &player_state,
        &config,
        &parent_query,
    );

    draw_interactive_tool_gizmos(
        &mut gizmos,
        &player_state,
        opt_builder.as_deref(),
        opt_slingshot.as_deref(),
        opt_predictor.as_deref(),
        star_vec,
        star_mass_val,
        elapsed,
    );
}

fn should_skip_orbital_gizmos(
    player_state: &PlayerInteractionState,
    shockwave_pool: &ImpactShockwavePool,
    debris_pool: &RocheDebrisPool,
    has_supernova: bool,
    opt_builder: Option<&crate::game::ui::PlanetBuilderState>,
    opt_slingshot: Option<&crate::simulation::resources::SlingshotState>,
    opt_predictor: Option<&crate::simulation::predictor::TrajectoryPredictorState>,
) -> bool {
    if player_state.overlay_mode == DiagnosticOverlayMode::Hidden && !has_supernova {
        return true;
    }

    let slingshot_dragging = opt_slingshot.is_some_and(|s| s.is_active && s.drag_origin.is_some());
    let has_predictor = opt_predictor.is_some_and(|p| {
        p.is_enabled && (!p.trajectory_points.is_empty() || p.active_encounter.is_some())
    });

    player_state.orbit_mode == OrbitVisualizationMode::Off
        && shockwave_pool.shockwaves.is_empty()
        && debris_pool.streams.is_empty()
        && !has_supernova
        && opt_builder.is_none()
        && !slingshot_dragging
        && !has_predictor
}

fn draw_interactive_tool_gizmos(
    gizmos: &mut Gizmos,
    player_state: &PlayerInteractionState,
    opt_builder: Option<&crate::game::ui::PlanetBuilderState>,
    opt_slingshot: Option<&crate::simulation::resources::SlingshotState>,
    opt_predictor: Option<&crate::simulation::predictor::TrajectoryPredictorState>,
    star_vec: Vec3,
    star_mass_val: f64,
    elapsed: f32,
) {
    if player_state.overlay_mode == DiagnosticOverlayMode::Hidden {
        return;
    }

    if player_state.active_tool == PlayerTool::GravitationalTractor {
        draw_tractor_beam_gizmo(gizmos, player_state);
    }

    if let Some(builder) = opt_builder {
        draw_planet_builder_preview(gizmos, builder, star_vec, star_mass_val, elapsed);
    }

    if let Some(slingshot) = opt_slingshot {
        draw_slingshot_preview(gizmos, slingshot, star_vec, star_mass_val, elapsed);
    }

    if let Some(predictor) = opt_predictor {
        draw_trajectory_prediction_gizmos(gizmos, predictor, star_vec, elapsed);
    }
}

pub fn draw_bombardment_gizmos(
    mut gizmos: Gizmos,
    projectiles_query: Query<(
        &SimPosition,
        &crate::simulation::terraforming::BombardmentProjectile,
    )>,
    targets_query: Query<(&SimPosition, &Radius)>,
    sim_time: Option<Res<SimTime>>,
    player_state: Res<PlayerInteractionState>,
) {
    if player_state.overlay_mode == DiagnosticOverlayMode::Hidden {
        return;
    }
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    draw_bombardment_projectiles_gizmo(&mut gizmos, &projectiles_query, &targets_query, elapsed);
}

pub fn draw_persistent_remnants_gizmos(
    mut gizmos: Gizmos,
    remnant_pool: Option<Res<PersistentRemnantPool>>,
    sim_time: Option<Res<SimTime>>,
) {
    let Some(pool) = remnant_pool else {
        return;
    };
    if pool.remnants.is_empty() {
        return;
    }
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    draw_persistent_supernova_remnants(&mut gizmos, &pool, visual_time);
}

pub fn draw_kilonova_gizmos(mut gizmos: Gizmos, kilonova_pool: Option<Res<KilonovaPool>>) {
    if let Some(pool) = kilonova_pool.filter(|p| !p.instances.is_empty()) {
        draw_kilonova_effects(&mut gizmos, &pool);
    }
}

pub fn draw_tde_gizmos(mut gizmos: Gizmos, tde_pool: Option<Res<TidalDisruptionPool>>) {
    let Some(pool) = tde_pool else {
        return;
    };
    if pool.streams.is_empty() {
        return;
    }
    draw_tidal_disruption_streams(&mut gizmos, &pool);
}
