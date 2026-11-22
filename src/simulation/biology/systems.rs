//! Living civilization evolution, thermal stabilization, autonomous colonization, and trophies.

use bevy::prelude::*;
use rustc_hash::FxHashSet;

use super::types::{
    CivilizationMilestoneEvent, CivilizationState, ColonyFoundedEvent, KardashevEra,
    technosignature_from_kardashev,
};
use crate::game::phases::{GlobalTrophies, MilestoneId, PhaseManager};
use crate::game::ui::types::NotificationToast;
use crate::rendering::effects::megastructures::DysonSwarm;
use crate::simulation::components::celestial::{BodyType, CelestialBody, CentralStar, Temperature};
use crate::simulation::components::planetary::{
    BiosphereState, ClimateRegime, PlanetaryClimate, VolatileInventory,
};
use crate::simulation::economy::{ProgressionState, ScienceData, TechnologyId, UnlockedTechs};
use crate::simulation::resources::{SimTime, TimeWarp};
use crate::simulation::terraforming::TerraformingAtmosphere;

type CivWorldItem<'a> = (
    Entity,
    &'a CelestialBody,
    Option<&'a Temperature>,
    Option<&'a mut PlanetaryClimate>,
    Option<&'a VolatileInventory>,
    Option<&'a mut TerraformingAtmosphere>,
    Option<&'a mut BiosphereState>,
    Option<&'a mut CivilizationState>,
);

type ColonyCandidateItem<'a> = (
    Entity,
    &'a CelestialBody,
    Option<&'a PlanetaryClimate>,
    Option<&'a VolatileInventory>,
    Option<&'a BiosphereState>,
    Option<&'a CivilizationState>,
);

struct TechBuffs {
    has_mirror_array: bool,
    has_defense_grid: bool,
    has_mag_shield: bool,
    has_ascension: bool,
    has_antimatter: bool,
    dyson_support: f32,
}

fn gather_tech_buffs(
    opt_techs: Option<&UnlockedTechs>,
    dyson_query: &Query<&DysonSwarm, With<CentralStar>>,
) -> TechBuffs {
    let has = |id| opt_techs.is_some_and(|t| t.has(id));
    let max_dyson = dyson_query
        .iter()
        .map(|s| s.completion_fraction)
        .fold(0.0_f32, f32::max);
    TechBuffs {
        has_mirror_array: has(TechnologyId::OrbitalMirrorArray),
        has_defense_grid: has(TechnologyId::PlanetaryDefenseGrid),
        has_mag_shield: has(TechnologyId::MagnetosphericShielding),
        has_ascension: has(TechnologyId::KardashevAscension),
        has_antimatter: has(TechnologyId::AntimatterDrive),
        dyson_support: if max_dyson >= 0.25 { max_dyson } else { 0.0 },
    }
}

fn apply_thermal_stabilization_buffs(
    opt_climate: &mut Option<Mut<'_, PlanetaryClimate>>,
    opt_terra: &mut Option<Mut<'_, TerraformingAtmosphere>>,
    has_mirror_array: bool,
    stabilizer_secs: f32,
    dt_yr: f64,
    is_inhabited: bool,
) {
    let dt_f = dt_yr.clamp(0.0, 25.0) as f32;
    if has_mirror_array
        && (is_inhabited || opt_terra.is_some())
        && let Some(climate) = opt_climate.as_deref_mut()
    {
        let diff = (288.0 - climate.surface_temperature_k).clamp(-45.0, 45.0);
        let rate = (0.04 * dt_f).clamp(0.0, 0.35);
        climate.surface_temperature_k += diff * rate;
        if (265.0..=335.0).contains(&climate.surface_temperature_k)
            && climate.climate_regime != ClimateRegime::AirlessVacuum
        {
            climate.climate_regime = ClimateRegime::TemperateHabitable;
        }
    }

    if stabilizer_secs > 0.0 {
        if let Some(climate) = opt_climate.as_deref_mut() {
            let pull = ((288.0 - climate.surface_temperature_k) * 0.25).clamp(-60.0, 60.0);
            climate.surface_temperature_k += pull * (0.35 + dt_f.min(0.65));
            if (260.0..=345.0).contains(&climate.surface_temperature_k) {
                climate.climate_regime = ClimateRegime::TemperateHabitable;
            }
        }
        if let Some(terra) = opt_terra.as_deref_mut() {
            terra.impact_dust_optical_depth = (terra.impact_dust_optical_depth * 0.65).max(0.0);
        }
    }
}

