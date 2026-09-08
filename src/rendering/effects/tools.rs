use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::resources::PlayerInteractionState;
use crate::utils::constants::G_ASTRO;
use crate::utils::math::*;

pub fn draw_tractor_beam_gizmo(gizmos: &mut Gizmos, player_state: &PlayerInteractionState) {
    if let Some(t_pos) = player_state.tractor_position {
        let t_vec = Vec3::new(t_pos.x as f32, t_pos.y as f32, t_pos.z as f32);
        gizmos.sphere(
            Isometry3d::from_translation(t_vec),
            0.5,
            Color::srgba(0.9, 0.2, 0.8, 0.7),
        );
    }
}

pub fn draw_planet_builder_preview(
    gizmos: &mut Gizmos,
    builder: &crate::game::ui::PlanetBuilderState,
    star_vec: Vec3,
    star_mass_val: f64,
    elapsed: f32,
) {
    if !builder.is_open {
        return;
    }

    let a = builder.semi_major_axis_au;
    let e = builder.eccentricity;
    let pulse = 0.65 + 0.35 * (elapsed * 3.5).sin().abs();

    let preview_elements = OrbitalElements {
        semi_major_axis: a,
        eccentricity: e,
        inclination: 0.0,
        longitude_ascending_node: 0.0,
        argument_of_periapsis: 0.0,
        true_anomaly: f64::from(elapsed * 0.8) % (2.0 * std::f64::consts::PI),
        periapsis: if a > 0.0 { a * (1.0 - e) } else { 1.0 },
        apoapsis: if a > 0.0 && e < 1.0 {
            a * (1.0 + e)
        } else {
            f64::INFINITY
        },
        period_years: if a > 0.0 {
            (a.powi(3) / star_mass_val.max(0.1)).sqrt()
        } else {
            1.0
        },
        specific_energy: -G_ASTRO * star_mass_val / (2.0 * a.max(0.01)),
        periapsis_dir: DVec3::new(1.0, 0.0, 0.0),
        semilatus_dir: DVec3::new(0.0, 0.0, 1.0),
    };

    let preview_points = generate_orbit_points(&preview_elements, 96);
    if preview_points.len() > 1 {
        let prev_col = Color::srgba(0.20, 0.95, 1.0, 0.85 * pulse);
        for window in preview_points.windows(2) {
            if let [p0, p1] = window {
                gizmos.line(*p0 + star_vec, *p1 + star_vec, prev_col);
            }
        }
    }

    if let Some(ghost_pt) =
        position_at_true_anomaly(&preview_elements, preview_elements.true_anomaly)
    {
        let ghost_world = ghost_pt + star_vec;
        gizmos.sphere(
            Isometry3d::from_translation(ghost_world),
            0.06 * pulse,
            Color::srgba(1.0, 0.85, 0.25, 0.95),
        );
        gizmos.circle(
            Isometry3d::new(
                ghost_world,
                Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            ),
            0.12 * pulse,
            Color::srgba(0.3, 0.95, 1.0, 0.70),
        );
    }

    let (opt_peri, opt_apo) = apsides_positions(&preview_elements);
    if let Some(peri) = opt_peri {
        gizmos.sphere(
            Isometry3d::from_translation(peri + star_vec),
            0.035,
            Color::srgba(0.2, 1.0, 0.5, 0.9),
        );
    }
    if let Some(apo) = opt_apo {
        gizmos.sphere(
            Isometry3d::from_translation(apo + star_vec),
            0.035,
            Color::srgba(1.0, 0.5, 0.2, 0.9),
        );
    }
}
