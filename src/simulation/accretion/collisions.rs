//! Collision detection and accretion physics processing.

use bevy::math::DVec3;
use bevy::prelude::*;
use hashbrown::HashSet;
use smallvec::SmallVec;
use std::f64::consts::PI;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

use super::events::*;
