//! Double-precision floating render origin for jitter-free true-scale rendering.

use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;

use crate::rendering::camera::PanOrbitCamera;
use crate::simulation::components::{CelestialBody, SimPosition};
use crate::simulation::probes::{FlightComputer, ProbeStatus, ProbeType, SpaceProbe};
use crate::simulation::resources::PlayerInteractionState;

/// Double-precision floating render origin in simulation coordinates (AU).
///
/// All 3D scene transforms, gizmos, shaders, and camera world coordinates are expressed relative
/// to `RenderOrigin` (`(sim_pos - origin.0).as_vec3()`), providing full 24-bit `f32` precision
/// around the focused planet, moon, rover, or camera viewpoint anywhere in the simulation without
/// mutating underlying `f64` physics in [`SimPosition`].
#[derive(Resource, ExtractResource, Debug, Clone, Copy, Default, PartialEq)]
#[extract_app(RenderApp)]
pub struct RenderOrigin(pub DVec3);

impl RenderOrigin {
    /// Converts a double-precision simulation coordinate (AU) into render-world `Vec3`.
    #[inline]
    #[must_use]
    pub fn to_render(&self, sim_pos: DVec3) -> Vec3 {
        (sim_pos - self.0).as_vec3()
    }

    /// Shifts a single-precision simulation coordinate (AU) into render-world `Vec3`.
    #[inline]
    #[must_use]
    pub fn shift_vec3(&self, sim_pos_f32: Vec3) -> Vec3 {
        self.to_render(sim_pos_f32.as_dvec3())
    }

    /// Converts a render-world `Vec3` back into double-precision simulation coordinates (AU).
    #[inline]
    #[must_use]
    pub fn to_sim(&self, render_pos: Vec3) -> DVec3 {
        self.0 + render_pos.as_dvec3()
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Matches surface vantage resolution queries without interfering with SimPosition"
)]
fn resolve_surface_origin(
    candidate_target: Option<Entity>,
    bodies_query: &Query<
        (&SimPosition, &CelestialBody),
        (Without<PanOrbitCamera>, Without<SpaceProbe>),
    >,
    probes_query: &Query<
        (&SpaceProbe, &SimPosition, Option<&FlightComputer>),
        Without<PanOrbitCamera>,
    >,
) -> Option<DVec3> {
    if let Some(ent) = candidate_target {
        if let Ok((_, _, Some(fc))) = probes_query.get(ent)
            && let Ok((pos, body)) = bodies_query.get(fc.target_entity)
            && !body.body_type.is_star_or_remnant()
        {
            return Some(pos.0);
        }
        if let Ok((pos, body)) = bodies_query.get(ent)
            && !body.body_type.is_star_or_remnant()
        {
            return Some(pos.0);
        }
    }

    for require_landed in [true, false] {
        for (probe, _, opt_fc) in probes_query.iter() {
            if probe.probe_type == ProbeType::Rover
                && (!require_landed || probe.status == ProbeStatus::Landed)
                && let Some(fc) = opt_fc
                && let Ok((pos, body)) = bodies_query.get(fc.target_entity)
                && !body.body_type.is_star_or_remnant()
            {
                return Some(pos.0);
            }
        }
    }

    for (pos, body) in bodies_query.iter() {
        if !body.body_type.is_star_or_remnant() {
            return Some(pos.0);
        }
    }

    None
}

/// Synchronizes [`RenderOrigin`] with the active camera focus, focused body, or Surface Mode host.
#[allow(
    clippy::type_complexity,
    reason = "Disjoint ECS queries for camera, celestial bodies, and probes"
)]
pub fn update_render_origin(
    mut origin: ResMut<RenderOrigin>,
    opt_player_state: Option<Res<PlayerInteractionState>>,
    mut cam_query: Query<&mut PanOrbitCamera>,
    bodies_query: Query<
        (&SimPosition, &CelestialBody),
        (Without<PanOrbitCamera>, Without<SpaceProbe>),
    >,
    probes_query: Query<
        (&SpaceProbe, &SimPosition, Option<&FlightComputer>),
        Without<PanOrbitCamera>,
    >,
) {
    let Ok(mut cam) = cam_query.single_mut() else {
        return;
    };
    cam.sync_f64_from_f32_edits();

    let next_origin = if cam.surface_horizon_mode {
        let candidate = opt_player_state
            .as_deref()
            .and_then(|ps| ps.selected_entity)
            .or(cam.target_entity);
        resolve_surface_origin(candidate, &bodies_query, &probes_query)
            .unwrap_or(cam.target_focus_au)
    } else if let Some(target_ent) = cam.target_entity
        && cam.focus_transition_progress >= 1.0
    {
        if let Ok((pos, _)) = bodies_query.get(target_ent) {
            pos.0
        } else if let Ok((_, pos, _)) = probes_query.get(target_ent) {
            pos.0
        } else {
            cam.focus_au
        }
    } else {
        cam.focus_au
    };

    if origin.0 != next_origin {
        origin.0 = next_origin;
    }
}
