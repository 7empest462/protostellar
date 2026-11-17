//! Solar System Evolution Phase State Machine and Scientific Milestones.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::disk::belts::BeltCensus;
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
    /// Lin-Shu density wave organizes gas and star clusters into grand-design spiral arms
    SpiralGalaxyEvolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MilestoneId {
    DustCoagulation,
    PlanetesimalGrowth,
    StellarIgnition,
    GapClearing,
    CoreDifferentiation,
    StableMultiPlanet,
    GiantPlanetResonance,
    GiantPlanetEmergence,
    LateHeavyBombardment,
    PlanetesimalBombardment,
    VolatileOceanDelivery,
    PlanetaryRingGenesis,
    DynamoMagneticShield,
    BiosphereGenesis,
    StellarMetamorphosis,
    SpiralArmsEmergence,
    // GMC
    GmcJeansCollapse,
    GmcProtostar,
    GmcStarCluster,
    GmcRadiationFeedback,
    GmcClusterNucleus,
    // Exoplanets
    TrappistSurvival,
    HotJupiterParked,
    // Relativity
    SgrACloseApproach,
    QuasarIgnition,
    TidalDisruption,
    KozaiTiltExchange,
    // High-Energy & Magnetospheric Telemetry
    MagnetarSuperflare,
    GeomagneticSuperstorm,
    BinaryInspiral,
}

/// A scientific discovery milestone tracking solar system emergence.
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
    pub scattered_bodies: rustc_hash::FxHashSet<Entity>,
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
            scattered_bodies: rustc_hash::FxHashSet::default(),
            time_active_years: 0.0,
            manual_trigger_requested: false,
        }
    }
}

/// Resource tracking phase milestone metrics and statistics.
/// Resource tracking globally unlocked trophies.
#[derive(Resource, Debug, Clone, Default)]
pub struct GlobalTrophies {
    pub unlocked: rustc_hash::FxHashSet<MilestoneId>,
}

#[derive(Resource, Debug, Clone)]
pub struct PhaseManager {
    pub current_phase: SystemPhase,
    pub planet_count: usize,
    pub protoplanet_count: usize,
    pub planetesimal_count: usize,
    pub asteroid_count: usize,
    pub comet_count: usize,
    pub belt_census: BeltCensus,
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
            belt_census: BeltCensus::default(),
            disk_mass_remaining: 0.035,
            star_mass: 1.0,
            is_star_ignited: false,
            phase_description:
                "Dense protoplanetary disk orbiting young protostar. Dust and pebbles are accreting.",
            milestones: default_milestones(),
            latest_unlocked_milestone: None,
            milestone_toast_timer: 0.0,
        }
    }
}

/// Default 14 scientific milestones for Solar System formation & evolution.
pub fn default_milestones() -> Vec<ScientificMilestone> {
    vec![
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
        ScientificMilestone {
            id: MilestoneId::SpiralArmsEmergence,
            title: "🌀 14. Grand Design Spiral Galaxy Evolution",
            prompt: "Lin-Shu density wave organizes gas and star clusters into grand-design logarithmic spiral arms.",
            achieved: false,
            achieve_timestamp: None,
        },
    ]
}

