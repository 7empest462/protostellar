//! Core Simulation Plugin bundling physics, disk generation, accretion, and thermodynamics.

pub mod accretion;
pub mod atmosphere_escape;
pub mod biology;
pub mod campaign;
pub mod components;
pub mod disk;
pub mod disk_migration;
pub mod economy;
pub mod events;
pub mod geology;
pub mod kozai_lidov;
pub mod naming;
pub mod pebble_accretion;
pub mod physics;
pub mod predictor;
pub mod probes;
pub mod relativity;
pub mod resources;
pub mod scenarios;
pub mod serialization;
pub mod space_weather;
pub mod telemetry;
pub mod terraforming;
pub mod thermodynamics;
pub mod tides;

use bevy::prelude::*;

pub use crate::simulation::geology::{
    apply_epoch_to_world, calculate_continental_drift_phase, calculate_ocean_oxidation_progress,
    calculate_oxygen_level_pal, calculate_supercontinent_aggregation,
    calculate_vegetation_expansion, sync_geological_evolution_system, EpochTargetPlanet,
    GeologicalEpoch, GeologicalState, TimelineScrubber,
};
pub use crate::simulation::space_weather::{
    update_planetary_auroral_ovals_system, update_stellar_flares_system, AuroralOvalState,
    CmeShockwaveEvent, GeomagneticStormLevel, StellarFlareState,
};

use crate::simulation::accretion::*;
pub use crate::simulation::atmosphere_escape::{
    update_atmospheric_escape_evolution, AtmosphericEscapeConfig, AtmosphericEscapeRegime,
    AtmosphericEscapeState, AtmosphericStrippedEvent,
};
pub use crate::simulation::campaign::{
    create_mission_objectives, spawn_campaign_mission, CampaignMissionCompletedEvent,
    CampaignMissionFailedEvent, CampaignMissionId, CampaignPlugin, CampaignState, MissionObjective,
    MissionStatus, StartCampaignMissionEvent,
};
use crate::simulation::components::*;
use crate::simulation::disk::*;
pub use crate::simulation::economy::*;
pub use crate::simulation::events::{
    ActiveCosmicEvent, CosmicEventManager, CosmicEventPhase, CosmicEventResolvedEvent,
    CosmicEventStartedEvent, CosmicEventType, CosmicEventsPlugin, HazardComet, RogueInterloper,
    TonemappingDisplayMode, TriggerCosmicEvent,
};
pub use crate::simulation::kozai_lidov::{
    detect_hierarchical_triples, update_kozai_lidov_evolution, KozaiDisruptionEvent,
    KozaiLidovConfig, KozaiLidovState, KozaiRegime,
};
use crate::simulation::pebble_accretion::{
    apply_pebble_accretion, spawn_streaming_instability_minor_bodies,
};
use crate::simulation::physics::*;
pub use crate::simulation::predictor::*;
pub use crate::simulation::relativity::{
    update_relativity_evolution, GravitationalWaveMergerEvent, KerrBlackHole, RelativisticState,
    RelativityConfig,
};
use crate::simulation::resources::*;
pub use crate::simulation::resources::{HabitableZoneConfig, LagrangeOverlayConfig};
use crate::simulation::scenarios::*;
use crate::simulation::serialization::*;
use crate::simulation::telemetry::*;
pub use crate::simulation::terraforming::*;
use crate::simulation::thermodynamics::*;
pub use crate::simulation::tides::{
    cryovolcanic_ring_feeding_system, update_tidal_evolution, TidalConfig, TidalState,
};

pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::simulation::probes::ProbesPlugin)
            .add_plugins(crate::simulation::economy::EconomyPlugin)
            .add_plugins(crate::simulation::campaign::CampaignPlugin)
            .add_plugins(crate::simulation::events::CosmicEventsPlugin);
        app.init_resource::<SimulationConfig>()
            .init_resource::<TimeWarp>()
            .init_resource::<SimTime>()
            .init_resource::<EnergyMonitor>()
            .init_resource::<DiskParameters>()
            .init_resource::<PlayerInteractionState>()
            .init_resource::<SlingshotState>()
            .init_resource::<PlanetesimalSpawner>()
            .init_resource::<ActiveScenarioState>()
            .init_resource::<SimulationTelemetryHistory>()
            .init_resource::<TheiaImpactState>()
            .init_resource::<TrajectoryPredictorState>()
            .init_resource::<TidalConfig>()
            .init_resource::<RelativityConfig>()
            .init_resource::<AtmosphericEscapeConfig>()
            .init_resource::<KozaiLidovConfig>()
            .init_resource::<TidalDisruptionPool>()
            .init_resource::<HabitableZoneConfig>()
            .init_resource::<LagrangeOverlayConfig>()
            .init_resource::<GmcClusterDiskManager>()
            .add_message::<AccretionMergeEvent>()
            .add_message::<MoonFormationEvent>()
            .add_message::<CollisionBounceEvent>()
            .add_message::<RocheDisruptionEvent>()
            .add_message::<TidalDisruptionEvent>()
            .add_message::<StarIgnitionEvent>()
            .add_message::<PlanetaryEngulfmentEvent>()
            .add_message::<SupernovaEvent>()
            .add_message::<LoadScenarioEvent>()
            .add_message::<SaveSystemEvent>()
            .add_message::<LoadSystemEvent>()
            .add_message::<BombardmentEvent>()
            .add_message::<GravitationalWaveMergerEvent>()
            .add_message::<AtmosphericStrippedEvent>()
            .add_message::<KozaiDisruptionEvent>()
            .add_message::<CmeShockwaveEvent>()
            .add_systems(Startup, setup_simulation)
            .add_systems(
                Update,
                (
                    handle_load_scenario_events,
                    handle_save_system_events,
                    handle_load_system_events,
                    update_active_scenarios,
                    update_simulation_visual_time,
                    step_physics_simulation,
                    process_accretion_and_collisions.after(step_physics_simulation),
                    apply_pebble_accretion.after(step_physics_simulation),
                    spawn_streaming_instability_minor_bodies.after(step_physics_simulation),
                    direct_nebular_gas_accretion.after(step_physics_simulation),
                    update_thermodynamics.after(step_physics_simulation),
                    update_impact_basin_relaxation.after(step_physics_simulation),
                    update_atmospheric_escape_evolution.after(update_thermodynamics),
                    auto_spawn_planetesimals.after(step_physics_simulation),
                    auto_spawn_delayed_proto_earth.after(step_physics_simulation),
                    update_theia_rendezvous.after(process_accretion_and_collisions),
                    update_late_heavy_bombardment_cascade
                        .after(step_physics_simulation)
                        .before(process_accretion_and_collisions),
                    update_black_hole_star_dynamics.after(step_physics_simulation),
                    dissipate_gas_disk.after(step_physics_simulation),
                    record_planetary_telemetry.after(process_accretion_and_collisions),
                ),
            )
            .add_systems(
                Update,
                (
                    gmc_cluster_gas_accretion.after(step_physics_simulation),
                    crate::simulation::accretion::black_holes::update_black_hole_accretion_jets
                        .after(process_accretion_and_collisions),
                    update_guided_bombardment_projectiles.after(step_physics_simulation),
                    update_terraforming_atmospheres.after(update_thermodynamics),
                    update_trajectory_predictor,
                    update_tidal_evolution.after(process_accretion_and_collisions),
                    cryovolcanic_ring_feeding_system.after(update_tidal_evolution),
                    update_relativity_evolution.after(process_accretion_and_collisions),
                    detect_hierarchical_triples.after(step_physics_simulation),
                    update_kozai_lidov_evolution.after(detect_hierarchical_triples),
                    sync_geological_evolution_system.after(update_thermodynamics),
                    update_stellar_flares_system.after(step_physics_simulation),
                    update_planetary_auroral_ovals_system.after(update_stellar_flares_system),
                    crate::simulation::naming::update_auto_generated_names
                        .after(step_physics_simulation),
                    crate::simulation::biology::civilization::habitability_tracking_system
                        .after(step_physics_simulation),
                    process_roche_lobe_overflow.after(process_accretion_and_collisions),
                    crate::simulation::accretion::roche::cleanup_roche_lobe_overflow
                        .after(process_roche_lobe_overflow),
                    process_classical_novae.after(process_roche_lobe_overflow),
                    promote_stabilized_gmc_protostars.after(update_thermodynamics),
                    update_gmc_circumstellar_disks.after(step_physics_simulation),
                ),
            )
            .init_resource::<TimelineScrubber>();
    }
}

fn setup_simulation(
    mut commands: Commands,
    disk_params: Res<DiskParameters>,
    config: Res<SimulationConfig>,
    mut player_state: ResMut<PlayerInteractionState>,
) {
    let star_ent = spawn_protoplanetary_disk(&mut commands, &disk_params, &config);
    player_state.selected_entity = Some(star_ent);
}
