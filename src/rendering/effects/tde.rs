//! Relativistic Tidal Disruption Event (TDE) plasma streamer simulation and rendering.
//!
//! Models relativistic tidal shearing of stars and planets into infalling spiral
//! plasma ribbons funneled into a black hole's ISCO and accretion disk with
//! canonical Rees fallback luminosity scaling: dM/dt ∝ (1 + t/t_peak)^(-5/3).

use bevy::prelude::*;
use std::f32::consts::TAU;

use crate::simulation::accretion::TidalDisruptionEvent;
use crate::simulation::components::SimPosition;
use crate::simulation::resources::{TidalDisruptionPool, TidalDisruptionStream};

/// System that collects TDE messages and simulates relativistic infall, Keplerian shear,
/// and azimuthal tidal compression of the plasma stream.
pub fn update_tidal_disruption_streams(
    time: Res<Time>,
    mut tde_pool: ResMut<TidalDisruptionPool>,
    mut tde_reader: MessageReader<TidalDisruptionEvent>,
    bodies_query: Query<&SimPosition>,
) {
    let dt = time.delta_secs();

    for ev in tde_reader.read() {
        let n_nodes = 72;
        let mut stream_nodes = Vec::with_capacity(n_nodes);

        let delta = ev.disruption_pos - ev.bh_pos;
        let base_azimuth = delta.z.atan2(delta.x);
        let r_disrupt = delta.xz().length().max(0.005);

        // ISCO radius calculated from Schwarzschild geometry (3 r_s = 6 G M / c^2)
        let r_schwa = (1.974e-8 * ev.bh_mass_solar) as f32;
        let r_isco_calc = (r_schwa * 3.0).max(ev.isco_radius_au as f32);
        let r_max = (r_disrupt * 0.45).max(0.0001);
        let r_isco = r_isco_calc.clamp(0.0001, r_max);
        let r_tidal = r_disrupt.max(r_isco * 1.5);

        for k in 0..n_nodes {
            let s = k as f32 / (n_nodes as f32); // Infall parameter: 0 at outer tail, 1 at ISCO
                                                 // Logarithmic spiral geodesic from r_tidal down to r_isco
            let node_r = r_isco + (r_tidal - r_isco) * (1.0 - s).powf(1.4);
            // Multi-turn relativistic wrap (unwinds ~3.5 turns)
            let node_azimuth = base_azimuth + s * (TAU * 3.5);
            // Tidal vertical compression towards equatorial plane: h(r) ∝ (r / r_t)^(3/4)
            let vert_scale = (node_r / r_tidal).powf(0.75) * 0.015;
            let z_jitter = ((k as f32 * 2.3).sin() * vert_scale) * (1.0 - s * 0.7);
            let intensity = (1.0 - s * 0.3).clamp(0.0, 1.0);

            stream_nodes.push((s, node_r, node_azimuth, z_jitter, intensity));
        }

        tde_pool.streams.push(TidalDisruptionStream {
            bh_entity: ev.bh_entity,
            bh_pos: ev.bh_pos,
            disruption_pos: ev.disruption_pos,
            r_tidal,
            r_isco,
            timer: 0.0,
            max_timer: 6.0,
            initial_mass_solar: ev.body_mass_solar,
            initial_velocity: ev.initial_velocity,
            stream_nodes,
        });
    }

    tde_pool.streams.retain_mut(|stream| {
        stream.timer += dt;

        if let Ok(pos) = bodies_query.get(stream.bh_entity) {
            stream.bh_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }

        // Keplerian relativistic differential shear: inner nodes revolve much faster
        for node in &mut stream.stream_nodes {
            let r = node.1.max(0.002);
            let omega = (2.2 / (r * r * r).sqrt()).clamp(1.5, 35.0);
            node.2 += omega * dt;
            // Radial drift inwards toward ISCO
            let drift_rate = (0.25 / r.sqrt()).clamp(0.05, 0.8) * dt;
            node.1 = (node.1 - drift_rate).max(stream.r_isco * 0.95);
        }

        stream.timer < stream.max_timer
    });
}

/// Renders luminous, relativistic spaghetti plasma ribbons funneled into the black hole.
pub fn draw_tidal_disruption_streams(gizmos: &mut Gizmos, tde_pool: &TidalDisruptionPool) {
    for stream in &tde_pool.streams {
        let p = (stream.timer / stream.max_timer).clamp(0.0, 1.0);
        // Rees fallback rate dM/dt ∝ (1 + 3p)^(-5/3)
        let fallback_luminosity = (1.0 + 3.0 * p).powf(-5.0 / 3.0);
        let fade_envelope = if p < 0.08 {
            p / 0.08
        } else if p > 0.75 {
            (1.0 - p) / 0.25
        } else {
            1.0
        };
        let global_alpha = (fallback_luminosity * fade_envelope).clamp(0.0, 1.0);

        let n = stream.stream_nodes.len();
        for i in 0..n {
            let Some(&(s, r1, phi1, z1, _intensity)) = stream.stream_nodes.get(i) else {
                continue;
            };
            let pos1 = stream.bh_pos + Vec3::new(r1 * phi1.cos(), z1, r1 * phi1.sin());

            // Thermal emission gradient:
            // s = 1 (ISCO plunge) -> cyan-white / ultra-relativistic plasma (T > 10^6 K)
            // s = 0.5 (inner spiral) -> fiery solar gold (T ~ 50,000 K)
            // s = 0 (outer tail) -> deep crimson / amber (T ~ 10,000 K)
            let color1 = if s > 0.75 {
                let t_frac = (s - 0.75) / 0.25;
                Color::srgba(
                    0.85 + 0.15 * t_frac,
                    0.95 + 0.05 * t_frac,
                    1.00,
                    0.95 * global_alpha,
                )
            } else if s > 0.35 {
                let t_frac = (s - 0.35) / 0.40;
                Color::srgba(
                    1.00,
                    0.65 + 0.30 * t_frac,
                    0.20 + 0.70 * t_frac,
                    0.90 * global_alpha,
                )
            } else {
                let t_frac = s / 0.35;
                Color::srgba(
                    1.00,
                    0.25 + 0.40 * t_frac,
                    0.10 + 0.10 * t_frac,
                    0.80 * global_alpha,
                )
            };

            // High-density plasma knot beads along the stream
            if i % 3 == 0 {
                let base_scale = (stream.r_isco * 0.04).clamp(0.004, 0.05);
                let bead_radius = (base_scale + 0.012 * (1.0 - s) * (1.0 - p * 0.5)).max(0.003);
                gizmos.sphere(Isometry3d::from_translation(pos1), bead_radius, color1);
            }

            // Connect continuous plasma filament to adjacent node
            if let Some(&(_, r2, phi2, z2, _)) = stream.stream_nodes.get(i + 1) {
                let pos2 = stream.bh_pos + Vec3::new(r2 * phi2.cos(), z2, r2 * phi2.sin());
                gizmos.line(pos1, pos2, color1);
            }
        }
    }
}