/// Dedicated 13 scientific milestones for Disk Genesis (organic planet formation with no starter planets).
pub fn genesis_milestones() -> Vec<ScientificMilestone> {
    vec![
        ScientificMilestone {
            id: MilestoneId::DustCoagulation,
            title: "🌱 1. Dust Coagulation",
            prompt: "Observe microscopic grains clumping past the snow line into planetesimals.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::PlanetesimalGrowth,
            title: "☄️ 2. Oligarchic Embryo Growth",
            prompt: "Accrete enough disk solids to form a self-gravitating protoplanetary embryo (>0.005 M⊕).",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::StellarIgnition,
            title: "⭐ 3. T-Tauri Protostellar Ignition",
            prompt: "Protostellar contraction heats the core to 10 MK, igniting fusion and clearing inner gas.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::GapClearing,
            title: "🪐 4. Annular Gap Clearing",
            prompt: "Grow a major planet whose Hill sphere gravitationally clears an annular disk lane.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::CoreDifferentiation,
            title: "🌍 5. Core Differentiation",
            prompt: "Achieve iron core / silicate mantle gravitational settling inside a rocky world.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::StableMultiPlanet,
            title: "🌟 6. Multi-Planet Architecture",
            prompt: "Build a multi-planet system with three or more organically formed planets.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::GiantPlanetEmergence,
            title: "🪐 7. Giant Planet Runaway Accretion",
            prompt: "Grow a planetary core beyond the snow line massive enough to become a Gas Giant or Ice Giant.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::PlanetesimalBombardment,
            title: "☄️ 8. Organic Late Bombardment",
            prompt: "Young planets scatter leftover outer planetesimals and icy comets into the inner disk.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::VolatileOceanDelivery,
            title: "🌊 9. Volatile Ocean Genesis",
            prompt: "Endogenous pebble volatiles and cometary impacts condense into liquid surface oceans.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::PlanetaryRingGenesis,
            title: "🪐 10. Planetary Ring Genesis",
            prompt: "Tidally disrupt an icy moon or captured planetesimal inside a planet's Roche limit to form ring systems.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::DynamoMagneticShield,
            title: "🛡️ 11. Dynamo Magnetic Shield",
            prompt: "Generate a convective molten iron core and planetary rotation to sustain a magnetosphere (>0.15 G).",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::BiosphereGenesis,
            title: "🌱 12. Biosphere Genesis",
            prompt: "Evolve photosynthetic microbial life on a magnetically shielded temperate world with liquid oceans.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::StellarMetamorphosis,
            title: "🌟 13. Stellar Metamorphosis & White Dwarf Remnant",
            prompt: "Witness the central star expand into a Red Giant, engulf inner worlds, and contract into a White Dwarf.",
            achieved: false,
            achieve_timestamp: None,
        },
    ]
}

/// Dedicated 5 scientific milestones for Giant Molecular Cloud (GMC) collapse and cluster formation.
pub fn gmc_cluster_milestones() -> Vec<ScientificMilestone> {
    vec![
        ScientificMilestone {
            id: MilestoneId::GmcJeansCollapse,
            title: "🌌 1. Turbulent Fragmentation",
            prompt: "Gas density perturbations exceed the Jeans criterion, initiating gravitational collapse.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::GmcProtostar,
            title: "⭐ 2. First Protostellar Ignition",
            prompt: "Gravitational collapse compresses gas to nuclear core densities, spawning newborn protostars.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::GmcStarCluster,
            title: "✨ 3. Open Star Cluster Emergence",
            prompt: "Multiple protostellar cores decouple from the cloud into a bound stellar cluster (≥5 stars).",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::GmcRadiationFeedback,
            title: "💨 4. Ionization & Cavity Blowout",
            prompt: "Protostellar radiation pressure and outflows carve glowing HII cavities in the nebula.",
            achieved: false,
            achieve_timestamp: None,
        },
        ScientificMilestone {
            id: MilestoneId::GmcClusterNucleus,
            title: "🕳️ 5. Runaway Cluster Core Accretion",
            prompt: "Dynamical friction drives massive stars into the core, forming an intermediate-mass black hole seed.",
            achieved: false,
            achieve_timestamp: None,
        },
    ]
}

#[derive(Default)]
struct SystemStats {
    planets: usize,
    giant_planets: usize,
    protoplanets: usize,
    planetesimals: usize,
    asteroids: usize,
    comets: usize,
    belt_census: BeltCensus,
    has_differentiated: bool,
    has_rings: bool,
    has_dynamo: bool,
    has_biosphere: bool,
    total_delivered_water: f64,
    remaining_disk_mass: f64,
    is_red_giant_or_wd: bool,
    has_black_hole: bool,
    stars_count: usize,
    has_hot_jupiter: bool,
    has_active_kozai: bool,
    has_close_relativistic_orbit: bool,
    has_accreting_compact_object: bool,
    has_magnetar_flare: bool,
    has_geomagnetic_storm: bool,
    has_binary_inspiral: bool,
}

type PhaseStarQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Mass,
        &'static IgnitionState,
        Option<&'static StellarEvolutionState>,
        Option<&'static CelestialBody>,
        Option<&'static crate::simulation::neutron_stars::types::MagnetarCrustStress>,
    ),
    With<CentralStar>,
>;

type PhaseBodiesCoreOptions<'a> = (
    Option<&'a InternalDifferentiation>,
    Option<&'a VolatileInventory>,
    Option<&'a PlanetaryRingSystem>,
    Option<&'a BiosphereState>,
);

type PhaseBodiesTelemetryOptions<'a> = (
    Option<&'a crate::simulation::kozai_lidov::KozaiLidovState>,
    Option<&'a crate::simulation::relativity::RelativisticState>,
    Option<&'a crate::simulation::neutron_stars::types::MagnetarCrustStress>,
    Option<&'a crate::simulation::space_weather::AuroralOvalState>,
);

type PhaseBodiesQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Mass,
        &'static CelestialBody,
        &'static SimPosition,
        PhaseBodiesCoreOptions<'static>,
        PhaseBodiesTelemetryOptions<'static>,
    ),
    Without<CentralStar>,
>;

fn collect_system_statistics(
    star_query: &PhaseStarQuery,
    bodies_query: &PhaseBodiesQuery,
    phase_mgr: &mut PhaseManager,
) -> SystemStats {
    let mut stats = SystemStats::default();

    for (
        mass,
        body,
        pos,
        (opt_diff, opt_vol, opt_rings, opt_bio),
        (opt_kozai, opt_rel, opt_stress, opt_aurora),
    ) in bodies_query.iter()
    {
        if body.body_type == BodyType::BlackHole {
            stats.has_black_hole = true;
            if mass.0 >= 10.0 {
                stats.has_accreting_compact_object = true;
            }
        }
        if body.body_type.is_star_or_remnant() {
            stats.stars_count += 1;
        }
        let dist_au = pos.0.length();
        match body.body_type {
            BodyType::TerrestrialPlanet | BodyType::SuperEarth => stats.planets += 1,
            BodyType::GasGiant | BodyType::IceGiant => {
                stats.planets += 1;
                stats.giant_planets += 1;
                if dist_au > 0.0 && dist_au <= 0.15 {
                    stats.has_hot_jupiter = true;
                }
            }
            BodyType::Protoplanet => stats.protoplanets += 1,
            BodyType::Asteroid => {
                stats.asteroids += 1;
                stats.belt_census.record(dist_au);
            }
            BodyType::Comet => {
                stats.comets += 1;
                stats.belt_census.record(dist_au);
            }
            BodyType::Planetesimal | BodyType::DustGrain => {
                stats.planetesimals += 1;
                stats.belt_census.record(dist_au);
            }
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
        if opt_kozai.is_some_and(|k| k.is_in_resonance) {
            stats.has_active_kozai = true;
        }
        if let Some(rel) = opt_rel {
            if rel.precession_rate_arcsec_century >= 100.0 || rel.beta_v_over_c >= 0.005 {
                stats.has_close_relativistic_orbit = true;
            }
            if rel.is_coalescing || rel.gw_luminosity_watts >= 1.0e30 {
                stats.has_binary_inspiral = true;
            }
        }
        if opt_stress.is_some_and(|s| s.burst_active || s.total_starquakes_count > 0) {
            stats.has_magnetar_flare = true;
        }
        if opt_aurora.is_some_and(|a| a.geomagnetic_kp_index >= 7.0) {
            stats.has_geomagnetic_storm = true;
        }
        stats.remaining_disk_mass += mass.0;
    }

    phase_mgr.planet_count = stats.planets;
    phase_mgr.protoplanet_count = stats.protoplanets;
    phase_mgr.planetesimal_count = stats.planetesimals;
    phase_mgr.asteroid_count = stats.asteroids;
    phase_mgr.comet_count = stats.comets;
    phase_mgr.belt_census = stats.belt_census.clone();
    phase_mgr.disk_mass_remaining = stats.remaining_disk_mass;

    if let Ok((mass, ignition, opt_evo, opt_body, opt_stress)) = star_query.single() {
        stats.stars_count += 1;
        phase_mgr.star_mass = mass.0;
        phase_mgr.is_star_ignited = ignition.is_ignited;
        if opt_body.is_some_and(|b| b.body_type == BodyType::BlackHole) && mass.0 >= 10.0 {
            stats.has_accreting_compact_object = true;
        }
        if opt_stress.is_some_and(|s| s.burst_active || s.total_starquakes_count > 0) {
            stats.has_magnetar_flare = true;
        }
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
    global_trophies: &mut GlobalTrophies,
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
            MilestoneId::GiantPlanetEmergence => stats.giant_planets >= 1,
            MilestoneId::LateHeavyBombardment => lhb_state.is_active,
            MilestoneId::PlanetesimalBombardment => {
                lhb_state.is_active && (stats.planets >= 1 || lhb_state.comets_scattered > 0)
            }
            MilestoneId::VolatileOceanDelivery => stats.total_delivered_water >= 0.0005,
            MilestoneId::PlanetaryRingGenesis => stats.has_rings,
            MilestoneId::DynamoMagneticShield => stats.has_dynamo,
            MilestoneId::BiosphereGenesis => stats.has_biosphere,
            MilestoneId::StellarMetamorphosis => stats.is_red_giant_or_wd,
            MilestoneId::SpiralArmsEmergence => {
                current_sim_yr >= 2200.0 && (stats.planets + stats.protoplanets >= 2)
            }
            MilestoneId::GmcJeansCollapse => {
                stats.planets + stats.protoplanets + stats.stars_count >= 1
                    || current_sim_yr >= 30.0
            }
            MilestoneId::GmcProtostar => {
                stats.stars_count >= 2 || stats.planets >= 1 || is_star_ignited
            }
            MilestoneId::GmcStarCluster => stats.stars_count >= 5 || stats.planets >= 5,
            MilestoneId::GmcRadiationFeedback => {
                current_sim_yr >= 800.0 && (stats.stars_count >= 5 || stats.planets >= 5)
            }
            MilestoneId::GmcClusterNucleus => stats.has_black_hole || current_sim_yr >= 2500.0,
            MilestoneId::TrappistSurvival => current_sim_yr >= 1000.0 && stats.planets >= 6,
            MilestoneId::HotJupiterParked => stats.has_hot_jupiter || current_sim_yr >= 200.0,
            MilestoneId::SgrACloseApproach => {
                stats.has_close_relativistic_orbit || current_sim_yr >= 50.0
            }
            MilestoneId::QuasarIgnition => {
                stats.has_accreting_compact_object || current_sim_yr >= 20.0
            }
            MilestoneId::TidalDisruption => current_sim_yr >= 10.0,
            MilestoneId::KozaiTiltExchange => stats.has_active_kozai || current_sim_yr >= 100.0,
            MilestoneId::MagnetarSuperflare => stats.has_magnetar_flare || current_sim_yr >= 50.0,
            MilestoneId::GeomagneticSuperstorm => {
                stats.has_geomagnetic_storm || current_sim_yr >= 80.0
            }
            MilestoneId::BinaryInspiral => stats.has_binary_inspiral || current_sim_yr >= 120.0,
        };

        if passed {
            milestone.achieved = true;
            milestone.achieve_timestamp = Some(current_sim_yr);
            unlock_name = Some(milestone.title.to_string());
            global_trophies.unlocked.insert(milestone.id);
        }
    }

    if let Some(name) = unlock_name {
        phase_mgr.latest_unlocked_milestone = Some(name);
        phase_mgr.milestone_toast_timer = 6.0;
    }

    unlock_dynamic_physical_trophies(stats, phase_mgr, global_trophies);
}

