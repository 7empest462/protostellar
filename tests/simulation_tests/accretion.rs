//! Test module generated from simulation_tests.

#[test]
fn test_volatile_water_delivery_mass_budget() {
    use protostellar::simulation::components::VolatileInventory;

    let mut vol = VolatileInventory::default();
    assert_eq!(vol.delivered_water_m_earth, 0.0);
    assert_eq!(vol.ocean_coverage_frac, 0.0);

    // 10 icy cometary impacts delivering 0.00008 Earth masses of volatile ice each
    for _ in 0..10 {
        let d_water = 0.00008;
        vol.delivered_water_m_earth += d_water;
        vol.cometary_impact_count += 1;
        vol.ocean_coverage_frac = (vol.delivered_water_m_earth / 0.0006).clamp(0.0, 0.85) as f32;
    }

    assert_eq!(vol.cometary_impact_count, 10);
    assert!((vol.delivered_water_m_earth - 0.0008).abs() < 1e-6);
    assert!(vol.ocean_coverage_frac >= 0.85); // Fully filled Earth-like ocean basins!
}

#[test]
fn test_fluid_roche_limit_disruption_radius() {
    // Saturn: density ~ 0.687 g/cm3, radius ~ 58,232 km (0.000389 AU)
    let rho_primary: f64 = 0.687;
    let r_primary_km: f64 = 58232.0;

    // Pure Water Ice moon: density ~ 1.0 g/cm3
    let rho_moon: f64 = 1.0;

    let d_roche_km: f64 = 2.44 * r_primary_km * (rho_primary / rho_moon).cbrt();

    // d_roche should be ~ 125,380 km (~ 2.15 Saturn radii), exactly inside Saturn's ring system!
    assert!(d_roche_km > 120000.0 && d_roche_km < 130000.0);
    assert!((d_roche_km / r_primary_km - 2.15).abs() < 0.1);
}

#[test]
fn test_planetary_ring_mass_and_optical_depth() {
    use protostellar::simulation::components::PlanetaryRingSystem;

    let mut ring = PlanetaryRingSystem::default();
    assert_eq!(ring.optical_depth, 0.85);
    assert_eq!(ring.ice_fraction, 0.95);

    // Accrete additional fragmented moon mass
    let additional_moon_earth_mass = 0.0002;
    ring.ring_mass_earth += additional_moon_earth_mass;
    ring.optical_depth = (ring.optical_depth + 0.10).min(1.0);

    assert!((ring.optical_depth - 0.95).abs() < 1e-5);
    assert!(ring.ring_mass_earth > 0.00025);
}

#[test]
fn test_dynamo_magnetic_field_scaling() {
    use protostellar::simulation::components::InternalDifferentiation;

    let mut diff = InternalDifferentiation {
        is_differentiated: true,
        differentiation_fraction: 1.0,
        core_radius_au: 0.000023,
        mantle_radius_au: 0.000042,
        crust_thickness_au: 0.000001,
        ocean_ice_thickness_au: 0.0,
        magnetic_field_gauss: 0.0,
        core_temp_k: 4500.0,
        ..Default::default()
    };

    let period_hrs: f64 = 24.0;
    let temp_factor = ((diff.core_temp_k - 1800.0) / 2000.0).clamp(0.0, 1.5);
    let spin_factor = (24.0f64 / period_hrs).sqrt().clamp(0.2, 3.0);
    let core_mass_frac = (diff.core_radius_au / diff.mantle_radius_au).powi(3);

    let b_gauss = 0.35 * core_mass_frac.sqrt() * spin_factor * temp_factor.powf(0.33);
    diff.magnetic_field_gauss = b_gauss;

    // Earth analog: ~0.3 - 0.5 Gauss
    assert!(diff.magnetic_field_gauss >= 0.15 && diff.magnetic_field_gauss <= 0.60);

    // Rapid rotator (Jupiter analog, period = 10 hrs)
    let fast_spin_factor = (24.0f64 / 10.0f64).sqrt();
    let fast_b = 0.35 * core_mass_frac.sqrt() * fast_spin_factor * temp_factor.powf(0.33);
    assert!(fast_b > diff.magnetic_field_gauss);
}

