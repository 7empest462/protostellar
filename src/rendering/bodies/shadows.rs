//! Real-time planetary ring shadows and moon solar eclipse shadow computations.

use bevy::prelude::*;
use crate::simulation::components::PlanetaryRingSystem;

/// Calculates the planetary shadow factor on a ring fragment (0.0 = full umbra, 1.0 = full sunlight).
pub fn compute_planetary_ring_shadow(
    ring_pos_norm: Vec2,
    star_dir_local: Vec3,
    planet_rad_norm: f32,
) -> f32 {
    let p_ring = Vec3::new(ring_pos_norm.x, 0.0, ring_pos_norm.y);
    let s_closest = -p_ring.dot(star_dir_local);
    if s_closest <= 0.0 {
        return 1.0;
    }
    let p_closest = p_ring + s_closest * star_dir_local;
    let d_closest = p_closest.length();
    let penumbra = 0.022f32;
    let umbra_r = (planet_rad_norm - penumbra).max(0.0);
    let penumbra_r = planet_rad_norm + penumbra;
    if d_closest <= umbra_r {
        0.0
    } else if d_closest >= penumbra_r {
        1.0
    } else {
        let t = (d_closest - umbra_r) / (penumbra_r - umbra_r);
        t * t * (3.0 - 2.0 * t)
    }
}

/// Calculates ring shadow attenuation on a planet surface point (1.0 = unshadowed, 0.0 = total darkness).
pub fn compute_ring_shadow_on_planet(
    surf_norm: Vec3,
    star_dir: Vec3,
    spin_axis: Vec3,
    r_inner: f32,
    r_outer: f32,
    opt_depth: f32,
) -> f32 {
    let l_dot_s = star_dir.dot(spin_axis);
    let p_dot_s = surf_norm.dot(spin_axis);
    if l_dot_s.abs() <= 1e-4 {
        return 1.0;
    }
    let t_ring = -p_dot_s / l_dot_s;
    let n_dot_l = surf_norm.dot(star_dir);
    if t_ring <= 0.0 || n_dot_l <= 0.0 {
        return 1.0;
    }
    let p_int = surf_norm + t_ring * star_dir;
    let r_int = p_int.length();
    if r_int < r_inner || r_int > r_outer {
        return 1.0;
    }
    let u = (r_int - r_inner) / (r_outer - r_inner);
    let ring_density = if u < 0.22 {
        0.10 + 0.25 * (u / 0.22)
    } else if u < 0.65 {
        0.95 // B-Ring
    } else if u < 0.72 {
        let gap_t = (u - 0.65) / (0.72 - 0.65);
        (1.0 - (gap_t * std::f32::consts::PI).sin()) * 0.12 // Cassini division
    } else if u < 0.96 {
        0.75 // A-Ring
    } else {
        (1.0 - (u - 0.96) / 0.04) * 0.35
    };
    let edge_feather =
        ((r_int - r_inner) / 0.03).clamp(0.0, 1.0) * ((r_outer - r_int) / 0.03).clamp(0.0, 1.0);
    let shadow_atten = (ring_density * opt_depth * edge_feather).clamp(0.0, 0.96);
    1.0 - shadow_atten
}

/// Calculates moon solar eclipse illumination on a planet surface point (1.0 = daylight, 0.0 = total umbra).
pub fn compute_moon_eclipse_shadow(
    surf_norm: Vec3,
    star_dir: Vec3,
    moon_rel_pos_norm: Vec3,
    moon_rad_norm: f32,
) -> f32 {
    let v = moon_rel_pos_norm - surf_norm;
    let t_close = v.dot(star_dir);
    if t_close <= 0.0 {
        return 1.0;
    }
    let d_sq = v.length_squared() - t_close * t_close;
    let max_r = moon_rad_norm * 1.45;
    if d_sq >= max_r * max_r {
        return 1.0;
    }
    let d_perp = d_sq.max(0.0).sqrt();
    let umbra_r = moon_rad_norm * 0.70;
    let penumbra_r = moon_rad_norm * 1.25;
    if d_perp <= umbra_r {
        0.04
    } else if d_perp >= penumbra_r {
        1.0
    } else {
        let t = (d_perp - umbra_r) / (penumbra_r - umbra_r);
        let smooth_t = t * t * (3.0 - 2.0 * t);
        0.04 + 0.96 * smooth_t
    }
}

pub fn compute_ring_shadow_params(opt_rings: Option<&PlanetaryRingSystem>, radius_au: f64) -> Vec4 {
    if let Some(ring_sys) = opt_rings {
        let ring_ratio = if ring_sys.outer_radius_au > 0.0 && radius_au > 0.0 {
            (ring_sys.outer_radius_au / radius_au as f32).clamp(2.0, 3.5)
        } else {
            2.85
        };
        let inner_ratio = if ring_sys.outer_radius_au > 0.0 && ring_sys.inner_radius_au > 0.0 {
            (ring_sys.inner_radius_au / radius_au as f32).clamp(1.1, ring_ratio - 0.1)
        } else {
            1.25
        };
        Vec4::new(inner_ratio, ring_ratio, ring_sys.optical_depth, 1.0)
    } else {
        Vec4::ZERO
    }
}

pub fn compute_eclipse_moons_data(
    entity: Entity,
    planet_pos: Vec3,
    visual_radius: f32,
    star_dir: Vec3,
    all_moons: &[(Entity, Vec3, f32, Option<Entity>)],
) -> ([Vec4; 2], [Vec4; 2]) {
    let mut moons_pos = [Vec4::ZERO; 2];
    let mut moons_data = [Vec4::ZERO; 2];
    let mut slot = 0;
    for &(m_ent, m_pos, m_rad, opt_parent) in all_moons {
        if slot >= 2 {
            break;
        }
        if m_ent == entity {
            continue;
        }
        let is_child_moon = opt_parent == Some(entity)
            || (opt_parent.is_none() && (m_pos - planet_pos).length() < visual_radius * 40.0);
        if is_child_moon {
            let d_vec = m_pos - planet_pos;
            let s = d_vec.dot(star_dir);
            if s > 0.0 {
                let d_perp_sq = d_vec.length_squared() - s * s;
                let max_touch_dist = visual_radius + m_rad * 1.5;
                if d_perp_sq < max_touch_dist * max_touch_dist {
                    let norm_pos = d_vec / visual_radius.max(1e-5);
                    let norm_rad = (m_rad / visual_radius.max(1e-5)).clamp(0.01, 1.0);
                    if let (Some(pos_slot), Some(data_slot)) =
                        (moons_pos.get_mut(slot), moons_data.get_mut(slot))
                    {
                        *pos_slot = Vec4::new(norm_pos.x, norm_pos.y, norm_pos.z, norm_rad);
                        *data_slot = Vec4::new(1.0, 0.25, 0.96, 0.0);
                        slot += 1;
                    }
                }
            }
        }
    }
    (moons_pos, moons_data)
}
