//! Kilonova detonations, relativistic Gamma-Ray Burst (GRB) jets, and Binary Black Hole GW ringdown.

use bevy::prelude::*;

use crate::simulation::components::BodyType;
use crate::simulation::relativity::GravitationalWaveMergerEvent;

/// An active kilonova detonation or binary black hole ringdown event instance.
#[derive(Debug, Clone)]
pub struct KilonovaInstance {
    pub primary_entity: Entity,
    pub companion_entity: Entity,
    pub position: Vec3,
    pub timer: f32,
    pub max_timer: f32,
    pub current_radius_au: f32,
    pub max_radius_au: f32,
    pub ejecta_speed_au_s: f32,
    pub is_ns_merger: bool,
    pub is_bbh_merger: bool,
    pub peak_gw_frequency_hz: f64,
    pub radiated_gw_mass_solar: f64,
    pub remnant_type: BodyType,
    pub flash_intensity: f32,
}

/// Resource pool managing active kilonova fireballs and GW merger ringdown effects.
#[derive(Resource, Default)]
pub struct KilonovaPool {
    pub instances: Vec<KilonovaInstance>,
}

impl KilonovaPool {
    /// Helper to clear active events on scenario reset.
    pub fn clear(&mut self) {
        self.instances.clear();
    }
}

/// Ingests `GravitationalWaveMergerEvent`s and advances active kilonova fireballs and GRB jets.
pub fn update_kilonova_effects(
    mut pool: ResMut<KilonovaPool>,
    mut merger_events: MessageReader<GravitationalWaveMergerEvent>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();

    // 1. Ingest newly triggered merger events
    for ev in merger_events.read() {
        let pos = Vec3::new(
            ev.position.x as f32,
            ev.position.y as f32,
            ev.position.z as f32,
        );

        let is_bbh = ev.remnant_type == BodyType::BlackHole && ev.radiated_gw_mass_solar > 0.20;
        let is_ns = !is_bbh;

        let max_r = if is_ns { 160.0 } else { 220.0 };
        let speed = if is_ns { 45.0 } else { 65.0 };
        let max_t = if is_ns { 8.5 } else { 6.0 };

        pool.instances.push(KilonovaInstance {
            primary_entity: ev.primary_entity,
            companion_entity: ev.companion_entity,
            position: pos,
            timer: 0.0,
            max_timer: max_t,
            current_radius_au: 0.5,
            max_radius_au: max_r,
            ejecta_speed_au_s: speed,
            is_ns_merger: is_ns,
            is_bbh_merger: is_bbh,
            peak_gw_frequency_hz: ev.peak_gw_frequency_hz,
            radiated_gw_mass_solar: ev.radiated_gw_mass_solar,
            remnant_type: ev.remnant_type,
            flash_intensity: 1.0,
        });
    }

    // 2. Advance kinematics
    for inst in &mut pool.instances {
        inst.timer += dt;
        let p = (inst.timer / inst.max_timer).clamp(0.0, 1.0);

        // Prompt relativistic flash decay
        inst.flash_intensity = (-4.0 * inst.timer).exp();

        // Expanding blast wave (r ~ t^0.7)
        let blast_progress = p.powf(0.70);
        inst.current_radius_au = 0.5 + (inst.max_radius_au - 0.5) * blast_progress;
    }

    // 3. Retain active instances
    pool.instances.retain(|inst| inst.timer < inst.max_timer);
}

