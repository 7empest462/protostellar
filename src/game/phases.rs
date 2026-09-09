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
    pub asteroid_count: usize,
    pub comet_count: usize,
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
            asteroid_count: 0,
            comet_count: 0,
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

#[derive(Default)]
struct SystemStats {
    planets: usize,
    protoplanets: usize,
    planetesimals: usize,
    asteroids: usize,
    comets: usize,
    has_differentiated: bool,
    has_rings: bool,
    has_dynamo: bool,
    has_biosphere: bool,
    total_delivered_water: f64,
    remaining_disk_mass: f64,
    is_red_giant_or_wd: bool,
}

#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
fn collect_system_statistics(
    star_query: &Query<(&Mass, &IgnitionState, Option<&StellarEvolutionState>), With<CentralStar>>,
    bodies_query: &Query<
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
    phase_mgr: &mut PhaseManager,
) -> SystemStats {
    let mut stats = SystemStats::default();

    for (mass, body, opt_diff, opt_vol, opt_rings, opt_bio) in bodies_query.iter() {
        match body.body_type {
            BodyType::TerrestrialPlanet
            | BodyType::SuperEarth
            | BodyType::GasGiant
            | BodyType::IceGiant => stats.planets += 1,
            BodyType::Protoplanet => stats.protoplanets += 1,
            BodyType::Asteroid => stats.asteroids += 1,
            BodyType::Comet => stats.comets += 1,
            BodyType::Planetesimal | BodyType::DustGrain => stats.planetesimals += 1,
            _ => {}
        }
        if let Some(diff) = opt_diff {
            if diff.is_differentiated {
                stats.has_differentiated = true;
            }
            if diff.magnetic_field_gauss >= 0.15 {
                stats.has_dynamo = true;
            }
        }
        if let Some(vol) = opt_vol {
            stats.total_delivered_water += vol.delivered_water_m_earth;
        }
        if opt_rings.is_some() {
            stats.has_rings = true;
        }
        if let Some(bio) = opt_bio {
            if bio.biomass_coverage_frac >= 0.02 || bio.emergence_year.is_some() {
                stats.has_biosphere = true;
            }
        }
        stats.remaining_disk_mass += mass.0;
    }

    phase_mgr.planet_count = stats.planets;
    phase_mgr.protoplanet_count = stats.protoplanets;
    phase_mgr.planetesimal_count = stats.planetesimals;
    phase_mgr.asteroid_count = stats.asteroids;
    phase_mgr.comet_count = stats.comets;
    phase_mgr.disk_mass_remaining = stats.remaining_disk_mass;

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
                stats.is_red_giant_or_wd = true;
            }
        }
    }

    stats
}

fn evaluate_scientific_milestones(
    stats: &SystemStats,
    phase_mgr: &mut PhaseManager,
    lhb_state: &LateHeavyBombardmentState,
    current_sim_yr: f64,
) {
    let is_star_ignited = phase_mgr.is_star_ignited;
    let mut unlock_name = None;

    for milestone in &mut phase_mgr.milestones {
        if milestone.achieved {
            continue;
        }

        let passed = match milestone.id {
            MilestoneId::DustCoagulation => stats.planetesimals >= 4,
            MilestoneId::PlanetesimalGrowth => stats.protoplanets + stats.planets >= 1,
            MilestoneId::StellarIgnition => is_star_ignited,
            MilestoneId::GapClearing => stats.planets >= 1,
            MilestoneId::CoreDifferentiation => stats.has_differentiated,
            MilestoneId::StableMultiPlanet => stats.planets >= 3,
            MilestoneId::GiantPlanetResonance => lhb_state.resonance_crossed,
            MilestoneId::LateHeavyBombardment => lhb_state.is_active,
            MilestoneId::VolatileOceanDelivery => stats.total_delivered_water >= 0.0005,
            MilestoneId::PlanetaryRingGenesis => stats.has_rings,
            MilestoneId::DynamoMagneticShield => stats.has_dynamo,
            MilestoneId::BiosphereGenesis => stats.has_biosphere,
            MilestoneId::StellarMetamorphosis => stats.is_red_giant_or_wd,
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
}

fn evaluate_system_phase_transitions(
    stats: &SystemStats,
    phase_mgr: &mut PhaseManager,
    lhb_state: &mut LateHeavyBombardmentState,
    next_phase: &mut NextState<SystemPhase>,
    current_sim_yr: f64,
) {
    match phase_mgr.current_phase {
        SystemPhase::StarIgnition => {
            if stats.planets + stats.protoplanets >= 1 {
                phase_mgr.current_phase = SystemPhase::PlanetaryAccretion;
                phase_mgr.phase_description =
                    "Planetesimals are actively coalescing into planetary embryos.";
                next_phase.set(SystemPhase::PlanetaryAccretion);
            }
        }
        SystemPhase::PlanetaryAccretion => {
            if stats.planets >= 3 && current_sim_yr >= 800.0 {
                lhb_state.is_active = true;
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
        SystemPhase::MatureSolarSystem if stats.is_red_giant_or_wd => {
            phase_mgr.current_phase = SystemPhase::StellarMetamorphosis;
            phase_mgr.phase_description =
                "🌟 Stellar Metamorphosis! The star has swelled into a Red Giant or shed its envelope into a White Dwarf.";
            next_phase.set(SystemPhase::StellarMetamorphosis);
        }
        _ => {}
    }
}

/// Evaluates phase transition conditions based on astrophysical state.
#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
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

    let stats = collect_system_statistics(&star_query, &bodies_query, &mut phase_mgr);
    lhb_state.water_delivered_earth_masses = stats.total_delivered_water;

    for _ in ignition_events.read() {
        phase_mgr.current_phase = SystemPhase::StarIgnition;
        phase_mgr.phase_description =
            "⭐ Hydrogen fusion has ignited in the core! Radiation pressure sweeps the inner disk.";
        next_phase.set(SystemPhase::StarIgnition);
    }

    if lhb_state.manual_trigger_requested {
        lhb_state.is_active = true;
        lhb_state.manual_trigger_requested = false;
        lhb_state.resonance_crossed = true;
        phase_mgr.current_phase = SystemPhase::LateHeavyBombardment;
        phase_mgr.phase_description =
            "☄️ Late Heavy Bombardment! Giant planet resonance migrates ice giants and flings icy cometary showers inward.";
        next_phase.set(SystemPhase::LateHeavyBombardment);
    }

    let current_sim_yr = sim_time.elapsed_years;
    evaluate_scientific_milestones(&stats, &mut phase_mgr, &lhb_state, current_sim_yr);
    evaluate_system_phase_transitions(
        &stats,
        &mut phase_mgr,
        &mut lhb_state,
        &mut next_phase,
        current_sim_yr,
    );
}
