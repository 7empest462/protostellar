pub mod civilizations;
pub mod dark_matter;
pub mod habitable_zone;
pub mod impact_debris;
pub mod kilonova;
pub mod lagrange;
pub mod megastructures;
pub mod orbits;
pub mod planetary;
pub mod quasar_blowout;
pub mod relativistic;
pub mod remnants;
pub mod roche_stream;
pub mod shockwaves;
pub mod supernova;
pub mod tde;
pub mod tech_materials;
pub mod tech_meshes;
pub mod tools;
pub mod volcanism;

pub use civilizations::*;
pub use dark_matter::draw_dark_matter_halos;
pub use habitable_zone::*;
pub use impact_debris::*;
pub use kilonova::*;
pub use lagrange::*;
pub use megastructures::*;
pub use orbits::*;
pub use planetary::*;
pub use quasar_blowout::*;
pub use relativistic::*;
pub use remnants::*;
pub use roche_stream::*;
pub use shockwaves::*;
pub use supernova::*;
pub use tde::*;
pub use tech_materials::*;
pub use tools::*;
pub use volcanism::*;

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::EARTH_MASS_SOLAR;
use crate::utils::math::*;

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
    // When overlays are hidden, realistic, or in GPU MagneticFields mode, skip star-related
    // line gizmos so the 3D visual field representations remain pristine.
    if overlay_mode == DiagnosticOverlayMode::Hidden
        || overlay_mode == DiagnosticOverlayMode::MagneticFields
        || overlay_mode == DiagnosticOverlayMode::Realistic
    {
        if let Some(ignition) = opt_ignition {
            draw_ignition_shockwave(gizmos, star_vec, ignition);
        }
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

#[allow(
    clippy::too_many_arguments,
    reason = "Orbit rendering requires full state vectors, primary reference, and camera context"
)]
fn draw_body_orbit(
    gizmos: &mut Gizmos,
    pos: &SimPosition,
    vel: &SimVelocity,
    mass: &Mass,
    body_type: BodyType,
    is_selected: bool,
    is_hovered: bool,
    visual_rad: f32,
    params: &BodyEffectParams,
    opt_satellite: Option<&SatelliteOf>,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
) {
    let cam_pos = params.cam_tf.map(|t| t.translation);
    let primary_occluder = Some((params.star_vec, params.star_vis_r));
    let (rel_pos, rel_vel, primary_mass, anchor_rel_au, is_satellite) =
        if let Some(sat) = opt_satellite {
            if let Ok((p_pos, p_vel, p_mass)) = parent_query.get(sat.parent) {
                (
                    pos.0 - p_pos.0,
                    vel.0 - p_vel.0,
                    p_mass.0,
                    p_pos.0 - params.origin.0,
                    true,
                )
            } else {
                (
                    pos.0 - params.star_pos_dvec,
                    vel.0 - params.star_vel_dvec,
                    params.star_mass_val,
                    params.star_pos_dvec - params.origin.0,
                    false,
                )
            }
        } else {
            (
                pos.0 - params.star_pos_dvec,
                vel.0 - params.star_vel_dvec,
                params.star_mass_val,
                params.star_pos_dvec - params.origin.0,
                false,
            )
        };

    if let Some(elements) =
        state_vectors_to_orbital_elements(rel_pos, rel_vel, primary_mass, mass.0)
    {
        let base_color = orbits::determine_orbit_color(
            body_type,
            is_selected,
            is_satellite,
            elements.eccentricity,
        );
        let base_rgba = base_color.to_srgba();
        let minor_body_lod = matches!(
            body_type,
            BodyType::Asteroid | BodyType::Planetesimal | BodyType::DustGrain | BodyType::Comet
        ) && !is_selected
            && !is_hovered;

        if elements.eccentricity < 1.0 && elements.semi_major_axis > 0.0 {
            draw_ambient_and_trailing_orbit_ribbon(
                gizmos,
                &elements,
                anchor_rel_au,
                OrbitRenderStyle {
                    base_rgba,
                    is_selected,
                    visual_rad,
                    cam_pos,
                    minor_body_lod,
                    primary_occluder,
                },
            );

            if !minor_body_lod && (is_selected || elements.eccentricity >= 0.03) {
                draw_conic_apsides_and_nodes(
                    gizmos,
                    &elements,
                    anchor_rel_au,
                    cam_pos,
                    primary_occluder,
                );
            }
        } else if elements.eccentricity >= 1.0 {
            draw_hyperbolic_trajectory(
                gizmos,
                &elements,
                anchor_rel_au,
                base_rgba,
                cam_pos,
                primary_occluder,
            );
        }
    }
}

