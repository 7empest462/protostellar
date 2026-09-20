//! Integration test suite for Protostellar simulation, physics, accretion, thermodynamics, scenarios, and rendering.

#[path = "simulation_tests/accretion.rs"]
mod accretion;
#[path = "simulation_tests/atmosphere_escape.rs"]
mod atmosphere_escape;
#[path = "simulation_tests/atmospheres.rs"]
mod atmospheres;
#[path = "simulation_tests/camera.rs"]
mod camera;
#[path = "simulation_tests/comets.rs"]
mod comets;
#[path = "simulation_tests/geological_epochs.rs"]
mod geological_epochs;
#[path = "simulation_tests/gpu_and_particles.rs"]
mod gpu_and_particles;
#[path = "simulation_tests/inspector_and_minimization.rs"]
mod inspector_and_minimization;
#[path = "simulation_tests/kozai_lidov.rs"]
mod kozai_lidov;
#[path = "simulation_tests/late_heavy_bombardment.rs"]
mod late_heavy_bombardment;
#[path = "simulation_tests/lifecycle_and_visuals.rs"]
mod lifecycle_and_visuals;
#[path = "simulation_tests/magma_and_crust.rs"]
mod magma_and_crust;
#[path = "simulation_tests/magnetospheres.rs"]
mod magnetospheres;
#[path = "simulation_tests/mechanics.rs"]
mod mechanics;
#[path = "simulation_tests/orbits_and_effects.rs"]
mod orbits_and_effects;
#[path = "simulation_tests/physics.rs"]
mod physics;
#[path = "simulation_tests/precision_events.rs"]
mod precision_events;
#[path = "simulation_tests/predictor.rs"]
mod predictor;
#[path = "simulation_tests/pulsar_magnetar.rs"]
mod pulsar_magnetar;
#[path = "simulation_tests/relativistic_jets.rs"]
mod relativistic_jets;
#[path = "simulation_tests/relativity.rs"]
mod relativity;
#[path = "simulation_tests/save_load.rs"]
mod save_load;
#[path = "simulation_tests/scenarios.rs"]
mod scenarios;
#[path = "simulation_tests/shadows_and_eclipses.rs"]
mod shadows_and_eclipses;
#[path = "simulation_tests/slingshot.rs"]
mod slingshot;
#[path = "simulation_tests/space_weather.rs"]
mod space_weather;
#[path = "simulation_tests/storms.rs"]
mod storms;
#[path = "simulation_tests/telemetry.rs"]
mod telemetry;
#[path = "simulation_tests/terraforming.rs"]
mod terraforming;
#[path = "simulation_tests/theia_moon.rs"]
mod theia_moon;
#[path = "simulation_tests/thermodynamics.rs"]
mod thermodynamics;
#[path = "simulation_tests/tides.rs"]
mod tides;
#[path = "simulation_tests/ui.rs"]
mod ui;
