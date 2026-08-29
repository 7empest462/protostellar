//! Solar System Evolution Phase State Machine and Scientific Milestones.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::SimTime;
use crate::simulation::thermodynamics::StarIgnitionEvent;

/// Cosmological evolution phase of the solar system.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SystemPhase {
    /// Cold gas and dust collapsing under self-gravity
    #[default]
    MolecularCloudCollapse,
    /// Flattened rotating accretion disk orbiting the young protostar
    ProtoplanetaryDisk,
    /// Nuclear fusion ignites in the stellar core
    StarIgnition,
    /// Planetesimals and oligarchic protoplanetary embryos colliding
    PlanetaryAccretion,
    /// Cleared orbital lanes with stable rocky and giant worlds
    MatureSolarSystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneId {
    DustCoagulation,
    PlanetesimalGrowth,
    StellarIgnition,
    GapClearing,
    CoreDifferentiation,
    StableMultiPlanet,
}

#[derive(Debug, Clone)]
pub struct ScientificMilestone {
    pub id: MilestoneId,
    pub title: &'static str,
    pub prompt: &'static str,
    pub achieved: bool,
    pub achieve_timestamp: Option<f64>,
}

/// Resource tracking phase milestone metrics and statistics.
#[derive(Resource, Debug, Clone)]
pub struct PhaseManager {
    pub current_phase: SystemPhase,
    pub planet_count: usize,
    pub protoplanet_count: usize,
    pub planetesimal_count: usize,
    pub disk_mass_remaining: f64,
    pub star_mass: f64,
    pub is_star_ignited: bool,
    pub phase_description: &'static str,
    pub milestones: Vec<ScientificMilestone>,
    pub latest_unlocked_milestone: Option<String>,
    pub milestone_toast_timer: f32,
}

impl Default for PhaseManager {
    fn default() -> Self {
        Self {
            current_phase: SystemPhase::ProtoplanetaryDisk,
            planet_count: 0,
            protoplanet_count: 0,
            planetesimal_count: 0,
            disk_mass_remaining: 0.035,
            star_mass: 1.0,
            is_star_ignited: false,
            phase_description:
                "Dense protoplanetary disk orbiting young protostar. Dust and pebbles are accreting.",
            milestones: vec![
                ScientificMilestone {
                    id: MilestoneId::DustCoagulation,
                    title: "🌱 1. Dust Coagulation",
                    prompt: "Observe microscopic grains clumping past the snow line into planetesimals.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::PlanetesimalGrowth,
                    title: "☄️ 2. Embryo Growth",
                    prompt: "Accrete enough mass to form an oligarchic protoplanetary embryo (>0.005 M⊕).",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::StellarIgnition,
                    title: "⭐ 3. Stellar Core Ignition",
                    prompt: "Protostellar core reaches 10 MK, igniting hydrogen fusion and radiating solar wind.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::GapClearing,
                    title: "🪐 4. Annular Gap Clearing",
                    prompt: "Grow a major planet whose Hill sphere gravitationally clears an annular lane.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::CoreDifferentiation,
                    title: "🌍 5. Core Differentiation",
                    prompt: "Achieve iron core / silicate mantle gravitational settling in a rocky world.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::StableMultiPlanet,
                    title: "🌟 6. Multi-Planet Solar System",
                    prompt: "Form a stable multi-planet planetary system with co-planar orbits.",
                    achieved: false,
                    achieve_timestamp: None,
                },
            ],
            latest_unlocked_milestone: None,
            milestone_toast_timer: 0.0,
        }
    }
}

/// Evaluates phase transition conditions based on astrophysical state.
pub fn monitor_phase_transitions(
    time: Res<Time>,
    sim_time: Res<SimTime>,
    mut next_phase: ResMut<NextState<SystemPhase>>,
    mut phase_mgr: ResMut<PhaseManager>,
    mut ignition_events: MessageReader<StarIgnitionEvent>,
    star_query: Query<(&Mass, &IgnitionState), With<CentralStar>>,
    bodies_query: Query<
        (&Mass, &CelestialBody, Option<&InternalDifferentiation>),
        Without<CentralStar>,
    >,
) {
    let dt = time.delta_secs();
    if phase_mgr.milestone_toast_timer > 0.0 {
        phase_mgr.milestone_toast_timer -= dt;
    }

    // 1. Update counts and detect milestones
    let mut planets = 0;
    let mut protoplanets = 0;
    let mut planetesimals = 0;
    let mut has_differentiated = false;
    let mut remaining_disk_mass = 0.0;

    for (mass, body, opt_diff) in bodies_query.iter() {
        match body.body_type {
            BodyType::TerrestrialPlanet | BodyType::GasGiant | BodyType::IceGiant => planets += 1,
            BodyType::Protoplanet => protoplanets += 1,
            BodyType::Planetesimal | BodyType::DustGrain => planetesimals += 1,
            _ => {}
        }
        if let Some(diff) = opt_diff {
            if diff.is_differentiated {
                has_differentiated = true;
            }
        }
        remaining_disk_mass += mass.0;
    }

    phase_mgr.planet_count = planets;
    phase_mgr.protoplanet_count = protoplanets;
    phase_mgr.planetesimal_count = planetesimals;
    phase_mgr.disk_mass_remaining = remaining_disk_mass;

    if let Ok((mass, ignition)) = star_query.single() {
        phase_mgr.star_mass = mass.0;
        phase_mgr.is_star_ignited = ignition.is_ignited;
    }

    // 2. Evaluate Scientific Milestones
    let current_sim_yr = sim_time.elapsed_years;
    let is_star_ignited = phase_mgr.is_star_ignited;
    let mut unlock_name = None;

    for milestone in phase_mgr.milestones.iter_mut() {
        if milestone.achieved {
            continue;
        }

        let passed = match milestone.id {
            MilestoneId::DustCoagulation => planetesimals >= 4,
            MilestoneId::PlanetesimalGrowth => protoplanets + planets >= 1,
            MilestoneId::StellarIgnition => is_star_ignited,
            MilestoneId::GapClearing => planets >= 1,
            MilestoneId::CoreDifferentiation => has_differentiated,
            MilestoneId::StableMultiPlanet => planets >= 3,
        };

        if passed {
            milestone.achieved = true;
            milestone.achieve_timestamp = Some(current_sim_yr);
            unlock_name = Some(milestone.title.to_string());
        }
    }

    if let Some(name) = unlock_name {
        phase_mgr.latest_unlocked_milestone = Some(name);
        phase_mgr.milestone_toast_timer = 6.0;
    }

    // 3. Check for Ignition Event
    for _ in ignition_events.read() {
        phase_mgr.current_phase = SystemPhase::StarIgnition;
        phase_mgr.phase_description =
            "⭐ Hydrogen fusion has ignited in the core! Radiation pressure sweeps the inner disk.";
        next_phase.set(SystemPhase::StarIgnition);
    }

    // 4. Check for Accretion & Mature System Transitions
    match phase_mgr.current_phase {
        SystemPhase::StarIgnition => {
            if planets + protoplanets >= 1 {
                phase_mgr.current_phase = SystemPhase::PlanetaryAccretion;
                phase_mgr.phase_description =
                    "Planetesimals are actively coalescing into planetary embryos.";
                next_phase.set(SystemPhase::PlanetaryAccretion);
            }
        }
        SystemPhase::PlanetaryAccretion if planets >= 3 => {
            phase_mgr.current_phase = SystemPhase::MatureSolarSystem;
            phase_mgr.phase_description = "Orbits have stabilized into a mature planetary system.";
            next_phase.set(SystemPhase::MatureSolarSystem);
        }
        _ => {}
    }
}