struct BodyEffectParams {
    origin: crate::rendering::origin::RenderOrigin,
    star_vec: Vec3,
    star_pos_dvec: DVec3,
    star_vel_dvec: DVec3,
    star_mass_val: f64,
    star_vis_r: f32,
    selected_occluder: Option<(Vec3, f32)>,
    elapsed: f32,
    elapsed_years: f64,
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
    visual_rad: f32,
    opt_tail: Option<&AtmosphericEscapeTail>,
    opt_volc: Option<&VolcanicState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_spin: Option<&SpinState>,
    params: &BodyEffectParams,
    player_state: &PlayerInteractionState,
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
            visual_rad,
            params.elapsed,
        );
    }

    if let Some(volc) = opt_volc
        && player_state.overlay_mode != DiagnosticOverlayMode::Hidden
    {
        draw_volcanic_and_cryo_plumes(
            gizmos,
            body_vec,
            visual_rad,
            volc,
            opt_spin,
            params.star_vec,
            params.cam_tf.map_or(Vec3::ZERO, |t| t.translation),
            params.elapsed,
            params.elapsed_years,
        );
    }

    if let Some(diff) = opt_diff
        && !matches!(
            player_state.overlay_mode,
            DiagnosticOverlayMode::Realistic
                | DiagnosticOverlayMode::Hidden
                | DiagnosticOverlayMode::MagneticFields
        )
    {
        draw_planetary_magnetospheres(
            gizmos,
            body_vec,
            params.star_vec,
            visual_rad,
            diff,
            opt_spin,
            params.cam_tf.map(|t| t.translation),
        );
    }
}

/// Computes satellite glint attenuation `[0.0, 1.0]` so moons unresolved from their parent
/// planet at solar-system / galactic distances do not stack diffraction spikes on the parent's pixel.
#[inline]
fn compute_satellite_glint_resolution_fade(
    body_vec: Vec3,
    cam_dist: f32,
    opt_satellite: Option<&SatelliteOf>,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
    origin: crate::rendering::origin::RenderOrigin,
) -> f32 {
    let Some(sat) = opt_satellite else {
        return 1.0;
    };
    let Ok((p_pos, _, _)) = parent_query.get(sat.parent) else {
        return 1.0;
    };
    let parent_vec = origin.to_render(p_pos.0);
    let angular_sep = parent_vec.distance(body_vec) / cam_dist.max(0.001);
    let t = ((angular_sep - 0.0035) / 0.0085).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Clone, Copy)]
