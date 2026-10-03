//! Persistent Supernova Remnant (SNR) Nebulae, Sedov-Taylor expansion, and Pulsar Wind Nebulae (PWN).

use bevy::prelude::*;

use crate::rendering::effects::supernova::{SupernovaExplosionInstance, SupernovaType};
use crate::simulation::components::{BodyType, CelestialBody, CentralStar};
use crate::simulation::resources::SimTime;

/// An individual ionized filament or knot in the expanding remnant shell.
#[derive(Debug, Clone)]
pub struct RemnantFilament {
    pub dir: Vec3,
    pub axis: Vec3,
    pub radius_scale: f32,
    pub color: Color,
    pub width: f32,
}

/// A long-lived, diffuse interstellar nebula evolving over deep astronomical timescales.
#[derive(Debug, Clone)]
pub struct PersistentSupernovaRemnant {
    pub star_entity: Entity,
    pub center: Vec3,
    pub remnant_type: SupernovaType,
    pub initial_radius_au: f32,
    pub current_radius_au: f32,
    pub expansion_rate_au_yr: f32,
    pub age_years: f64,
    pub max_age_years: f64,
    pub opacity: f32,
    pub has_pwn: bool,
    pub filaments: Vec<RemnantFilament>,
}

/// Resource pool managing persistent supernova remnants and diffuse nebulae.
#[derive(Resource, Default)]
pub struct PersistentRemnantPool {
    pub remnants: Vec<PersistentSupernovaRemnant>,
}

impl PersistentRemnantPool {
    /// Clears all active remnants (e.g. on scenario change).
    pub fn clear(&mut self) {
        self.remnants.clear();
    }
}

/// Spawns a persistent remnant from a completed prompt supernova explosion.
pub fn spawn_remnant_from_explosion(
    pool: &mut PersistentRemnantPool,
    exp: &SupernovaExplosionInstance,
    has_pulsar_or_magnetar: bool,
) {
    let mut filaments = Vec::with_capacity(36);

    let count = match exp.explosion_type {
        SupernovaType::Hypernova => 42,
        SupernovaType::TypeII => 36,
        SupernovaType::TypeIa => 28,
        SupernovaType::PlanetaryNebula => 24,
    };

    for i in 0..count {
        let fi = i as f32;
        let phi = fi * 2.399_963_2; // Golden ratio spiral
        let y = 1.0 - (fi / (count as f32 - 1.0)) * 2.0;
        let radius_at_y = (1.0 - y * y).max(0.0).sqrt();
        let dir = Vec3::new(radius_at_y * phi.cos(), y, radius_at_y * phi.sin()).normalize();

        let axis =
            Vec3::new((fi * 1.7).sin(), (fi * 2.3).cos(), (fi * 3.1).sin()).normalize_or_zero();

        // Multi-zone chemical emission:
        // - Outer envelope: Balmer H-alpha ruby/crimson
        // - Intermediate mantle: Forbidden [O III] cyan/emerald
        // - Core cavity: Ionized Sulfur [S II] golden-amber
        let color = if i % 3 == 0 {
            Color::srgba(0.95, 0.22, 0.32, 0.75) // H-alpha
        } else if i % 3 == 1 {
            Color::srgba(0.12, 0.95, 0.78, 0.80) // [O III]
        } else {
            Color::srgba(1.0, 0.72, 0.20, 0.70) // [S II]
        };

        filaments.push(RemnantFilament {
            dir,
            axis,
            radius_scale: 0.85 + (fi * 0.43).sin() * 0.15,
            color,
            width: 0.18 + (fi * 0.31).cos().abs() * 0.12,
        });
    }

    let max_age = match exp.explosion_type {
        SupernovaType::Hypernova => 150_000.0,
        SupernovaType::TypeII => 100_000.0,
        SupernovaType::TypeIa => 80_000.0,
        SupernovaType::PlanetaryNebula => 50_000.0,
    };

    pool.remnants.push(PersistentSupernovaRemnant {
        star_entity: exp.star_entity,
        center: exp.center,
        remnant_type: exp.explosion_type,
        initial_radius_au: exp.max_radius_au,
        current_radius_au: exp.max_radius_au,
        expansion_rate_au_yr: match exp.explosion_type {
            SupernovaType::Hypernova => 0.15,
            SupernovaType::TypeII => 0.10,
            SupernovaType::TypeIa => 0.08,
            SupernovaType::PlanetaryNebula => 0.02,
        },
        age_years: 0.0,
        max_age_years: max_age,
        opacity: 0.95,
        has_pwn: has_pulsar_or_magnetar,
        filaments,
    });
}

