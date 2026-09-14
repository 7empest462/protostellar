//! Data structures, snapshots, and query types for accretion and collision mechanics.

use bevy::math::DVec3;
use bevy::prelude::*;
use hashbrown::HashSet;
use smallvec::SmallVec;

use crate::simulation::components::*;
use crate::simulation::resources::*;

use super::super::events::*;

pub type AccretionQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut Mass,
        &'static mut SimPosition,
        &'static mut SimVelocity,
        &'static mut SimAcceleration,
        &'static mut Radius,
        &'static mut Temperature,
        &'static mut Composition,
        &'static mut CelestialBody,
        Option<&'static mut InternalDifferentiation>,
        Option<&'static mut SpinState>,
        Option<&'static mut SatelliteOf>,
        Option<&'static CentralStar>,
    ),
>;

#[derive(Clone)]
pub struct BodySnapshot {
    pub entity: Entity,
    pub mass: f64,
    pub pos: DVec3,
    pub vel: DVec3,
    pub radius: f64,
    pub temp: f64,
    pub comp: Composition,
    pub body_type: BodyType,
    pub spin: DVec3,
    pub name: String,
    pub is_central: bool,
    pub satellite_parent: Option<Entity>,
}

#[derive(Clone)]
pub struct SortedPair {
    pub primary_entity: Entity,
    pub p_m: f64,
    pub p_pos: DVec3,
    pub p_vel: DVec3,
    pub p_comp: Composition,
    pub p_type: BodyType,
    pub p_spin: DVec3,
    pub p_name: String,
    pub p_is_central: bool,
    pub secondary_entity: Entity,
    pub s_m: f64,
    pub s_pos: DVec3,
    pub s_vel: DVec3,
    pub s_comp: Composition,
    pub s_type: BodyType,
    pub s_spin: DVec3,
    pub s_name: String,
}

pub struct CollisionContext<'a, 'c1, 'c2, 'q1, 'q2, 'm1, 'm2, 'm3, 'm4> {
    pub commands: &'a mut Commands<'c1, 'c2>,
    pub bodies_query: &'a mut AccretionQuery<'q1, 'q2>,
    pub player_state: &'a mut PlayerInteractionState,
    pub merge_events: &'a mut MessageWriter<'m1, AccretionMergeEvent>,
    pub moon_events: &'a mut MessageWriter<'m2, MoonFormationEvent>,
    pub bounce_events: &'a mut MessageWriter<'m3, CollisionBounceEvent>,
    pub roche_events: &'a mut MessageWriter<'m4, RocheDisruptionEvent>,
    pub merged_away: &'a mut HashSet<Entity>,
    pub newly_formed_moons: &'a mut HashSet<Entity>,
    pub pending_despawns: &'a mut SmallVec<[Entity; 32]>,
    pub star_mass: f64,
    pub sim_time_years: f64,
}

pub fn sort_collision_pair(b1: &BodySnapshot, b2: &BodySnapshot) -> SortedPair {
    let (p, s) = if b1.is_central || (!b2.is_central && b1.mass >= b2.mass) {
        (b1, b2)
    } else {
        (b2, b1)
    };
    SortedPair {
        primary_entity: p.entity,
        p_m: p.mass,
        p_pos: p.pos,
        p_vel: p.vel,
        p_comp: p.comp,
        p_type: p.body_type,
        p_spin: p.spin,
        p_name: p.name.clone(),
        p_is_central: p.is_central,
        secondary_entity: s.entity,
        s_m: s.mass,
        s_pos: s.pos,
        s_vel: s.vel,
        s_comp: s.comp,
        s_type: s.body_type,
        s_spin: s.spin,
        s_name: s.name.clone(),
    }
}