fn step_civilization_growth(
    entity: Entity,
    name: &str,
    civ: &mut CivilizationState,
    bio: &mut BiosphereState,
    env: (f32, f32),
    buffs: &TechBuffs,
    dt_yr: f64,
    dt_vis: f32,
    milestones: &mut MessageWriter<CivilizationMilestoneEvent>,
) {
    let (temp_k, pressure_bar) = env;
    let prev_stability = civ.stability;
    civ.shield_overdrive_secs = (civ.shield_overdrive_secs - dt_vis).max(0.0);
    civ.climate_stabilizer_secs = (civ.climate_stabilizer_secs - dt_vis).max(0.0);
    civ.colony_cooldown_years = (civ.colony_cooldown_years - dt_yr).max(0.0);

    let hostile_temp = !(140.0..=480.0).contains(&temp_k);
    let vacuum = pressure_bar < 0.005;
    let protected = civ.climate_stabilizer_secs > 0.0
        || (buffs.has_mirror_array && !vacuum)
        || buffs.has_ascension;
    let is_hostile = (hostile_temp || vacuum) && !protected;

    let dt_eff = dt_yr.clamp(0.0, 200.0) as f32;
    if is_hostile {
        civ.stability = (civ.stability - 0.03 * dt_eff.max(0.02)).max(0.0);
        civ.great_filter_stress = (civ.great_filter_stress + 0.02 * dt_eff.max(0.02)).min(1.0);
        if prev_stability > 0.02 && civ.stability <= 0.01 {
            civ.population_billions = (civ.population_billions * 0.20).max(0.01);
            milestones.write(CivilizationMilestoneEvent {
                world_entity: entity,
                world_name: name.to_string(),
                era: civ.era(),
                kardashev_level: civ.kardashev_level,
                collapsed: true,
            });
        }
        return;
    }

    let growth_mult = if buffs.has_ascension { 2.0 } else { 1.0 };
    let stress_damp = if (0.68..=1.02).contains(&civ.kardashev_level) && !buffs.has_ascension {
        civ.great_filter_stress = (civ.great_filter_stress + 0.004 * dt_eff).min(0.85);
        0.75
    } else {
        civ.great_filter_stress = (civ.great_filter_stress - 0.015 * dt_eff).max(0.0);
        1.0
    };

    let stab_gain = if (255.0..=330.0).contains(&temp_k) {
        0.015
    } else {
        0.005
    };
    civ.stability = (civ.stability + stab_gain * dt_eff.max(0.01)).clamp(0.05, 1.0);
    civ.kardashev_level = (civ.kardashev_level
        + 0.004 * dt_eff * growth_mult * stress_damp * (0.4 + civ.stability * 0.6))
        .clamp(0.40, 2.20);

    let target_infra = ((civ.kardashev_level - 0.65) / 1.35).clamp(0.05, 1.0);
    let infra_rate = if buffs.has_ascension && civ.kardashev_level >= 0.90 {
        0.08 * dt_eff.max(0.01)
    } else {
        0.04 * dt_eff.max(0.01)
    };
    civ.orbital_infrastructure += (target_infra - civ.orbital_infrastructure) * infra_rate.min(1.0);

    let mut target_def = 0.15 + civ.orbital_infrastructure * 0.45 + buffs.dyson_support * 0.20;
    if buffs.has_defense_grid {
        target_def += 0.22;
    }
    if buffs.has_mag_shield {
        target_def += 0.18;
    }
    if civ.shield_overdrive_secs > 0.0 {
        target_def = 1.0;
    }
    civ.defense_readiness +=
        (target_def.clamp(0.10, 1.0) - civ.defense_readiness) * (0.12 * dt_eff.max(0.02)).min(1.0);
    civ.population_billions = (civ.population_billions
        + f64::from(0.012 * dt_eff * civ.stability * growth_mult))
    .clamp(0.05, 900.0);

    bio.technosignature = technosignature_from_kardashev(civ.kardashev_level);

    let new_era = civ.era();
    let bit = 1u8 << new_era.milestone_bit();
    if new_era.milestone_bit() >= KardashevEra::Industrial.milestone_bit()
        && (civ.era_milestone_mask & bit) == 0
    {
        civ.era_milestone_mask |= bit;
        milestones.write(CivilizationMilestoneEvent {
            world_entity: entity,
            world_name: name.to_string(),
            era: new_era,
            kardashev_level: civ.kardashev_level,
            collapsed: false,
        });
    }
}

