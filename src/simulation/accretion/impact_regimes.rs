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
/// Rules of thumb (Stewart & Leinhardt 2012, Kokubo & Genda 2010):
/// - Small secondary within the fluid Roche limit (outside the core) → **Disrupt** (shreds into planetary rings).
/// - Secondary inside the drawn planet surface (< 1.10x visual radius or core overlap) → **EmbeddedMerge**.
/// - Outer giants with small secondaries outside Roche limit → **GiantImpactMoon** (circumplanetary moon capture).
/// - Large secondary at glancing angle and moderate speed → **GiantImpactMoon** (Theia-style giant impact).
/// - High relative speed (u = v_imp / v_esc > 1.10) with oblique angle (b > 0.35) → **HitAndRun** / **Graze**.
/// - Low speed or head-on encounter → **Merger**.
pub fn classify_impact(p: ImpactParams, roche_limit_au: f64) -> ImpactRegime {
    // Stars and stellar remnants swallow and vaporize all impacting bodies into their convective envelope.
    if p.primary_type.is_star_or_remnant() {
        return ImpactRegime::Merger;
    }

    let gamma = p.mass_ratio();
    let u = p.v_imp_over_vesc();

    // 1. Roche disruption: small minor body or low-mass secondary (gamma <= 0.05)
    // torn apart by tidal shear into planetary rings when within the fluid Roche limit.
    let is_minor_disruptor = matches!(
        p.secondary_type,
        BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
    ) || gamma <= 0.05;

    let can_disrupt = !p.secondary_type.is_star_or_remnant()
        && p.primary_mass >= EARTH_MASS_SOLAR * 0.05
        && is_minor_disruptor
        && p.min_dist <= roche_limit_au
        && p.min_dist >= p.primary_radius_au * 0.5
        && p.b >= 0.08;
    if can_disrupt {
        return ImpactRegime::Disrupt;
    }

    // 2. Moon Formation:
    // a) Outer Gas / Ice Giant Circumplanetary Moon Capture:
    // When a small icy or rocky body approaches a giant planet OUTSIDE its Roche limit
    // and outside its visible surface (min_dist > max(roche, R_primary * 1.15)) with moderate speed (u < 2.5),
    // the giant's deep gravitational well captures it into a stable circumplanetary moon orbit.
    let is_giant_primary = p.primary_type == BodyType::GasGiant
        || p.primary_type == BodyType::IceGiant
        || p.primary_mass >= EARTH_MASS_SOLAR * 2.5;
    let can_giant_capture_moon = is_giant_primary
        && !p.secondary_type.is_star_or_remnant()
        && gamma <= 0.08
        && p.min_dist > roche_limit_au.max(p.primary_radius_au * 1.15)
        && u < 2.5;
    if can_giant_capture_moon {
        return ImpactRegime::GiantImpactMoon;
    }

    // b) Theia-style Giant Impact Moon (near-equal terrestrial embryo mass ratio,
    // low-angle sideswipe to glancing geometry b >= 0.15, encounter velocity u <= 1.35):
    // Recent geochemical/geophysical models (Nature 2023) show a low-angle direct collision
    // thoroughly fuses the planetary mantles while ejecting a silicate debris disk that accretes into the Moon.
    let can_giant_impact_moon = !p.secondary_type.is_star_or_remnant()
        && p.primary_mass >= EARTH_MASS_SOLAR * 0.05
        && (0.05..=0.45).contains(&gamma)
        && p.b >= 0.15
        && u <= 1.35;
    if can_giant_impact_moon {
        return ImpactRegime::GiantImpactMoon;
    }

    // 3. Interpenetration / Deep core plunge:
    // When non-moon encounters penetrate deeply into the primary's interior
    // (min_dist well below physical contact distance) or impact at extreme velocities (u > 1.35),
    // hydrodynamic shear and mutual shock dissipation unconditionally force coalescence into a merged body.
    let physical_contact_dist = p.primary_radius_au + p.secondary_radius_au;
    if p.min_dist < physical_contact_dist * 0.85 || p.min_dist < p.primary_radius_au * 0.95 {
        return ImpactRegime::EmbeddedMerge;
    }

    // 4. Hit-and-run / Graze (Stewart & Leinhardt 2012):
    // Energetic collisions between planetary embryos (similar-sized bodies gamma >= 0.01)
    // with oblique impact angles bounce off without merging, preventing runaway super-Earth formation.
    // Minor bodies (asteroids, comets, gamma < 0.01) cannot bounce off planets; they explosively impact and merge.
    let is_minor_impactor = matches!(
        p.secondary_type,
        BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal | BodyType::DustGrain
    ) || gamma < 0.01;

    if !is_minor_impactor {
        if u > 1.10 && p.b > 0.35 {
            return if u > 2.0 || p.b > 0.65 {
                ImpactRegime::HitAndRun
            } else {
                ImpactRegime::Graze
            };
        }
        if p.b > 0.75 && u > 0.85 {
            return ImpactRegime::Graze;
        }
    }

    // 5. Default: inelastic merger (head-on or low-velocity accretion, and all minor body impacts).
    ImpactRegime::Merger
}