struct BodyEffectFlags {
    is_blown_out_quasi: bool,
    is_selected: bool,
    is_hovered: bool,
    is_planet: bool,
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
    visual_rad: f32,
    temp_k: f64,
    opt_satellite: Option<&SatelliteOf>,
    params: &BodyEffectParams,
    player_state: &PlayerInteractionState,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
    flags: BodyEffectFlags,
) {
    let r_orbit = pos.0.length() as f32;
    let should_draw_orbit = match player_state.orbit_mode {
        OrbitVisualizationMode::Off => false,
        OrbitVisualizationMode::SelectedOnly => flags.is_selected,
        OrbitVisualizationMode::All => {
            flags.is_selected
                || flags.is_planet
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
            flags.is_selected,
            flags.is_hovered,
            visual_rad,
            params,
            opt_satellite,
            parent_query,
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
        params.cam_tf.map(|t| t.translation),
        params.star_vis_r,
    );

    if player_state.overlay_mode != DiagnosticOverlayMode::Hidden
        && crate::rendering::bodies::palettes::is_optically_luminous_or_reflective_glint_body(
            body.body_type,
            flags.is_blown_out_quasi,
        )
        && let Some(ctf) = params.cam_tf
    {
        let cam_dist = ctf.translation.distance(body_vec);
        let sat_fade = compute_satellite_glint_resolution_fade(
            body_vec,
            cam_dist,
            opt_satellite,
            parent_query,
            params.origin,
        );
        if sat_fade > 0.01 {
            let (photo_col, weight) =
                crate::rendering::bodies::palettes::compute_optical_glint_photometry(
                    body, mass.0, temp_k, comp,
                );
            let c = photo_col.to_srgba();
            let glint_col = Color::srgba(c.red, c.green, c.blue, weight * sat_fade);
            draw_distant_optical_glint(
                gizmos,
                body_vec,
                ctf.translation,
                *ctf.up(),
                *ctf.right(),
                visual_rad,
                glint_col,
                body.body_type,
            );
        }
    }

    if flags.is_selected && player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        draw_body_selection_and_beacons(
            gizmos,
            body_vec,
            body,
            visual_rad,
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
    opt_tf: Option<&Transform>,
    opt_temp: Option<&Temperature>,
    opt_bhs: Option<&BlackHoleStarState>,
    params: &BodyEffectParams,
    player_state: &PlayerInteractionState,
    config: &SimulationConfig,
    parent_query: &Query<(&SimPosition, &SimVelocity, &Mass)>,
) {
    let is_selected = player_state.selected_entity == Some(entity);
    let is_hovered = player_state.hovered_entity == Some(entity);
    let is_minor_body = matches!(
        body.body_type,
        BodyType::Asteroid | BodyType::Planetesimal | BodyType::DustGrain | BodyType::Comet
    );

    if is_minor_body
        && !is_selected
        && !is_hovered
        && player_state.orbit_mode == OrbitVisualizationMode::Off
        && player_state.overlay_mode == DiagnosticOverlayMode::Realistic
        && opt_volc.is_none()
    {
        return;
    }

    let is_planet = matches!(
        body.body_type,
        BodyType::Protoplanet
            | BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::GasGiant
            | BodyType::IceGiant
    );
    let body_vec = params.origin.to_render(pos.0);
    let r_orb = (body_vec - params.star_vec).length();
    let cam_dist = params
        .cam_tf
        .map_or(25.0, |t| t.translation.distance(body_vec));
    let phys_rad = opt_rad.map_or(0.00005, |r| r.0);
    let visual_rad = crate::rendering::bodies::transforms::resolve_synced_visual_radius(
        opt_tf,
        phys_rad,
        body.body_type,
        r_orb,
        cam_dist,
        config,
    );
    let temp_k = opt_temp.map_or_else(
        || {
            if body.body_type.is_star_or_remnant() {
                5778.0
            } else {
                250.0
            }
        },
        |t| t.0,
    );
    let is_blown_out_quasi = opt_bhs.is_some_and(|b| b.is_blown_out);

    draw_body_physical_features(
        gizmos,
        body_vec,
        vel,
        visual_rad,
        opt_tail,
        opt_volc,
        opt_diff,
        opt_spin,
        params,
        player_state,
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
        visual_rad,
        temp_k,
        opt_satellite,
        params,
        player_state,
        parent_query,
        BodyEffectFlags {
            is_blown_out_quasi,
            is_selected,
            is_hovered,
            is_planet,
        },
    );

    if is_hovered && !is_selected && player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        draw_hover_targeting_reticle(
            gizmos,
            body_vec,
            body,
            visual_rad,
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
            visual_rad,
            opt_diff,
            opt_spin,
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
    params: &BodyEffectParams,
) {
    if player_state.overlay_mode != DiagnosticOverlayMode::Hidden {
        draw_impact_shockwaves(gizmos, shockwave_pool, params.origin);
        draw_roche_debris_streamers(gizmos, debris_pool, params.origin);
    }
    if let Some(pool) = supernova_pool {
        draw_supernova_explosions(gizmos, pool, params.origin);
    }
    // AU distance guides look like planetary orbits — hide them with orbit trails.
    if player_state.orbit_mode != OrbitVisualizationMode::Off
        && player_state.overlay_mode != DiagnosticOverlayMode::Hidden
    {
        draw_au_guide_rings(
            gizmos,
            params.star_vec,
            params.cam_tf.map(|t| t.translation),
            (
                params.selected_occluder,
                Some((params.star_vec, params.star_vis_r)),
            ),
        );
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
        Option<&'static Transform>,
        Option<&'static Temperature>,
        Option<&'static BlackHoleStarState>,
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
        opt_tf,
        opt_temp,
        opt_bhs,
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
            opt_tf,
            opt_temp,
            opt_bhs,
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
        &'static SimVelocity,
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
    camera_query: Query<
        (
            &Transform,
            Option<&crate::rendering::camera::PanOrbitCamera>,
        ),
        With<Camera3d>,
    >,
    star_query: CentralStarQuery<'_, '_>,
    bodies_query: CelestialBodiesQuery<'_, '_>,
    parent_query: Query<(&SimPosition, &SimVelocity, &Mass)>,
    opt_builder: Option<Res<crate::game::ui::PlanetBuilderState>>,
    opt_slingshot: Option<Res<crate::simulation::resources::SlingshotState>>,
    opt_predictor: Option<Res<crate::simulation::predictor::TrajectoryPredictorState>>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let cam_entry = camera_query.iter().next();
    let has_supernova = supernova_pool
        .as_deref()
        .is_some_and(|p| !p.explosions.is_empty());
    if should_skip_orbital_gizmos(&player_state, has_supernova)
        || cam_entry.is_some_and(|(_, c)| c.is_some_and(|cam| cam.surface_horizon_mode))
    {
        return;
    }
    let opt_star = star_query.iter().next();
    let (star_pos_dvec, star_vel_dvec, star_mass_val, star_vec) =
        if let Some((s_pos, s_vel, s_mass, ..)) = opt_star {
            (s_pos.0, s_vel.0, s_mass.0, origin.to_render(s_pos.0))
        } else {
            (DVec3::ZERO, DVec3::ZERO, 1.0, origin.to_render(DVec3::ZERO))
        };

    let (elapsed, elapsed_years) = sim_time
        .as_deref()
        .map_or((0.0, 0.0), |st| (st.visual_time_secs, st.elapsed_years));
    let cam_tf = cam_entry.map(|(t, _)| *t);
    let cam_pos = cam_tf.map(|t| t.translation);
    let star_vis_r = opt_star.map_or(0.05, |(_, _, _, s_rad, _, s_body, ..)| {
        let cam_dist = cam_pos.map_or(25.0, |cp| cp.distance(star_vec));
        crate::rendering::bodies::transforms::resolve_synced_visual_radius(
            None,
            s_rad.0,
            s_body.body_type,
            0.0,
            cam_dist,
            &config,
        )
    });
    let selected_occluder = player_state.selected_entity.and_then(|sel| {
        bodies_query.get(sel).ok().map(
            |(_, pos, _, _, _, body, _, _, opt_rad, _, _, _, opt_tf, ..)| {
                let bv = origin.to_render(pos.0);
                let cd = cam_pos.map_or(25.0, |cp| cp.distance(bv));
                let vr = crate::rendering::bodies::transforms::resolve_synced_visual_radius(
                    opt_tf,
                    opt_rad.map_or(0.00005, |r| r.0),
                    body.body_type,
                    bv.distance(star_vec),
                    cd,
                    &config,
                );
                (bv, vr)
            },
        )
    });

    if let Some((_, _, s_mass, s_rad, opt_ign, s_body, opt_evo, _, opt_quasi, opt_kerr)) = opt_star
    {
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

    let params = BodyEffectParams {
        origin,
        star_vec,
        star_pos_dvec,
        star_vel_dvec,
        star_mass_val,
        star_vis_r,
        selected_occluder,
        elapsed,
        elapsed_years,
        cam_tf,
    };

    draw_cataclysm_and_environment_effects(
        &mut gizmos,
        &player_state,
        &shockwave_pool,
        &debris_pool,
        supernova_pool.as_deref(),
        &params,
    );

    draw_celestial_bodies_batch(
        &mut gizmos,
        &bodies_query,
        &params,
        &player_state,
        &config,
        &parent_query,
    );

    let slingshot_target_state = opt_slingshot
        .as_deref()
        .and_then(|s| s.target_entity)
        .and_then(|ent| {
            bodies_query
                .get(ent)
                .ok()
                .map(|(_, _, vel, mass, ..)| (vel.0, mass.0))
        });

    draw_interactive_tool_gizmos(
        &mut gizmos,
        &player_state,
        opt_builder.as_deref(),
        opt_slingshot.as_deref(),
        slingshot_target_state,
        opt_predictor.as_deref(),
        &params,
    );
}

#[inline]
fn should_skip_orbital_gizmos(player_state: &PlayerInteractionState, has_supernova: bool) -> bool {
    player_state.overlay_mode == DiagnosticOverlayMode::Hidden && !has_supernova
}

fn draw_interactive_tool_gizmos(
    gizmos: &mut Gizmos,
    player_state: &PlayerInteractionState,
    opt_builder: Option<&crate::game::ui::PlanetBuilderState>,
    opt_slingshot: Option<&crate::simulation::resources::SlingshotState>,
    slingshot_target_state: Option<(DVec3, f64)>,
    opt_predictor: Option<&crate::simulation::predictor::TrajectoryPredictorState>,
    params: &BodyEffectParams,
) {
    if player_state.overlay_mode == DiagnosticOverlayMode::Hidden {
        return;
    }

    if player_state.active_tool == PlayerTool::GravitationalTractor {
        draw_tractor_beam_gizmo(gizmos, player_state, params.origin);
    }

    if let Some(builder) = opt_builder {
        draw_planet_builder_preview(
            gizmos,
            builder,
            params.star_pos_dvec - params.origin.0,
            params.star_mass_val,
            params.elapsed,
        );
    }

    if let Some(slingshot) = opt_slingshot {
        draw_slingshot_preview(
            gizmos,
            slingshot,
            SlingshotStarState {
                pos: params.star_pos_dvec,
                vel: params.star_vel_dvec,
                mass: params.star_mass_val,
            },
            slingshot_target_state,
            params.elapsed,
            params.origin,
        );
    }

    if let Some(predictor) = opt_predictor {
        draw_trajectory_prediction_gizmos(
            gizmos,
            predictor,
            params.star_vec,
            params.cam_tf.map(|t| t.translation),
            (
                params.selected_occluder,
                Some((params.star_vec, params.star_vis_r)),
            ),
            params.elapsed,
            params.origin,
        );
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
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    if player_state.overlay_mode == DiagnosticOverlayMode::Hidden {
        return;
    }
    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    draw_bombardment_projectiles_gizmo(
        &mut gizmos,
        &projectiles_query,
        &targets_query,
        elapsed,
        origin,
    );
}

pub fn draw_persistent_remnants_gizmos(
    mut gizmos: Gizmos,
    remnant_pool: Option<Res<PersistentRemnantPool>>,
    sim_time: Option<Res<SimTime>>,
    opt_origin: Option<Res<crate::rendering::origin::RenderOrigin>>,
) {
    let Some(pool) = remnant_pool else {
        return;
    };
    if pool.remnants.is_empty() {
        return;
    }
    let origin = opt_origin.as_deref().copied().unwrap_or_default();
    let visual_time = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);
    draw_persistent_supernova_remnants(&mut gizmos, &pool, visual_time, origin);
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