fn find_best_colony_target(
    origin_ent: Entity,
    candidates: &[(Entity, String, i32)],
    colonized: &FxHashSet<Entity>,
) -> Option<(Entity, String)> {
    candidates
        .iter()
        .filter(|(e, _, _)| *e != origin_ent && !colonized.contains(e))
        .max_by_key(|(_, _, score)| *score)
        .map(|(e, n, _)| (*e, n.clone()))
}

fn collect_colony_candidates(
    query: &Query<ColonyCandidateItem>,
) -> (FxHashSet<Entity>, Vec<(Entity, String, i32)>) {
    let mut colonized: FxHashSet<Entity> = FxHashSet::default();
    let mut colony_candidates: Vec<(Entity, String, i32)> = Vec::new();
    for (ent, body, opt_clim, opt_vol, opt_bio, opt_civ) in query.iter() {
        let is_solid = matches!(
            body.body_type,
            BodyType::TerrestrialPlanet
                | BodyType::SuperEarth
                | BodyType::Protoplanet
                | BodyType::Moon
        );
        if !is_solid {
            continue;
        }
        if opt_civ.is_some() || opt_bio.is_some_and(|b| b.technosignature >= 0.05) {
            colonized.insert(ent);
            continue;
        }
        let temp_score = opt_clim.map_or(0, |c| {
            if (250.0..=335.0).contains(&c.surface_temperature_k) {
                30
            } else {
                5
            }
        });
        let water_score = opt_vol.map_or(0, |v| i32::from(v.ocean_coverage_frac > 0.05) * 20);
        colony_candidates.push((ent, body.name.clone(), temp_score + water_score));
    }
    (colonized, colony_candidates)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Spawns autonomous off-world colony entities"
)]
fn spawn_colony_seeds(
    commands: &mut Commands,
    worlds_q: &mut Query<CivWorldItem>,
    colony_founders: Vec<(Entity, String)>,
    colony_candidates: &[(Entity, String, i32)],
    colonized: &mut FxHashSet<Entity>,
    elapsed_years: f64,
    has_antimatter: bool,
    colony_events: &mut MessageWriter<ColonyFoundedEvent>,
) {
    for (origin_ent, origin_name) in colony_founders {
        let Some((target_ent, target_name)) =
            find_best_colony_target(origin_ent, colony_candidates, colonized)
        else {
            break;
        };
        colonized.insert(target_ent);
        if let Ok(mut cmd) = commands.get_entity(target_ent) {
            let mut colony_civ = CivilizationState::from_technosignature(0.60);
            colony_civ.is_colony = true;
            colony_civ.kardashev_level = 0.85;
            colony_civ.population_billions = 0.25;
            colony_civ.stability = 0.78;
            cmd.try_insert((
                colony_civ,
                BiosphereState {
                    habitability_score: 0.72,
                    biomass_coverage_frac: 0.25,
                    oxygen_fraction: 0.18,
                    emergence_year: Some(elapsed_years),
                    technosignature: technosignature_from_kardashev(0.85),
                    ..default()
                },
            ));
        }
        if let Ok((.., mut opt_civ)) = worlds_q.get_mut(origin_ent)
            && let Some(ref mut origin_civ) = opt_civ
        {
            origin_civ.colonies_founded += 1;
            origin_civ.colony_cooldown_years = if has_antimatter { 350.0 } else { 800.0 };
        }
        colony_events.write(ColonyFoundedEvent {
            origin_entity: origin_ent,
            colony_entity: target_ent,
            origin_name,
            colony_name: target_name,
        });
    }
}