/// Renders relativistic short-GRB polar jets, lanthanide r-process fireballs, and GW ringdown ripples.
pub fn draw_kilonova_effects(gizmos: &mut Gizmos, pool: &KilonovaPool) {
    for inst in &pool.instances {
        let p = (inst.timer / inst.max_timer).clamp(0.0, 1.0);
        let alpha = (1.0 - p).clamp(0.0, 1.0);
        let r = inst.current_radius_au;

        if inst.is_ns_merger {
            // =========================================================================
            // KILONOVA DETONATION (NS-NS / NS-BH Merger)
            // =========================================================================

            // 1. Prompt Ultra-High Energy Flash
            if inst.flash_intensity > 0.01 {
                let flash_r = (inst.timer * 6.0).min(3.5);
                gizmos.sphere(
                    Isometry3d::from_translation(inst.position),
                    flash_r,
                    Color::srgba(1.0, 1.0, 1.0, inst.flash_intensity * 0.95),
                );
                gizmos.sphere(
                    Isometry3d::from_translation(inst.position),
                    flash_r * 2.0,
                    Color::srgba(0.85, 0.45, 1.0, inst.flash_intensity * 0.65),
                );
            }

            // 2. Dual Collimated Short-GRB Relativistic Beaming Jets (Polar Axis ±Y)
            let jet_len = (r * 1.35).min(200.0);
            let jet_col = Color::srgba(0.65, 0.35, 1.0, alpha * 0.90);
            let tip_pos_north = inst.position + Vec3::new(0.0, jet_len, 0.0);
            let tip_pos_south = inst.position - Vec3::new(0.0, jet_len, 0.0);

            gizmos.line(inst.position, tip_pos_north, jet_col);
            gizmos.line(inst.position, tip_pos_south, jet_col);

            // Polar jet breakout caps
            let cap_r = (jet_len * 0.12).max(1.0);
            gizmos.circle(
                Isometry3d::new(
                    tip_pos_north,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                cap_r,
                Color::srgba(0.80, 0.50, 1.0, alpha * 0.75),
            );
            gizmos.circle(
                Isometry3d::new(
                    tip_pos_south,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                cap_r,
                Color::srgba(0.80, 0.50, 1.0, alpha * 0.75),
            );

            // 3. Radioactive Lanthanide-Rich r-Process Ejecta Fireball
            // Deep ruby-crimson and golden-amber optical emission (gold and platinum synthesis)
            let core_r = r * 0.45;
            gizmos.sphere(
                Isometry3d::from_translation(inst.position),
                core_r,
                Color::srgba(0.95, 0.18, 0.28, alpha * 0.60),
            );
            gizmos.sphere(
                Isometry3d::from_translation(inst.position),
                core_r * 0.65,
                Color::srgba(1.0, 0.75, 0.25, alpha * 0.70),
            );

            // 4. Expanding Relativistic Ejecta Shock Rings
            gizmos.circle(
                Isometry3d::new(
                    inst.position,
                    Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                ),
                r,
                Color::srgba(0.95, 0.25, 0.35, alpha * 0.80),
            );
            gizmos.circle(
                Isometry3d::new(
                    inst.position,
                    Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                ),
                r * 0.92,
                Color::srgba(1.0, 0.65, 0.20, alpha * 0.55),
            );
        } else {
            // =========================================================================
            // BINARY BLACK HOLE MERGER (Pure Gravitational Wave Ringdown)
            // =========================================================================

            // 1. Spacetime Curvature Chirp Ripples expanding at the speed of light
            for ring_idx in 0..4 {
                let ring_r = (r - (ring_idx as f32 * 18.0)).max(0.0);
                if ring_r > 0.0 {
                    let ring_alpha = alpha * (1.0 - (ring_idx as f32 * 0.22)).max(0.0);
                    let gw_col = Color::srgba(0.35, 0.85, 1.0, ring_alpha * 0.85);

                    gizmos.circle(
                        Isometry3d::new(
                            inst.position,
                            Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
                        ),
                        ring_r,
                        gw_col,
                    );
                    gizmos.circle(
                        Isometry3d::new(inst.position, Quat::from_rotation_y(0.785)),
                        ring_r * 0.95,
                        Color::srgba(0.75, 0.45, 1.0, ring_alpha * 0.50),
                    );
                }
            }

            // 2. Central Quasi-Normal Mode (QNM) Singularity Ringdown Shimmer
            let qnm_pulse = (inst.timer * 18.0).sin() * 0.15 + 0.85;
            let qnm_r = (1.5 * qnm_pulse).max(0.2);
            gizmos.sphere(
                Isometry3d::from_translation(inst.position),
                qnm_r,
                Color::srgba(0.20, 0.70, 1.0, alpha * 0.75),
            );
        }
    }
}
