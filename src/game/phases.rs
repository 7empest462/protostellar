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
    /// Giant planet orbital resonance instability and cometary bombardment
    LateHeavyBombardment,
    /// Cleared orbital lanes with stable rocky and giant worlds
    MatureSolarSystem,
    /// Far-future stellar metamorphosis: Red Giant expansion and White Dwarf remnant
    StellarMetamorphosis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MilestoneId {
    DustCoagulation,
    PlanetesimalGrowth,
    StellarIgnition,
    GapClearing,
    CoreDifferentiation,
    StableMultiPlanet,
    GiantPlanetResonance,
    LateHeavyBombardment,
    VolatileOceanDelivery,
    PlanetaryRingGenesis,
    DynamoMagneticShield,
    BiosphereGenesis,
    StellarMetamorphosis,
}

#[derive(Debug, Clone)]
pub struct ScientificMilestone {
    pub id: MilestoneId,
    pub title: &'static str,
    pub prompt: &'static str,
    pub achieved: bool,
    pub achieve_timestamp: Option<f64>,
}

/// Dynamic tracker for the Late Heavy Bombardment & Giant Planet Migration epoch.
#[derive(Resource, Debug, Clone)]
pub struct LateHeavyBombardmentState {
    pub is_active: bool,
    pub resonance_ratio: f64,
    pub resonance_crossed: bool,
    pub migration_progress: f64, // 0.0 to 1.0
    pub water_delivered_earth_masses: f64,
    pub comets_scattered: usize,
    pub time_active_years: f64,
    pub manual_trigger_requested: bool,
}