/// Bootstraps and evolves [`CivilizationState`], thermal defenses, and interplanetary colonies.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Civilization evolution bridges climates, biospheres, dyson swarms, tech buffs, and colony events"
)]
pub fn evolve_civilizations_system(
    mut commands: Commands,
    time: Option<Res<Time>>,
    sim_time: Option<Res<SimTime>>,
    time_warp: Option<Res<TimeWarp>>,
    opt_techs: Option<Res<UnlockedTechs>>,
    dyson_query: Query<&DysonSwarm, With<CentralStar>>,
    mut param_set: ParamSet<(Query<CivWorldItem>, Query<ColonyCandidateItem>)>,
    mut milestone_events: MessageWriter<CivilizationMilestoneEvent>,
    mut colony_events: MessageWriter<ColonyFoundedEvent>,
) {
    if time_warp
        .as_deref()
        .is_some_and(|tw| tw.is_paused && !tw.step_once)
    {
        return;
    }
    let dt_yr = sim_time
        .as_deref()
        .map_or(0.05, |s| s.current_dt_yr)
        .max(0.0);
    let dt_vis = time
        .as_deref()
        .map_or(0.016, Time::delta_secs)
        .clamp(0.0, 0.25);
    let buffs = gather_tech_buffs(opt_techs.as_deref(), &dyson_query);
    let (mut colonized, colony_candidates) = collect_colony_candidates(&param_set.p1());

    let mut colony_founders: Vec<(Entity, String)> = Vec::new();
    let colony_threshold = if buffs.has_ascension { 1.10 } else { 1.30 };

    for (ent, body, opt_temp, mut opt_climate, opt_vol, mut opt_terra, mut opt_bio, mut opt_civ) in
        param_set.p0().iter_mut()
    {
        if body.body_type.is_star_or_remnant() {
            continue;
        }
        let stabilizer_secs = opt_civ
            .as_deref()
            .map_or(0.0, |c| c.climate_stabilizer_secs);
        let is_inhabited = opt_civ.is_some() || opt_bio.is_some();
        apply_thermal_stabilization_buffs(
            &mut opt_climate,
            &mut opt_terra,
            buffs.has_mirror_array,
            stabilizer_secs,
            dt_yr,
            is_inhabited,
        );

        let temp_k = opt_climate.as_deref().map_or_else(
            || opt_temp.map_or(288.0, |t| t.0 as f32),
            |c| c.surface_temperature_k,
        );
        let pressure_bar = opt_vol.map_or(1.0, |v| v.atmospheric_pressure_bar);

        let Some(ref mut bio) = opt_bio else {
            continue;
        };
        let Some(ref mut civ) = opt_civ else {
            if bio.technosignature >= 0.05
                && let Ok(mut cmd) = commands.get_entity(ent)
            {
                cmd.try_insert(CivilizationState::from_technosignature(bio.technosignature));
                colonized.insert(ent);
            }
            continue;
        };

        step_civilization_growth(
            ent,
            &body.name,
            civ,
            bio,
            (temp_k, pressure_bar),
            &buffs,
            dt_yr,
            dt_vis,
            &mut milestone_events,
        );

        if civ.kardashev_level >= colony_threshold
            && civ.stability >= 0.60
            && civ.colony_cooldown_years <= 0.0
        {
            colony_founders.push((ent, body.name.clone()));
        }
    }

    let elapsed_years = sim_time.as_deref().map_or(0.0, |s| s.elapsed_years);
    spawn_colony_seeds(
        &mut commands,
        &mut param_set.p0(),
        colony_founders,
        &colony_candidates,
        &mut colonized,
        elapsed_years,
        buffs.has_antimatter,
        &mut colony_events,
    );
}