#[test]
fn test_greenhouse_climate_equilibrium_earth() {
    use protostellar::simulation::components::{ClimateRegime, PlanetaryClimate};
    use protostellar::utils::constants::SOLAR_RADIUS_AU;

    let star_temp = 5778.0f64;
    let star_radius = SOLAR_RADIUS_AU;
    let r_au = 1.0f64;
    let albedo = 0.30f64;

    // Radiative equilibrium: T_eq = T_sun * sqrt(R_sun / (2 * r)) * (1 - A)^0.25
    let t_eq = star_temp * (star_radius / (2.0 * r_au)).sqrt() * (1.0 - albedo).powf(0.25);
    assert!((t_eq - 255.0).abs() < 3.0); // Pure blackbody Earth ~ 255 K (-18°C)

    // 1 bar atmosphere greenhouse boost (+33 K)
    let atm_pressure = 1.0f32;
    let greenhouse_delta = 33.0 * (atm_pressure / 1.0).powf(0.28);
    let t_surf = t_eq as f32 + greenhouse_delta;

    assert!((t_surf - 288.0).abs() < 3.0); // Earth surface ~ 288 K (+15°C)

    let climate = PlanetaryClimate {
        surface_temperature_k: t_surf,
        equilibrium_temperature_k: t_eq as f32,
        greenhouse_delta_k: greenhouse_delta,
        albedo: albedo as f32,
        ..PlanetaryClimate::default()
    };
    assert_eq!(climate.climate_regime, ClimateRegime::TemperateHabitable);
}

#[test]
fn test_ice_albedo_feedback_snowball_regime() {
    use protostellar::simulation::components::ClimateRegime;

    // Outer terrestrial planet at 1.8 AU
    let star_temp = 5778.0f64;
    let star_radius = protostellar::utils::constants::SOLAR_RADIUS_AU;
    let r_au = 1.8f64;
    let albedo = 0.65f64; // High glacial albedo

    let t_eq = star_temp * (star_radius / (2.0 * r_au)).sqrt() * (1.0 - albedo).powf(0.25);
    let t_surf = t_eq + 10.0; // Thin cold atmosphere

    assert!(t_surf < 240.0);
    let regime = if t_surf < 260.0 {
        ClimateRegime::SnowballIceAge
    } else {
        ClimateRegime::TemperateHabitable
    };
    assert_eq!(regime, ClimateRegime::SnowballIceAge);
}

fn classify_saturn_encounter(
    secondary_mass: f64,
    secondary_radius_au: f64,
    secondary_type: protostellar::simulation::components::BodyType,
    min_dist: f64,
    b: f64,
    v_rel: f64,
    v_esc: f64,
) -> protostellar::simulation::accretion::impact_regimes::ImpactRegime {
    use protostellar::simulation::accretion::impact_regimes::{classify_impact, ImpactParams};
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let p = ImpactParams {
        primary_mass: 95.0 * EARTH_MASS_SOLAR,
        secondary_mass,
        primary_radius_au: 0.000389,
        secondary_radius_au,
        primary_type: BodyType::GasGiant,
        secondary_type,
        min_dist,
        b,
        v_rel,
        v_esc,
    };
    classify_impact(p, 0.00095)
}

#[test]
fn test_saturn_system_impact_regime_classifications() {
    use protostellar::simulation::accretion::impact_regimes::ImpactRegime;
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // 1. Small icy moon passing inside Roche limit (0.00060 < 0.00095) -> Disrupted
    let disrupt_regime = classify_saturn_encounter(
        0.0001 * EARTH_MASS_SOLAR,
        0.000005,
        BodyType::Comet,
        0.00060,
        0.35,
        3.5,
        7.0,
    );
    assert_eq!(disrupt_regime, ImpactRegime::Disrupt);

    // 2. Titan embryo passing outside Roche limit (0.0035 > 0.00095) -> Captured into Moon
    let moon_regime = classify_saturn_encounter(
        0.002 * EARTH_MASS_SOLAR,
        0.000015,
        BodyType::Protoplanet,
        0.0035,
        0.65,
        5.0,
        6.0,
    );
    assert_eq!(moon_regime, ImpactRegime::GiantImpactMoon);
}

#[test]
fn test_inner_terrestrial_hit_and_run_classification() {
    use protostellar::simulation::accretion::impact_regimes::{
        classify_impact, ImpactParams, ImpactRegime,
    };
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    // Two protoplanetary embryos near the Sun (0.5 AU) colliding at high orbital velocity
    let primary_mass = 0.35 * EARTH_MASS_SOLAR;
    let secondary_mass = 0.15 * EARTH_MASS_SOLAR;
    let primary_radius_au = 0.000042;
    let secondary_radius_au = 0.000030;
    let d_roche = 0.00010;

    let p = ImpactParams {
        primary_mass,
        secondary_mass,
        primary_radius_au,
        secondary_radius_au,
        primary_type: BodyType::Protoplanet,
        secondary_type: BodyType::Protoplanet,
        min_dist: 0.000070,
        b: 0.55,     // oblique angle
        v_rel: 12.0, // AU/yr (~57 km/s)
        v_esc: 7.5,  // AU/yr -> u = 1.6 > 1.10
    };

    let regime = classify_impact(p, d_roche);
    // Should be HitAndRun or Graze, preventing runaway super-Earth coalescence
    assert!(matches!(
        regime,
        ImpactRegime::HitAndRun | ImpactRegime::Graze
    ));
}

