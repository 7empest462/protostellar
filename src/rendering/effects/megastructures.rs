use crate::simulation::components::celestial::{CentralStar, SimPosition};
use crate::simulation::components::planetary::BiosphereState;
use crate::simulation::resources::SimTime;
use bevy::prelude::*;

#[derive(Component)]
pub struct DysonSwarm {
    pub completion_fraction: f32, // 0.0 to 1.0
    pub mirror_count: usize,
}

pub fn update_megastructure_construction(
    mut commands: Commands,
    query_planets: Query<&BiosphereState>,
    mut query_stars: Query<(Entity, Option<&mut DysonSwarm>), With<CentralStar>>,
    sim_time: Res<SimTime>,
) {
    let dt = sim_time.current_dt_yr as f32;
    let mut highest_techno = 0.0;

    for bio in query_planets.iter() {
        if bio.technosignature > highest_techno {
            highest_techno = bio.technosignature;
        }
    }

    if highest_techno > 1.5 {
        for (star_entity, opt_swarm) in query_stars.iter_mut() {
            if let Some(mut swarm) = opt_swarm {
                swarm.completion_fraction = (swarm.completion_fraction + 0.01 * dt).min(1.0);
                swarm.mirror_count = (swarm.completion_fraction * 100_000.0) as usize;
                // Light interception is handled dynamically in thermodynamics.rs
            } else if let Ok(mut ecmd) = commands.get_entity(star_entity) {
                ecmd.insert(DysonSwarm {
                    completion_fraction: 0.001,
                    mirror_count: 100,
                });
            }
        }
    }
}

pub fn draw_dyson_swarms(
    mut gizmos: Gizmos,
    query_swarms: Query<(&SimPosition, &DysonSwarm)>,
    sim_time: Option<Res<SimTime>>,
) {
    let elapsed = sim_time.as_deref().map_or(0.0, |st| st.visual_time_secs);

    for (pos, swarm) in query_swarms.iter() {
        let center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        let base_radius = 0.25; // roughly 0.25 AU
        let num_orbits = (swarm.completion_fraction * 60.0) as i32;

        for i in 0..num_orbits {
            let offset_angle = (i as f32) * 1.37 + elapsed * 0.05;
            let tilt = (i as f32) * 0.8;

            let normal = Vec3::new(tilt.cos(), tilt.sin(), offset_angle.sin()).normalize();
            if let Ok(dir) = Dir3::new(normal) {
                let rot = Quat::from_rotation_arc(Vec3::Z, dir.as_vec3());
                let color = Color::srgba(1.0, 0.9, 0.4, 0.4 * swarm.completion_fraction);
                gizmos.circle(
                    Isometry3d::new(center, rot),
                    base_radius + (i as f32) * 0.001,
                    color,
                );
            }
        }
    }
}
