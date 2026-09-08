//! Integration test suite for Protostellar simulation, physics, accretion, thermodynamics, scenarios, and rendering.

#[path = "simulation_tests/accretion.rs"]
mod accretion;
#[path = "simulation_tests/camera.rs"]
mod camera;
#[path = "simulation_tests/gpu_and_particles.rs"]
mod gpu_and_particles;
#[path = "simulation_tests/lifecycle_and_visuals.rs"]
mod lifecycle_and_visuals;
#[path = "simulation_tests/mechanics.rs"]
mod mechanics;
#[path = "simulation_tests/orbits_and_effects.rs"]
mod orbits_and_effects;
#[path = "simulation_tests/physics.rs"]
mod physics;
#[path = "simulation_tests/scenarios.rs"]
mod scenarios;
#[path = "simulation_tests/thermodynamics.rs"]
mod thermodynamics;
#[path = "simulation_tests/ui.rs"]
mod ui;