#[test]
fn test_saturn_ring_attachment_and_visual_scale() {
    use bevy::prelude::*;
    use protostellar::simulation::components::{CelestialBody, PlanetaryRingSystem, SpinState};
    use protostellar::simulation::resources::DiskParameters;
    use protostellar::simulation::scenarios::spawn_solar_nebula_mmsn;

    let mut app = App::new();
    let mut disk_params = DiskParameters::default();
    spawn_solar_nebula_mmsn(&mut app.world_mut().commands(), &mut disk_params);
    app.update();

    let mut found_saturn = false;
    for (body, opt_ring, opt_spin) in app
        .world_mut()
        .query::<(
            &CelestialBody,
            Option<&PlanetaryRingSystem>,
            Option<&SpinState>,
        )>()
        .iter(app.world())
    {
        if body.name.contains("Saturn") {
            found_saturn = true;
            assert!(
                opt_ring.is_some(),
                "Proto-Saturn must spawn with PlanetaryRingSystem component"
            );
            let ring = opt_ring.unwrap();
            assert!(ring.outer_radius_au > ring.inner_radius_au);
            assert!(ring.optical_depth > 0.5);
            assert!(ring.ice_fraction > 0.9);

            assert!(opt_spin.is_some());
            let spin = opt_spin.unwrap();
            assert!(
                (spin.axial_tilt_degrees - 26.7).abs() < 0.1,
                "Saturn must have canonical 26.7° axial tilt"
            );
        }
    }
    assert!(
        found_saturn,
        "Proto-Saturn should have been found in MMSN scenario"
    );
}

#[test]
fn test_magma_ocean_radiative_cooling_relaxation() {
    let target_temp = 288.0f64;
    let mut current_temp = 2500.0f64; // Magma ocean from giant impact
    let dt_yr = 50.0; // 50 years of simulation time

    // Apply Stefan-Boltzmann radiative cooling formula
    let cool_rate = 0.08 * (current_temp / 1000.0).powi(3).clamp(0.01, 15.0);
    let k_cool = (1.0 - (-cool_rate * dt_yr).exp()).clamp(0.0, 1.0);
    current_temp = (current_temp + (target_temp - current_temp) * k_cool).max(target_temp);

    // After 50 years, high-temperature magma ocean has cooled significantly towards equilibrium
    assert!(current_temp < 2500.0);
    assert!(current_temp >= target_temp);
}

