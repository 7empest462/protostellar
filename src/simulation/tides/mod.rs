//! Gravitational tidal dissipation, orbital circularization, spin-orbit synchronization,
//! and internal viscoelastic heating.

pub mod dissipation;
pub mod systems;
pub mod types;

pub use dissipation::*;
pub use systems::*;
pub use types::*;
