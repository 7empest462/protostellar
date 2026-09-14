//! Late Heavy Bombardment cascade and volatile delivery impactor spawner.

use bevy::math::DVec3;
use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

fn count_active_crossers(
    query: &Query<(Entity, &SimPosition, &SimVelocity, &CelestialBody, &Mass)>,
    star_pos: DVec3,
    star_m: f64,
) -> (usize, usize) {
    let mut active_crossers = 0;
    let mut total_bodies = 0;

    for (_, pos, vel, body, _) in query.iter() {
        total_bodies += 1;
        if matches!(
            body.body_type,
            BodyType::Asteroid | BodyType::Comet | BodyType::Planetesimal
        ) {
            let r_vec = pos.0 - star_pos;
            let r = (r_vec.x * r_vec.x + r_vec.z * r_vec.z).sqrt();
            let v_sq = vel.0.length_squared();
            let specific_e = 0.5 * v_sq - (G_ASTRO * star_m) / r.max(0.01);
            if specific_e < 0.0 {
                let a = -(G_ASTRO * star_m) / (2.0 * specific_e);
                let h_vec = r_vec.cross(vel.0);
                let h = h_vec.length();
                let e_sq = (1.0 - (h * h) / (G_ASTRO * star_m * a)).max(0.0);
                let e = e_sq.sqrt();
                let q = a * (1.0 - e);
                if q <= 1.6 && r <= 6.0 {
                    active_crossers += 1;
                }
            } else if r <= 2.5 {
                active_crossers += 1;
            }
        }
    }

    (active_crossers, total_bodies)
}

fn spawn_lhb_impactor(
    commands: &mut Commands,
    disk_params: &DiskParameters,
    star_pos: DVec3,
    star_m: f64,
    c_idx: usize,
    target_earth: Option<(DVec3, DVec3)>,
) {
    let is_comet = (c_idx % 3) != 1;
    let (r_spawn, q_target, mass_solar, rad_au, comp, body_type, name) = if is_comet {
        let r_s = 6.0 + ((c_idx * 17) % 50) as f64 * 0.05;
        let q_t = 0.98 + ((c_idx * 7) % 10) as f64 * 0.004;
        let m_s = 0.000_25 * EARTH_MASS_SOLAR;
        let r_au = EARTH_RADIUS_AU * 0.06;
        let mut comp = Composition::icy();
        comp.ice_frac = 0.55;
        (
            r_s,
            q_t,
            m_s,
            r_au,
            comp,
            BodyType::Comet,
            format!("LHB-Comet C/{}", 1900 + (c_idx % 1000)),
        )
    } else {
        let r_s = 2.4 + ((c_idx * 23) % 40) as f64 * 0.02;
        let q_t = 0.98 + ((c_idx * 11) % 10) as f64 * 0.004;
        let m_s = 0.000_25 * EARTH_MASS_SOLAR;
        let r_au = EARTH_RADIUS_AU * 0.08;
        let mut comp = Composition::carbonaceous();
        comp.ice_frac = 0.30;
        (
            r_s,
            q_t,
            m_s,
            r_au,
            comp,
            BodyType::Asteroid,
            format!("LHB-Asteroid ({})", 10000 + (c_idx % 90000)),
        )
    };

    let (pos, vel) = if let Some((e_pos, _e_vel)) = target_earth {
        let r_e = (e_pos.x * e_pos.x + e_pos.z * e_pos.z).sqrt().max(0.5);
        let phi_e = e_pos.z.atan2(e_pos.x);
        let omega_e = (G_ASTRO * star_m / (r_e * r_e * r_e)).sqrt();
        let q = q_target.min(r_e * 1.005).max(r_e * 0.995);
        let a = f64::midpoint(r_spawn, q);
        let t_flight = std::f64::consts::PI * (a * a * a / (G_ASTRO * star_m)).sqrt();
        let delta_theta = omega_e * t_flight;
        let target_theta = phi_e + delta_theta;
        let spawn_angle = target_theta + std::f64::consts::PI;
        let inc_y = (((c_idx * 13) % 7) as f64 - 3.0) * 0.0003;

        let p = star_pos
            + DVec3::new(
                r_spawn * spawn_angle.cos(),
                inc_y,
                r_spawn * spawn_angle.sin(),
            );
        let v_apo = (G_ASTRO * star_m * (2.0 * q) / (r_spawn * (r_spawn + q))).sqrt();
        let u_tan = DVec3::new(-spawn_angle.sin(), 0.0, spawn_angle.cos());
        let v = u_tan * v_apo;
        (p, v)
    } else {
        let angle = ((c_idx * 137) % 360) as f64 * std::f64::consts::PI / 180.0;
        let inc_angle = (((c_idx * 29) % 20) as f64 - 10.0) * 0.005;
        let p = star_pos
            + DVec3::new(
                r_spawn * angle.cos(),
                r_spawn * inc_angle,
                r_spawn * angle.sin(),
            );
        let v_tangential =
            (G_ASTRO * star_m * (2.0 * q_target) / (r_spawn * (r_spawn + q_target))).sqrt();
        let v_inward = -v_tangential * 0.15;
        let u_tan = DVec3::new(-angle.sin(), 0.0, angle.cos());
        let u_rad = DVec3::new(angle.cos(), inc_angle, angle.sin()).normalize_or_zero();
        let v = u_tan * v_tangential + u_rad * v_inward;
        (p, v)
    };

    let mut diff = InternalDifferentiation::default();
    diff.recalculate(mass_solar, rad_au, &comp);

    let temp_k = disk_params.reference_temp_1au * (r_spawn.max(0.1)).powf(-0.5);

    commands.spawn((
        SimPosition(pos),
        SimVelocity(vel),
        SimAcceleration(DVec3::ZERO),
        Mass(mass_solar),
        Radius(rad_au),
        Temperature(temp_k),
        comp,
        diff,
        CelestialBody { body_type, name },
        VolatileInventory {
            delivered_water_m_earth: 0.0,
            cometary_impact_count: 0,
            ocean_coverage_frac: 0.0,
            atmospheric_pressure_bar: 0.0,
        },
        SpinState::default(),
    ));
}