/// Advances Sedov-Taylor expansion and thermal dissipation for persistent remnants.
pub fn update_persistent_remnants(
    mut remnant_pool: ResMut<PersistentRemnantPool>,
    sim_time: Option<Res<SimTime>>,
    time: Res<Time>,
    star_query: Query<(Entity, &CelestialBody), With<CentralStar>>,
) {
    if remnant_pool.remnants.is_empty() {
        return;
    }

    let dt_sim_yr = sim_time
        .as_deref()
        .map_or(0.0, |st| st.current_dt_yr.max(0.0));
    let dt_visual_s = time.delta_secs();

    // Check central remnant body types
    let opt_star = star_query.iter().next();

    for rem in &mut remnant_pool.remnants {
        // Advance age in simulated astronomical years (or visual baseline if paused/slow)
        let effective_dt_yr = if dt_sim_yr > 0.0 {
            dt_sim_yr
        } else {
            f64::from(dt_visual_s) * 0.5
        };

        rem.age_years += effective_dt_yr;

        // If the central remnant is a Pulsar or Magnetar, activate the Pulsar Wind Nebula
        if let Some((star_ent, star_body)) = opt_star {
            if star_ent == rem.star_entity
                && matches!(star_body.body_type, BodyType::Pulsar | BodyType::Magnetar)
            {
                rem.has_pwn = true;
            }
        }

        // Sedov-Taylor phase expansion: R(t) = R_0 * (1 + t / t_sedov)^0.38
        let time_ratio = (rem.age_years / 400.0).max(0.0);
        let growth_factor = (1.0 + time_ratio).powf(0.38) as f32;
        rem.current_radius_au = rem.initial_radius_au * growth_factor;

        // Slow dissipation and radiative cooling
        let age_frac = (rem.age_years / rem.max_age_years).clamp(0.0, 1.0) as f32;
        let cooling_decay = (-rem.age_years / 35_000.0).exp() as f32;
        rem.opacity = ((1.0 - age_frac) * cooling_decay * 0.95).clamp(0.0, 1.0);
    }

    // Retain active, visible remnants
    remnant_pool
        .remnants
        .retain(|r| r.opacity > 0.005 && r.age_years < r.max_age_years);
}

/// Renders persistent filamentary shells, ionization rings, and Pulsar Wind Nebula synchrotron cores.
pub fn draw_persistent_supernova_remnants(
    gizmos: &mut Gizmos,
    pool: &PersistentRemnantPool,
    visual_time: f32,
) {
    for rem in &pool.remnants {
        let op = rem.opacity;
        if op <= 0.005 {
            continue;
        }

        let r = rem.current_radius_au;

        // 1. Forward Shock Boundary Rings (Multi-planar circles)
        let rim_col = match rem.remnant_type {
            SupernovaType::Hypernova => Color::srgba(0.85, 0.40, 1.0, op * 0.55),
            SupernovaType::TypeII => Color::srgba(0.35, 0.90, 0.95, op * 0.60),
            SupernovaType::TypeIa => Color::srgba(1.0, 0.92, 0.75, op * 0.50),
            SupernovaType::PlanetaryNebula => Color::srgba(0.20, 0.95, 0.70, op * 0.65),
        };

        gizmos.circle(
            Isometry3d::new(
                rem.center,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            r,
            rim_col,
        );
        gizmos.circle(
            Isometry3d::new(
                rem.center,
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ),
            r * 0.98,
            Color::srgba(
                rim_col.to_srgba().red,
                rim_col.to_srgba().green,
                rim_col.to_srgba().blue,
                op * 0.35,
            ),
        );

        // 2. Pulsar Wind Nebula (PWN) Synchrotron Core
        // If a rotating Pulsar or Magnetar is powering the interior (e.g. Crab Nebula)
        if rem.has_pwn {
            let pwn_r = (r * 0.28).max(1.5);
            let pwn_pulse = (visual_time * 2.5).sin() * 0.08 + 0.92;
            let pwn_col = Color::srgba(0.30, 0.85, 1.0, op * 0.75 * pwn_pulse);

            // Polar synchrotron wind torus
            gizmos.circle(
                Isometry3d::new(rem.center, Quat::from_rotation_x(0.35)),
                pwn_r,
                pwn_col,
            );
            gizmos.circle(
                Isometry3d::new(rem.center, Quat::from_rotation_y(0.78)),
                pwn_r * 0.85,
                Color::srgba(0.65, 0.35, 1.0, op * 0.55),
            );
            gizmos.sphere(
                Isometry3d::from_translation(rem.center),
                pwn_r * 0.40,
                Color::srgba(0.90, 0.95, 1.0, op * 0.30),
            );
        }

        // 3. Planetary Nebula Bipolar Lobes
        if rem.remnant_type == SupernovaType::PlanetaryNebula {
            let lobe_r = r * 0.65;
            let lobe_offset = Vec3::new(0.0, lobe_r * 0.55, 0.0);
            let pneb_col = Color::srgba(0.95, 0.25, 0.65, op * 0.45);

            gizmos.circle(
                Isometry3d::new(
                    rem.center + lobe_offset,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                lobe_r * 0.75,
                pneb_col,
            );
            gizmos.circle(
                Isometry3d::new(
                    rem.center - lobe_offset,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                lobe_r * 0.75,
                pneb_col,
            );
        }
    }
}