fn unlock_dynamic_physical_trophies(
    stats: &SystemStats,
    phase_mgr: &mut PhaseManager,
    global_trophies: &mut GlobalTrophies,
) {
    let candidates = [
        (
            stats.has_magnetar_flare,
            MilestoneId::MagnetarSuperflare,
            "⚡ Magnetar Giant Flare",
        ),
        (
            stats.has_geomagnetic_storm,
            MilestoneId::GeomagneticSuperstorm,
            "🌌 Carrington Superstorm",
        ),
        (
            stats.has_binary_inspiral,
            MilestoneId::BinaryInspiral,
            "🌀 Relativistic Binary Inspiral",
        ),
    ];
    for (condition, id, label) in candidates {
        if condition && global_trophies.unlocked.insert(id) {
            phase_mgr.latest_unlocked_milestone = Some(label.to_string());
            phase_mgr.milestone_toast_timer = 6.0;
        }
    }
}

fn evaluate_system_phase_transitions(
    stats: &SystemStats,
    phase_mgr: &mut PhaseManager,
    lhb_state: &mut LateHeavyBombardmentState,
    next_phase: &mut NextState<SystemPhase>,
    current_sim_yr: f64,
    is_genesis: bool,
) {
    let lhb_ready = if is_genesis {
        stats.planets >= 1 && (current_sim_yr >= 800.0 || lhb_state.resonance_crossed)
    } else {
        current_sim_yr >= 800.0 || lhb_state.resonance_crossed
    };
    let lhb_description = if is_genesis {
        "☄️ Late Bombardment Epoch! Young planets gravitationally scatter outer planetesimals and icy comets inward."
    } else {
        "☄️ Late Heavy Bombardment! Giant planet resonance migrates ice giants and flings icy cometary showers inward."
    };

    if lhb_ready
        && matches!(
            phase_mgr.current_phase,
            SystemPhase::ProtoplanetaryDisk
                | SystemPhase::StarIgnition
                | SystemPhase::PlanetaryAccretion
        )
    {
        lhb_state.is_active = true;
        phase_mgr.current_phase = SystemPhase::LateHeavyBombardment;
        phase_mgr.phase_description = lhb_description;
        next_phase.set(SystemPhase::LateHeavyBombardment);
        return;
    }

    match phase_mgr.current_phase {
        SystemPhase::ProtoplanetaryDisk if is_genesis => {
            if stats.planets + stats.protoplanets >= 1 {
                phase_mgr.current_phase = SystemPhase::PlanetaryAccretion;
                phase_mgr.phase_description =
                    "Planetesimals are actively coalescing into planetary embryos.";
                next_phase.set(SystemPhase::PlanetaryAccretion);
            }
        }
        SystemPhase::StarIgnition => {
            if stats.planets + stats.protoplanets >= 1 {
                phase_mgr.current_phase = SystemPhase::PlanetaryAccretion;
                phase_mgr.phase_description =
                    "Planetesimals are actively coalescing into planetary embryos.";
                next_phase.set(SystemPhase::PlanetaryAccretion);
            }
        }
        SystemPhase::PlanetaryAccretion => {
            if lhb_ready {
                lhb_state.is_active = true;
                phase_mgr.current_phase = SystemPhase::LateHeavyBombardment;
                phase_mgr.phase_description = lhb_description;
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
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "bevy ECS query is complex"
)]
pub fn monitor_phase_transitions(
    time: Res<Time>,
    sim_time: Res<SimTime>,
    mut lhb_state: ResMut<LateHeavyBombardmentState>,
    mut next_phase: ResMut<NextState<SystemPhase>>,
    mut phase_mgr: ResMut<PhaseManager>,
    mut global_trophies: ResMut<GlobalTrophies>,
    mut ignition_events: MessageReader<StarIgnitionEvent>,
    mut load_events: Option<MessageReader<crate::simulation::scenarios::LoadScenarioEvent>>,
    opt_scenario: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    star_query: PhaseStarQuery,
    bodies_query: PhaseBodiesQuery,
) {
    if load_events
        .as_mut()
        .is_some_and(|r| r.read().next().is_some())
    {
        return;
    }

    let dt = time.delta_secs();
    if phase_mgr.milestone_toast_timer > 0.0 {
        phase_mgr.milestone_toast_timer -= dt;
    }

    let stats = collect_system_statistics(&star_query, &bodies_query, &mut phase_mgr);
    lhb_state.water_delivered_earth_masses = stats.total_delivered_water;

    let is_genesis = opt_scenario.as_deref().is_some_and(|s| {
        s.current_preset == crate::simulation::scenarios::ScenarioPreset::AccretionDiskGenesis
    });

    for _ in ignition_events.read() {
        let can_enter_ignition_phase = matches!(
            phase_mgr.current_phase,
            SystemPhase::MolecularCloudCollapse | SystemPhase::ProtoplanetaryDisk
        );
        if can_enter_ignition_phase || !is_genesis {
            phase_mgr.current_phase = SystemPhase::StarIgnition;
            phase_mgr.phase_description =
                "⭐ Hydrogen fusion has ignited in the core! Radiation pressure sweeps the inner disk.";
            next_phase.set(SystemPhase::StarIgnition);
        }
    }

    if lhb_state.manual_trigger_requested {
        lhb_state.is_active = true;
        lhb_state.manual_trigger_requested = false;
        lhb_state.resonance_crossed = true;
        lhb_state.migration_progress = 0.0;
        lhb_state.time_active_years = 0.0;
        phase_mgr.current_phase = SystemPhase::LateHeavyBombardment;
        phase_mgr.phase_description = if is_genesis {
            "☄️ Late Bombardment Epoch! Young planets gravitationally scatter outer planetesimals and icy comets inward."
        } else {
            "☄️ Late Heavy Bombardment! Giant planet resonance migrates ice giants and flings icy cometary showers inward."
        };
        next_phase.set(SystemPhase::LateHeavyBombardment);
    }

    let current_sim_yr = sim_time.elapsed_years;
    evaluate_scientific_milestones(
        &stats,
        &mut phase_mgr,
        &lhb_state,
        current_sim_yr,
        &mut global_trophies,
    );
    evaluate_system_phase_transitions(
        &stats,
        &mut phase_mgr,
        &mut lhb_state,
        &mut next_phase,
        current_sim_yr,
        is_genesis,
    );
}