/// Synchronizes [`ProgressionState`] aggregates and unlocks civilization/defense trophies.
#[allow(
    clippy::too_many_arguments,
    reason = "Aggregates civilization metrics, Dyson progress, and progression trophies"
)]
pub fn sync_progression_and_trophies_system(
    mut opt_progression: Option<ResMut<ProgressionState>>,
    mut opt_trophies: Option<ResMut<GlobalTrophies>>,
    mut opt_science: Option<ResMut<ScienceData>>,
    mut opt_phase: Option<ResMut<PhaseManager>>,
    mut opt_toast: Option<ResMut<NotificationToast>>,
    opt_techs: Option<Res<UnlockedTechs>>,
    sim_time: Option<Res<SimTime>>,
    civ_query: Query<&CivilizationState>,
    dyson_query: Query<&DysonSwarm, With<CentralStar>>,
) {
    let Some(ref mut prog) = opt_progression else {
        return;
    };

    let mut highest_k = 0.0_f32;
    let mut active_civs = 0_u32;
    let mut colonies = 0_u32;
    let mut great_filter_overcome = false;
    for civ in civ_query.iter() {
        if civ.stability > 0.01 {
            active_civs += 1;
            if civ.is_colony {
                colonies += 1;
            }
        }
        highest_k = highest_k.max(civ.kardashev_level);
        if civ.kardashev_level >= 1.05 && civ.stability >= 0.55 && civ.great_filter_stress <= 0.25 {
            great_filter_overcome = true;
        }
    }

    let max_dyson = dyson_query
        .iter()
        .map(|s| s.completion_fraction)
        .fold(0.0_f32, f32::max);
    let stellar_engine = opt_techs
        .as_deref()
        .is_some_and(|t| t.has(TechnologyId::StellarEngine))
        && max_dyson >= 0.35;

    if (prog.highest_kardashev - highest_k).abs() > 1e-4 {
        prog.highest_kardashev = highest_k;
    }
    if prog.active_civilization_count != active_civs {
        prog.active_civilization_count = active_civs;
    }
    if prog.colony_world_count != colonies {
        prog.colony_world_count = colonies;
    }
    if prog.stellar_engine_active != stellar_engine {
        prog.stellar_engine_active = stellar_engine;
    }

    let Some(ref mut trophies) = opt_trophies else {
        return;
    };
    let checks = [
        (
            highest_k >= 1.0,
            MilestoneId::CivilizationTypeOne,
            "🏙️ Type I Planetary Civilization",
            "A civilization mastered planetary energy budgets (K1.00).",
        ),
        (
            max_dyson >= 0.50 || highest_k >= 2.0,
            MilestoneId::DysonSwarmTypeTwo,
            "☀️ Type II Dyson Megastructure",
            "Constructed a 50%+ Dyson Swarm harvesting stellar output.",
        ),
        (
            prog.crises_averted >= 1,
            MilestoneId::CrisisDeflected,
            "🛡️ Cosmic Crisis Averted",
            "Neutralized a system-scale existential crisis before devastation.",
        ),
        (
            colonies >= 1,
            MilestoneId::InterplanetaryColony,
            "🛸 Interplanetary Colony",
            "An advanced civilization established a self-sustaining off-world colony.",
        ),
        (
            great_filter_overcome,
            MilestoneId::GreatFilterOvercome,
            "🕊️ Great Filter Overcome",
            "Navigated industrial/climate fragility beyond K1.05 in stable equilibrium.",
        ),
    ];

    let yr = sim_time.as_deref().map_or(0.0, |s| s.elapsed_years);
    for (cond, id, title, desc) in checks {
        if cond && trophies.unlocked.insert(id) {
            if let Some(ref mut sci) = opt_science {
                sci.record_discovery(title, desc, 150.0, yr);
            }
            if let Some(ref mut pm) = opt_phase {
                pm.latest_unlocked_milestone = Some(title.to_string());
                pm.milestone_toast_timer = 5.0;
            }
            if let Some(ref mut t) = opt_toast {
                t.message = format!("🏆 TROPHY UNLOCKED: {title} (+150 Science Data!)");
                t.timer = 5.0;
            }
        }
    }
}
