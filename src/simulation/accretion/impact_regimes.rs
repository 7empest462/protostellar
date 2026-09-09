//! Impact regime classification (Stewart / Leinhardt-style scaling, game-tuned).
//!
//! Decides whether a close encounter Merges, grazes, disrupts, or (rarely)
//! forms a moon — and forces accretion when the secondary is inside the
//! primary's *visual* radius so bodies cannot freeze in the crust.

use std::f64::consts::PI;

use crate::simulation::components::BodyType;
use crate::utils::constants::*;

/// Outcome of a pairwise encounter at contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactRegime {
    /// Secondary is inside the primary's rendered sphere → must accrete.
    EmbeddedMerge,
    /// Low-speed / head-on → inelastic merger.
    Merger,
    /// High-angle, high-speed → bounce with partial momentum exchange.
    HitAndRun,
    /// Grazing contact, little mass exchange.
    Graze,
    /// Secondary torn into a ring/debris stream (Roche).
    Disrupt,
    /// Classic giant impact: large secondary, glancing → debris disk / moon.
    GiantImpactMoon,
}

/// Inputs for classification (all in simulation units: AU, yr, M_sun).
#[derive(Debug, Clone, Copy)]
pub struct ImpactParams {
    pub primary_mass: f64,
    pub secondary_mass: f64,
    pub primary_radius_au: f64,
    pub secondary_radius_au: f64,
    pub primary_type: BodyType,
    pub secondary_type: BodyType,
    /// Closest approach distance this step (AU).
    pub min_dist: f64,
    /// Impact parameter proxy in [0, 1] (0 = head-on, 1 = barely grazing).
    pub b: f64,
    /// Relative speed (AU/yr).
    pub v_rel: f64,
    /// Mutual escape speed at contact (AU/yr).
    pub v_esc: f64,
}

impl ImpactParams {
    pub fn mass_ratio(&self) -> f64 {
        self.secondary_mass / self.primary_mass.max(1e-30)
    }

    pub fn v_imp_over_vesc(&self) -> f64 {
        self.v_rel / self.v_esc.max(1e-12)
    }
}

/// Estimate visual radius using the same power-law as `SimulationConfig`
/// defaults (size exaggeration 1.0, min floor 0.0006 AU).
pub fn estimate_visual_radius_au(physical_radius_au: f64) -> f64 {
    const REF: f32 = 0.00465;
    const BASE: f32 = 0.025;
    const POWER: f32 = 0.45;
    const MIN: f32 = 0.0006;
    let ratio = (physical_radius_au as f32 / REF).max(1e-6);
    f64::from((BASE * ratio.powf(POWER)).max(MIN))
}

/// Physical radius from mass + density (M_sun, M_sun/AU³ → AU).
pub fn radius_from_mass_density(mass: f64, density: f64) -> f64 {
    ((3.0 * mass / density.max(1e-30)) / (4.0 * PI))
        .cbrt()
        .max(EARTH_RADIUS_AU * 0.05)
}

/// Classify the encounter.
///
/// Rules of thumb (planet-formation literature, simplified for real-time):
/// - Inside ~2.5× visual radius of the primary → **EmbeddedMerge** (no stuck moons).
/// - \(v_\mathrm{imp}/v_\mathrm{esc} \lesssim 1\) and \(b \lesssim 0.7\) → Merger.
/// - High \(b\) + high speed → HitAndRun / Graze.
/// - Deep in Roche lobe + small secondary → Disrupt.
/// - Near-equal mass, glancing, moderate speed → GiantImpactMoon.
pub fn classify_impact(p: ImpactParams, roche_limit_au: f64) -> ImpactRegime {
    let visual_r = estimate_visual_radius_au(p.primary_radius_au);
    let gamma = p.mass_ratio();
    let u = p.v_imp_over_vesc();

    // 1. Cannot sit inside the drawn planet.
    if p.min_dist < visual_r * 2.5 {
        return ImpactRegime::EmbeddedMerge;
    }

    // 2. Roche disruption (small secondary inside fluid Roche limit).
    let can_disrupt = !p.primary_type.is_star_or_remnant()
        && !p.secondary_type.is_star_or_remnant()
        && p.primary_mass >= EARTH_MASS_SOLAR * 0.05
        && gamma <= 0.20
        && p.min_dist <= roche_limit_au
        && p.b >= 0.15;
    if can_disrupt {
        return ImpactRegime::Disrupt;
    }

    // 3. Giant-impact moon (rare, needs substantial secondary + glancing geometry).
    let can_moon = !p.primary_type.is_star_or_remnant()
        && !p.secondary_type.is_star_or_remnant()
        && p.primary_mass >= EARTH_MASS_SOLAR * 0.05
        && (0.05..=0.45).contains(&gamma)
        && p.b >= 0.55
        && u < 2.5;
    if can_moon {
        return ImpactRegime::GiantImpactMoon;
    }

    // 4. Hit-and-run / graze (high angle, energetic).
    if p.b > 0.80 && u > 1.5 {
        return if u > 2.5 {
            ImpactRegime::HitAndRun
        } else {
            ImpactRegime::Graze
        };
    }

    // 5. Default: merge (accretion).
    ImpactRegime::Merger
}
