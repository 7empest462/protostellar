//! Precision orbital intercept and giant impact mechanics for Theia and Proto-Earth.
//!
//! Guides Theia into an oblique collision with Proto-Earth at T ~ 50-100 yr (or on-demand),
//! triggering the Moon-forming giant impact, creating The Moon in a stable orbit,
//! and enriching Earth's lower mantle with Theia LLSVP remnants.

use bevy::math::DVec3;
use bevy::prelude::*;
use std::f64::consts::PI;

use crate::game::ui::NotificationToast;
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::utils::constants::*;

/// Tracks the status and execution of the Theia-Earth Moon-forming giant impact.
#[derive(Resource, Debug, Clone, Default)]
pub struct TheiaImpactState {
    /// Whether trajectory intercept guidance is actively steering Theia toward Earth.
    pub intercept_active: bool,
    /// Whether the Moon-forming collision has resolved and The Moon is in orbit.
    pub moon_formed: bool,
    /// Sim year when intercept guidance was initiated.
    pub intercept_start_year: Option<f64>,
    /// Manual trigger requested by UI action or hotkey [M].
    pub manual_trigger_requested: bool,
    /// Number of simulation updates spent actively steering toward Earth.
    pub intercept_steps: usize,
}

/// Automatically activates intercept at T ~ 50-100 yr or upon on-demand trigger,
/// guiding Theia into an oblique collision with Proto-Earth and forming The Moon.
#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
pub fn update_theia_rendezvous(
    mut commands: Commands,
    sim_time: Res<SimTime>,
    time_warp: Res<TimeWarp>,
    mut theia_state: ResMut<TheiaImpactState>,
    star_query: Query<(&SimPosition, &Mass), With<CentralStar>>,
    mut bodies_query: Query<
        (
            Entity,
            &mut SimPosition,
            &mut SimVelocity,
            &mut Mass,
            &mut Radius,
            &mut CelestialBody,
            Option<&mut InternalDifferentiation>,
            Option<&mut SpinState>,
            Option<&mut SatelliteOf>,
        ),
        Without<CentralStar>,
    >,
    mut toast: Option<ResMut<NotificationToast>>,
) {
    if time_warp.is_paused && !time_warp.step_once {
        return;
    }

    if theia_state.moon_formed {
        return;
    }

    // Check if Moon already formed in previous step or scenario
    let moon_exists = bodies_query
        .iter()
        .any(|(_, _, _, _, _, body, ..)| body.name == "The Moon" || body.name.contains("The Moon"));
    if moon_exists {
        theia_state.moon_formed = true;
        theia_state.intercept_active = false;
        return;
    }

    let elapsed = sim_time.elapsed_years;
    let should_trigger = theia_state.manual_trigger_requested
        || (elapsed >= 50.0 && !theia_state.moon_formed && !theia_state.intercept_active);

    if !should_trigger && !theia_state.intercept_active {
        return;
    }

    if theia_state.manual_trigger_requested {
        theia_state.manual_trigger_requested = false;
        theia_state.intercept_active = true;
        theia_state.intercept_start_year.get_or_insert(elapsed);
        if let Some(ref mut t) = toast {
            t.message =
                "🌑 THEIA MOON COLLISION TRIGGERED // Intercept trajectory locked for Moon formation!"
                    .to_string();
            t.timer = 8.0;
        }
    } else if !theia_state.intercept_active {
        theia_state.intercept_active = true;
        theia_state.intercept_start_year.get_or_insert(elapsed);
        if let Some(ref mut t) = toast {
            t.message =
                "🌑 THEIA INTERCEPT DETECTED // Proto-Earth & Theia entering Moon-forming encounter!"
                    .to_string();
            t.timer = 8.0;
        }
    }

    // 1. Locate or spawn Earth/Proto-Earth
    let earth_opt = bodies_query
        .iter()
        .find(|(_, pos, _, _, _, body, ..)| {
            let name = body.name.as_str();
            let is_earth_name = name == "Earth"
                || name == "Proto-Earth"
                || name.starts_with("Proto-Earth")
                || name.starts_with("Earth (");
            let r_xy = (pos.0.x * pos.0.x + pos.0.z * pos.0.z).sqrt();
            let near_1au = (r_xy - 1.0).abs() < 0.35;
            !name.contains("Theia")
                && !name.contains("Moon")
                && !name.contains("Planet Nine")
                && (is_earth_name
                    || (body.body_type.is_planet() && near_1au && !name.contains("Super-Earth")))
        })
        .map(|(e, ..)| e);

    let earth_ent = if let Some(e) = earth_opt {
        e
    } else {
        let Ok((star_pos, star_mass)) = star_query.single() else {
            return;
        };
        let m_s = 0.88 * EARTH_MASS_SOLAR;
        let rad = EARTH_RADIUS_AU * 0.94;
        let pos = star_pos.0 + DVec3::new(1.0, 0.0, 0.0);
        let v_c = (G_ASTRO * star_mass.0 / 1.0).sqrt();
        let vel = DVec3::new(0.0, 0.0, v_c);
        let comp = Composition::rocky();
        let mut diff = InternalDifferentiation::default();
        diff.recalculate(m_s, rad, &comp);
        commands
            .spawn((
                SimPosition(pos),
                SimVelocity(vel),
                SimAcceleration(DVec3::ZERO),
                Mass(m_s),
                Radius(rad),
                Temperature(288.0),
                comp,
                diff,
                CelestialBody {
                    name: "Proto-Earth".to_string(),
                    body_type: BodyType::Protoplanet,
                },
                VolatileInventory::default(),
                SpinState::default(),
            ))
            .id()
    };

    // 2. Locate or spawn Theia
    let theia_opt = bodies_query
        .iter()
        .find(|(_, _, _, _, _, body, ..)| {
            !body.name.contains("Earth")
                && !body.name.contains("Moon")
                && (body.name == "Theia" || body.name.starts_with("Theia"))
        })
        .map(|(e, ..)| e);

    let theia_ent = if let Some(t) = theia_opt {
        t
    } else {
        let Ok((star_pos, star_mass)) = star_query.single() else {
            return;
        };
        let earth_pos = bodies_query
            .get(earth_ent)
            .map_or(star_pos.0 + DVec3::new(1.0, 0.0, 0.0), |(_, pos, ..)| pos.0);
        let r_e = (earth_pos - star_pos.0).length().max(0.5);
        let phi_e = (earth_pos.z - star_pos.0.z).atan2(earth_pos.x - star_pos.0.x);
        let phi_t = phi_e + 0.12;
        let r_t = r_e * 1.08;
        let pos_t = star_pos.0 + DVec3::new(r_t * phi_t.cos(), 0.0, r_t * phi_t.sin());
        let v_t_mag = (G_ASTRO * star_mass.0 / r_t).sqrt();
        let vel_t = DVec3::new(-v_t_mag * phi_t.sin(), 0.0, v_t_mag * phi_t.cos());
        let m_t = 0.12 * EARTH_MASS_SOLAR;
        let rad_t = EARTH_RADIUS_AU * 0.53;
        let comp = Composition {
            metal_frac: 0.42,
            silicate_frac: 0.58,
            ice_frac: 0.0,
            organics_frac: 0.0,
            gas_frac: 0.0,
        };
        let mut diff = InternalDifferentiation::default();
        diff.recalculate(m_t, rad_t, &comp);
        commands
            .spawn((
                SimPosition(pos_t),
                SimVelocity(vel_t),
                SimAcceleration(DVec3::ZERO),
                Mass(m_t),
                Radius(rad_t),
                Temperature(270.0),
                comp,
                diff,
                CelestialBody {
                    name: "Theia".to_string(),
                    body_type: BodyType::Protoplanet,
                },
                VolatileInventory::default(),
                SpinState::default(),
            ))
            .id()
    };

    if earth_ent == theia_ent {
        return;
    }

    let Ok([mut earth, mut theia]) = bodies_query.get_many_mut([earth_ent, theia_ent]) else {
        return;
    };

    let p_pos = earth.1 .0;
    let p_vel = earth.2 .0;
    let p_mass = earth.3 .0;
    let p_rad = earth.4 .0;

    let s_pos = theia.1 .0;
    let s_vel = theia.2 .0;
    let s_mass = theia.3 .0;
    let s_rad = theia.4 .0;

    let r_contact = (p_rad + s_rad).max(0.0045);
    let rel_pos = s_pos - p_pos;
    let dist = rel_pos.length();
    let rel_vel = s_vel - p_vel;
    let v_rel = rel_vel.length();
    let dt = sim_time.current_dt_yr;

    theia_state.intercept_steps += 1;

    // Trigger condition: within contact envelope or swept past in a large time-step
    let is_impact_imminent =
        dist <= r_contact * 2.0 || (v_rel * dt >= dist * 0.9 && dist <= r_contact * 4.0);

    if is_impact_imminent {
        // Execute Giant Impact resolution:
        theia_state.moon_formed = true;
        theia_state.intercept_active = false;

        // 1. Earth state update
        let total_primary_mass = p_mass + s_mass * 0.897; // ~90% mantle accretes
        earth.3 .0 = total_primary_mass;
        earth.4 .0 = EARTH_RADIUS_AU * 1.0;
        earth.5.name = "Earth".to_string();
        earth.5.body_type = BodyType::TerrestrialPlanet;

        if let Some(ref mut diff) = earth.6 {
            diff.recalculate(total_primary_mass, earth.4 .0, &Composition::rocky());
            diff.has_theia_llsvp = true;
            diff.llsvp_density_contrast = 0.028;
        }
        if let Some(ref mut spin) = earth.7 {
            spin.rotation_period_hours = 6.0;
            spin.axial_tilt_degrees = 23.4;
        }

        // 2. The Moon state update
        let moon_mass = 0.0123 * EARTH_MASS_SOLAR;
        let orbit_dist_au: f64 = 0.0075; // Outside visual mesh & atmosphere (~1,120,000 km)
        let p_moon_yr = 2.0 * PI * (orbit_dist_au.powi(3) / (G_ASTRO * total_primary_mass)).sqrt();
        let v_moon_circ = (G_ASTRO * total_primary_mass / orbit_dist_au).sqrt();

        let tangent = rel_pos.cross(DVec3::Y).normalize_or_zero();
        let mut moon_tangent = if tangent.length_squared() > 0.1 {
            tangent
        } else {
            DVec3::new(0.0, 0.0, 1.0)
        };
        if moon_tangent.dot(p_vel) < 0.0 {
            moon_tangent = -moon_tangent;
        }

        theia.3 .0 = moon_mass;
        theia.4 .0 = EARTH_RADIUS_AU * 0.272;
        let rel_dir = if rel_pos.length_squared() > 1e-12 {
            rel_pos.normalize()
        } else {
            DVec3::new(1.0, 0.0, 0.0)
        };
        theia.1 .0 = p_pos + rel_dir * orbit_dist_au;
        theia.2 .0 = p_vel + moon_tangent * v_moon_circ;
        theia.5.name = "The Moon".to_string();
        theia.5.body_type = BodyType::Moon;

        if let Some(ref mut diff) = theia.6 {
            diff.recalculate(moon_mass, theia.4 .0, &Composition::rocky());
        }

        commands.entity(theia_ent).insert(SatelliteOf {
            parent: earth_ent,
            semi_major_axis_au: orbit_dist_au,
            orbital_period_years: p_moon_yr,
            true_anomaly: 0.0,
        });

        if let Some(ref mut t) = toast {
            t.message = "🌕 GIANT IMPACT MOON FORMATION // Theia collided with Earth! Silicate debris accreted into The Moon.".to_string();
            t.timer = 10.0;
        }
        return;
    }

    // 3. Proportional navigation guidance toward Earth
    let Ok((star_pos, star_mass)) = star_query.single() else {
        return;
    };
    let star_m = star_mass.0.max(0.1);

    if dist > 1.2 {
        // Relocate Theia into an inbound rendezvous orbit ahead of Earth
        let r_e = (p_pos - star_pos.0).length().max(0.5);
        let phi_e = (p_pos.z - star_pos.0.z).atan2(p_pos.x - star_pos.0.x);
        let phi_approach = phi_e + 0.08;
        let r_approach = r_e * 1.04;
        theia.1 .0 = star_pos.0
            + DVec3::new(
                r_approach * phi_approach.cos(),
                0.0,
                r_approach * phi_approach.sin(),
            );
        let v_circ = (G_ASTRO * star_m / r_approach).sqrt();
        theia.2 .0 = DVec3::new(
            -v_circ * phi_approach.sin(),
            0.0,
            v_circ * phi_approach.cos(),
        );
        theia_state.intercept_steps = 0;
        return;
    }

    // Oblique sideswipe offset b ~ 0.35 relative to Earth's orbital plane
    let phi_e = (p_pos.z - star_pos.0.z).atan2(p_pos.x - star_pos.0.x);
    let b_offset = DVec3::new(-phi_e.sin(), 0.0, phi_e.cos()) * (r_contact * 0.35);
    let aim_pos = p_pos + b_offset;

    let to_aim = aim_pos - s_pos;
    let to_aim_dir = to_aim.normalize_or_zero();

    // Active rendezvous trajectory closing step
    let closing_speed = (dist / 0.15).clamp(0.5, 6.0);
    let step_dist = (closing_speed * dt.max(0.01)).min(dist * 0.50);
    theia.1 .0 += to_aim_dir * step_dist;

    // Inward transfer velocity matching Earth's orbital motion plus closing speed
    let mut v_desired = p_vel + to_aim_dir * closing_speed;
    let v_esc_local = (2.0 * G_ASTRO * star_m / s_pos.length().max(0.5)).sqrt();
    if v_desired.length() > v_esc_local * 1.1 {
        v_desired = v_desired.normalize_or_zero() * (v_esc_local * 1.1);
    }
    let blend = (dt / 0.08).clamp(0.1, 0.95);
    theia.2 .0 = theia.2 .0.lerp(v_desired, blend);
}