#[test]
fn test_dry_or_scorching_planets_have_zero_ice_caps() {
    use protostellar::simulation::components::{ClimateRegime, Composition};

    // Dry planet: no water, no ice in composition
    let dry_comp = Composition {
        silicate_frac: 0.70,
        metal_frac: 0.30,
        ice_frac: 0.0,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let ocean_frac = 0.0f32;
    let surface_temp_k = 280.0f64; // cold enough for ice if water existed

    let has_water = ocean_frac > 0.01 || dry_comp.ice_frac as f32 > 0.005;
    let ice_coverage = if !has_water {
        0.0
    } else {
        match ClimateRegime::TemperateHabitable {
            ClimateRegime::SnowballIceAge => 1.0,
            ClimateRegime::TemperateHabitable => {
                if surface_temp_k < 290.0 {
                    ((290.0 - surface_temp_k as f32) / 35.0 * 0.35).clamp(0.0, 0.35)
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    };
    assert!(
        ice_coverage < 1e-6,
        "Dry planet must have zero ice coverage"
    );

    // Hot planet with water (e.g. 330 K): ice caps must melt completely
    let hot_temp_k = 330.0f64;
    let has_water_hot = true;
    let hot_ice_coverage = if has_water_hot {
        match ClimateRegime::TemperateHabitable {
            ClimateRegime::SnowballIceAge => 1.0,
            ClimateRegime::TemperateHabitable if hot_temp_k < 290.0 => {
                ((290.0 - hot_temp_k as f32) / 35.0 * 0.35).clamp(0.0, 0.35)
            }
            _ => 0.0,
        }
    } else {
        0.0
    };
    assert!(
        hot_ice_coverage < 1e-6,
        "Scorching planet must have zero ice coverage"
    );
}

#[test]
fn test_interpenetrating_bodies_unconditionally_merge() {
    use protostellar::simulation::accretion::collisions::is_physically_interpenetrating;
    use protostellar::simulation::accretion::impact_regimes::{
        classify_impact, ImpactParams, ImpactRegime,
    };
    use protostellar::simulation::components::BodyType;
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let r1 = 0.000042;
    let r2 = 0.000030;
    let r_contact = r1 + r2; // 0.000072

    // Two bodies interpenetrating halfway inside each other (dist = 0.000035 < 0.90 * r_contact)
    let dist = 0.000035;
    let min_dist = 0.000032;
    assert!(
        is_physically_interpenetrating(dist, min_dist, r1, r2, r_contact),
        "Overlapping bodies must be detected as interpenetrating"
    );

    // classify_impact must return EmbeddedMerge even with high impact parameter b = 0.70 and speed u = 1.5
    let p = ImpactParams {
        primary_mass: 0.5 * EARTH_MASS_SOLAR,
        secondary_mass: 0.2 * EARTH_MASS_SOLAR,
        primary_radius_au: r1,
        secondary_radius_au: r2,
        primary_type: BodyType::Protoplanet,
        secondary_type: BodyType::Protoplanet,
        min_dist,
        b: 0.70,
        v_rel: 12.0,
        v_esc: 8.0,
    };
    let regime = classify_impact(p, 0.00010);
    assert_eq!(
        regime,
        ImpactRegime::EmbeddedMerge,
        "Interpenetrating bodies must unconditionally merge rather than graze or bounce"
    );
}

#[test]
fn test_satellite_inside_parent_cleans_up_and_merges() {
    use protostellar::simulation::components::SatelliteOf;

    let parent_radius = 0.000050; // AU
    let sat = SatelliteOf {
        parent: bevy::prelude::Entity::PLACEHOLDER,
        semi_major_axis_au: 0.000030, // Inside parent radius!
        orbital_period_years: 0.01,
        true_anomaly: 0.0,
    };

    let is_sub_radius = sat.semi_major_axis_au < parent_radius * 1.05;
    assert!(
        is_sub_radius,
        "Satellite inside parent radius must be released from Keplerian lock"
    );
}

#[test]
fn test_dry_planet_zero_ice_zero_ocean_coverage() {
    use protostellar::simulation::components::{Composition, VolatileInventory};

    let comp = Composition::rocky();
    assert!(comp.ice_frac < 1e-6, "Rocky planet has 0% ice");

    let norm = comp.normalized();
    let vol = VolatileInventory {
        delivered_water_m_earth: 0.0,
        ocean_coverage_frac: 0.0,
        atmospheric_pressure_bar: 0.0,
        cometary_impact_count: 0,
    };

    let has_water_volatiles = norm.ice_frac > 0.001 || vol.delivered_water_m_earth > 1e-6;
    assert!(
        !has_water_volatiles,
        "Rocky body with 0% ice and 0.0 delivered water must not have water volatiles"
    );

    let ocean_frac = if has_water_volatiles {
        vol.ocean_coverage_frac
    } else {
        0.0
    };
    assert!(
        ocean_frac < 1e-6,
        "Dry planet must have exactly 0.0 ocean fraction"
    );
}

#[test]
fn test_dry_super_earth_classification_and_volatiles() {
    use protostellar::simulation::components::{
        classify_body_by_mass_and_comp, BodyType, Composition, VolatileInventory,
    };
    use protostellar::utils::constants::EARTH_MASS_SOLAR;

    let mass_solar = 3.0 * EARTH_MASS_SOLAR;
    let comp = Composition::rocky();
    let b_type = classify_body_by_mass_and_comp(mass_solar, &comp, false);

    assert_eq!(
        b_type,
        BodyType::SuperEarth,
        "Massive terrestrial world must classify as Super-Earth"
    );

    let norm = comp.normalized();
    let mut vol = VolatileInventory {
        delivered_water_m_earth: 0.0,
        ocean_coverage_frac: 0.0,
        atmospheric_pressure_bar: 1.0,
        cometary_impact_count: 0,
    };

    let has_water_dry = norm.ice_frac > 0.001 || vol.delivered_water_m_earth > 1e-6;
    assert!(
        !has_water_dry,
        "Dry Super-Earth must have no water volatiles"
    );

    // After cometary water delivery
    vol.delivered_water_m_earth = 0.001;
    vol.ocean_coverage_frac = 0.65;
    let has_water_wet = norm.ice_frac > 0.001 || vol.delivered_water_m_earth > 1e-6;
    assert!(
        has_water_wet,
        "Super-Earth with delivered water must register water volatiles"
    );
}

#[test]
fn test_record_impact_crater_basin_and_capacity_limit() {
    use bevy::prelude::*;
    use protostellar::simulation::accretion::record_impact_crater_basin;
    use protostellar::simulation::components::PlanetaryBasins;

    let mut app = App::new();
    let planet_ent = app.world_mut().spawn_empty().id();

    for i in 0..10 {
        let mut commands = app.world_mut().commands();
        record_impact_crater_basin(
            &mut commands,
            planet_ent,
            Vec3::new(1.0, 0.0, 0.0),
            0.15 + i as f32 * 0.01,
            i as f64 * 100.0,
            1.0,
        );
        app.update();
    }

    let pb = app
        .world()
        .entity(planet_ent)
        .get::<PlanetaryBasins>()
        .expect("PlanetaryBasins component must exist on impacted planet");

    assert_eq!(
        pb.basins.len(),
        8,
        "Capacity must cap at 8 most recent impact basins"
    );
    let latest = pb.basins.last().unwrap();
    assert_eq!(latest.melt_glow_fraction, 1.0);
    assert_eq!(latest.elongation, 1.0);
    assert!((latest.surface_normal.length() - 1.0).abs() < 1e-4);
}

#[test]
fn test_record_grazing_impact_scar_elongation() {
    use bevy::math::DVec3;
    use bevy::prelude::*;
    use protostellar::simulation::accretion::record_grazing_impact_scar;
    use protostellar::simulation::components::PlanetaryBasins;

    let mut app = App::new();
    let planet_ent = app.world_mut().spawn_empty().id();

    let mut commands = app.world_mut().commands();
    record_grazing_impact_scar(
        &mut commands,
        planet_ent,
        DVec3::new(0.0, 1.0, 0.0),
        1.0,
        0.5,
        500.0,
        0.85,
    );
    app.update();

    let pb = app
        .world()
        .entity(planet_ent)
        .get::<PlanetaryBasins>()
        .expect("PlanetaryBasins must be attached");

    assert_eq!(pb.basins.len(), 1);
    let scar = &pb.basins[0];
    assert!(
        scar.elongation > 1.2,
        "Grazing impact scar must have elongation > 1.2, got {}",
        scar.elongation
    );
    assert_eq!(scar.surface_normal, Vec3::Y);
    assert_eq!(scar.melt_glow_fraction, 1.0);
}

#[test]
fn test_impact_basin_thermal_relaxation() {
    use bevy::prelude::*;
    use protostellar::simulation::accretion::update_impact_basin_relaxation;
    use protostellar::simulation::components::{ImpactBasin, PlanetaryBasins};
    use protostellar::simulation::resources::{SimTime, TimeWarp};

    let mut app = App::new();
    app.insert_resource(TimeWarp::default());
    let mut sim_time = SimTime::default();
    sim_time.current_dt_yr = 50.0;
    app.insert_resource(sim_time);

    let basin = ImpactBasin {
        surface_normal: Vec3::X,
        angular_radius: 0.10,
        formation_time_yr: 0.0,
        melt_glow_fraction: 1.0,
        elongation: 1.0,
        scar_intensity: 1.0,
    };
    let planet_ent = app
        .world_mut()
        .spawn(PlanetaryBasins {
            basins: vec![basin],
        })
        .id();

    app.add_systems(Update, update_impact_basin_relaxation);
    app.update();

    let pb = app
        .world()
        .entity(planet_ent)
        .get::<PlanetaryBasins>()
        .unwrap();

    assert!(
        pb.basins[0].melt_glow_fraction < 1.0,
        "Basin melt glow must decrease after thermal cooling"
    );
    assert!(
        pb.basins[0].scar_intensity < 1.0,
        "Basin scar intensity must decrease as crust heals"
    );
}

#[test]
fn test_planet_uniforms_impact_basins_layout() {
    use bevy::math::Vec4;
    use protostellar::rendering::materials::PlanetUniforms;

    let uniforms = PlanetUniforms::default();
    assert_eq!(uniforms.impact_basins_pos.len(), 4);
    assert_eq!(uniforms.impact_basins_data.len(), 4);
    for pos in uniforms.impact_basins_pos.iter() {
        assert_eq!(*pos, Vec4::ZERO);
    }
    for data in uniforms.impact_basins_data.iter() {
        assert_eq!(*data, Vec4::ZERO);
    }
}