/// Maintains a steady cascade of planet-crossing impactors (asteroids & comets) during the Late Heavy Bombardment epoch.
pub fn update_late_heavy_bombardment_cascade(
    mut commands: Commands,
    time_warp: Res<TimeWarp>,
    sim_time: Res<SimTime>,
    disk_params: Res<DiskParameters>,
    mut lhb_state: ResMut<crate::game::phases::LateHeavyBombardmentState>,
    query: Query<(Entity, &SimPosition, &SimVelocity, &CelestialBody, &Mass)>,
    star_query: Query<(&SimPosition, &Mass), With<CentralStar>>,
    mut cascade_timer: Local<f64>,
    mut cascade_counter: Local<usize>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    if !lhb_state.is_active || lhb_state.migration_progress >= 0.95 {
        return;
    }

    let Ok((star_pos, star_mass)) = star_query.single() else {
        return;
    };

    let dt = sim_time.current_dt_yr;
    *cascade_timer -= dt;

    let star_m = star_mass.0.max(0.1);
    let (active_crossers, _) = count_active_crossers(&query, star_pos.0, star_m);

    let target_earth = query
        .iter()
        .find(|(_, pos, _, body, _)| {
            body.name.contains("Earth")
                || (body.body_type.is_planet()
                    && ((pos.0 - star_pos.0).length() - 1.0).abs() < 0.35)
        })
        .map(|(_, pos, vel, _, _)| (pos.0 - star_pos.0, vel.0));

    let target_crossers = 12;
    if active_crossers < target_crossers && *cascade_timer <= 0.0 {
        *cascade_counter += 1;
        let c_idx = *cascade_counter;

        *cascade_timer = if active_crossers < 5 { 0.35 } else { 2.0 };

        spawn_lhb_impactor(
            &mut commands,
            &disk_params,
            star_pos.0,
            star_m,
            c_idx,
            target_earth,
        );
        lhb_state.comets_scattered = (lhb_state.comets_scattered + 1).min(100_000);
    }
}
