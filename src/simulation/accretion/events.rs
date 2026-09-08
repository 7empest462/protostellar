//! Accretion and collision event definitions.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::BodyType;

/// Event fired when a grazing giant impact shears an impactor into an orbiting natural moon.
#[derive(Message, Debug, Clone)]
pub struct MoonFormationEvent {
    pub parent_entity: Entity,
    pub moon_entity: Entity,
    pub moon_mass: f64,
    pub orbital_radius_au: f64,
    pub orbital_period_years: f64,
}

/// Event fired when two celestial bodies collide and merge into a single entity.
#[derive(Message, Debug, Clone)]
pub struct AccretionMergeEvent {
    pub primary_entity: Entity,
    pub secondary_entity: Entity,
    pub merged_mass: f64,
    pub merged_position: DVec3,
    pub merged_velocity: DVec3,
    pub new_body_type: BodyType,
    pub energy_released: f64,
}

/// Event fired when two bodies undergo a grazing bounce or partial collision.
#[derive(Message, Debug, Clone)]
pub struct CollisionBounceEvent {
    pub entity1: Entity,
    pub entity2: Entity,
    pub relative_velocity_km_s: f64,
    pub impact_parameter: f64,
}

/// Event fired when a body crosses the tidal Roche limit and is disrupted into a debris ring.
#[derive(Message, Debug, Clone)]
pub struct RocheDisruptionEvent {
    pub disrupted_entity: Entity,
    pub primary_entity: Entity,
    pub disruption_radius: f64,
    pub ring_mass_earth: f64,
    pub ice_fraction: f32,
    pub silicate_fraction: f32,
    pub primary_pos: Vec3,
    pub disruption_pos: Vec3,
    pub primary_name: String,
    pub disrupted_name: String,
}