impl Default for LateHeavyBombardmentState {
    fn default() -> Self {
        Self {
            is_active: false,
            resonance_ratio: 1.85,
            resonance_crossed: false,
            migration_progress: 0.0,
            water_delivered_earth_masses: 0.0,
            comets_scattered: 0,
            time_active_years: 0.0,
            manual_trigger_requested: false,
        }
    }
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
                    title: "🌟 6. Multi-Planet Architecture",
                    prompt: "Form a multi-planet system with co-planar stable orbits.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::GiantPlanetResonance,
                    title: "🪐 7. 2:1 Giant Resonance",
                    prompt: "Jupiter and Saturn cross the critical 2:1 mean-motion orbital resonance.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::LateHeavyBombardment,
                    title: "☄️ 8. Late Heavy Bombardment",
                    prompt: "Ice giants migrate outward into the Kuiper Belt, scattering icy cometary showers inward.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::VolatileOceanDelivery,
                    title: "🌊 9. Volatile Ocean Genesis",
                    prompt: "Cometary bombardments deliver volatile water ice, creating surface oceans on rocky worlds.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::PlanetaryRingGenesis,
                    title: "🪐 10. Planetary Ring Genesis",
                    prompt: "Tidally disrupt an icy moon or captured planetesimal inside a planet's fluid Roche limit to form ring systems.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::DynamoMagneticShield,
                    title: "🛡️ 11. Dynamo Magnetic Shield",
                    prompt: "Generate a convective molten iron core and planetary rotation to establish a protective magnetosphere (>0.15 G).",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::BiosphereGenesis,
                    title: "🌱 12. Biosphere Genesis",
                    prompt: "Evolve photosynthetic microbial life and vegetation on a shielded temperate world with liquid surface oceans.",
                    achieved: false,
                    achieve_timestamp: None,
                },
                ScientificMilestone {
                    id: MilestoneId::StellarMetamorphosis,
                    title: "🌟 13. Stellar Metamorphosis & White Dwarf Remnant",
                    prompt: "Witness the central star expand into a Red Giant, engulf inner worlds, and contract into a degenerate White Dwarf.",
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
    mut lhb_state: ResMut<LateHeavyBombardmentState>,
    mut next_phase: ResMut<NextState<SystemPhase>>,
    mut phase_mgr: ResMut<PhaseManager>,
    mut ignition_events: MessageReader<StarIgnitionEvent>,
    star_query: Query<(&Mass, &IgnitionState, Option<&StellarEvolutionState>), With<CentralStar>>,
    bodies_query: Query<
        (
            &Mass,
            &CelestialBody,
            Option<&InternalDifferentiation>,
            Option<&VolatileInventory>,
            Option<&PlanetaryRingSystem>,
            Option<&BiosphereState>,
        ),
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
    let mut has_rings = false;
    let mut has_dynamo = false;
    let mut has_biosphere = false;
    let mut total_delivered_water = 0.0;
    let mut remaining_disk_mass = 0.0;
    let mut is_red_giant_or_wd = false;

    for (mass, body, opt_diff, opt_vol, opt_rings, opt_bio) in bodies_query.iter() {
        match body.body_type {
            BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::GasGiant
            | BodyType::IceGiant => planets += 1,
            BodyType::Protoplanet => protoplanets += 1,
            BodyType::Planetesimal | BodyType::DustGrain => planetesimals += 1,
            _ => {}
        }
        if let Some(diff) = opt_diff {
            if diff.is_differentiated {
                has_differentiated = true;
            }
            if diff.magnetic_field_gauss >= 0.15 {
                has_dynamo = true;
            }
        }
        if let Some(vol) = opt_vol {
            total_delivered_water += vol.delivered_water_m_earth;
        }
        if opt_rings.is_some() {
            has_rings = true;
        }
        if let Some(bio) = opt_bio {
            if bio.biomass_coverage_frac >= 0.02 || bio.emergence_year.is_some() {
                has_biosphere = true;
            }
        }
        remaining_disk_mass += mass.0;
    }

    phase_mgr.planet_count = planets;
    phase_mgr.protoplanet_count = protoplanets;
    phase_mgr.planetesimal_count = planetesimals;
    phase_mgr.disk_mass_remaining = remaining_disk_mass;

    if let Ok((mass, ignition, opt_evo)) = star_query.single() {
        phase_mgr.star_mass = mass.0;
        phase_mgr.is_star_ignited = ignition.is_ignited;
        if let Some(evo) = opt_evo {
            if matches!(
                evo.phase,
                StellarEvolutionPhase::RedGiantBranch
                    | StellarEvolutionPhase::HeliumFlashAgb
                    | StellarEvolutionPhase::PlanetaryNebulaEjection
                    | StellarEvolutionPhase::WhiteDwarf
            ) {
                is_red_giant_or_wd = true;
            }
        }
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
            MilestoneId::GiantPlanetResonance => lhb_state.resonance_crossed,
            MilestoneId::LateHeavyBombardment => lhb_state.is_active,
            MilestoneId::VolatileOceanDelivery => total_delivered_water >= 0.0005,
            MilestoneId::PlanetaryRingGenesis => has_rings,
            MilestoneId::DynamoMagneticShield => has_dynamo,
            MilestoneId::BiosphereGenesis => has_biosphere,
            MilestoneId::StellarMetamorphosis => is_red_giant_or_wd,
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

    // 4. Check for Accretion, LHB, and Mature System Transitions
    match phase_mgr.current_phase {
        SystemPhase::StarIgnition => {
            if planets + protoplanets >= 1 {
                phase_mgr.current_phase = SystemPhase::PlanetaryAccretion;
                phase_mgr.phase_description =
                    "Planetesimals are actively coalescing into planetary embryos.";
                next_phase.set(SystemPhase::PlanetaryAccretion);
            }
        }
        SystemPhase::PlanetaryAccretion => {
            // Trigger LHB automatically after planetary accretion epoch or manual trigger
            if lhb_state.manual_trigger_requested || (planets >= 3 && current_sim_yr >= 800.0) {
                lhb_state.is_active = true;
                lhb_state.manual_trigger_requested = false;
                phase_mgr.current_phase = SystemPhase::LateHeavyBombardment;
                phase_mgr.phase_description =
                    "☄️ Late Heavy Bombardment! Giant planet resonance migrates ice giants and flings icy cometary showers inward.";
                next_phase.set(SystemPhase::LateHeavyBombardment);
            }
        }
        SystemPhase::LateHeavyBombardment
            if lhb_state.migration_progress >= 0.95 && current_sim_yr >= 3500.0 =>
        {
            phase_mgr.current_phase = SystemPhase::MatureSolarSystem;
            phase_mgr.phase_description =
                "🌟 Orbits have relaxed into stable, clean architectures with water-bearing worlds.";
            next_phase.set(SystemPhase::MatureSolarSystem);
        }
        SystemPhase::MatureSolarSystem if is_red_giant_or_wd => {
            phase_mgr.current_phase = SystemPhase::StellarMetamorphosis;
            phase_mgr.phase_description =
                "🌟 Stellar Metamorphosis! The star has swelled into a Red Giant or shed its envelope into a White Dwarf.";
            next_phase.set(SystemPhase::StellarMetamorphosis);
        }
        _ => {}
    }
}
