//! Kozai-Lidov Secular Resonance & Hierarchical Triple Dynamics.
//!
//! Models the von Zeipel–Lidov–Kozai (ZLK) quadrupole mechanism, mutual orbital inclination
//! dynamics, General Relativistic 1PN suppression, and high-eccentricity tidal migration.

pub mod physics;
pub mod systems;
pub mod types;

pub use physics::*;
pub use systems::*;
pub use types::*;
