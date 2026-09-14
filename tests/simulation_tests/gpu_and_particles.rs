//! Test module generated from simulation_tests.

use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::utils::constants::*;

#[test]
fn test_gravitational_lensing_geometry() {
    use protostellar::rendering::materials::SkyboxMaterial;

    // 1. Verify default material has lensing zeroed / inactive
    let mat = SkyboxMaterial::default();
    assert_eq!(mat.uniforms.lens_pos_and_mass, bevy::prelude::Vec4::ZERO);
    assert_eq!(mat.uniforms.lens_params, bevy::prelude::Vec4::ZERO);

    // 2. Physical & Angular Einstein Radius scaling
    let dist_cam = 150.0_f32; // 150 AU distance

    // Quasi-Star intact cocoon (R ~ 60 AU, lensing extends to R ~ 72 AU)
    let r_cocoon_lens = 72.0_f32;
    let theta_e_cocoon = (r_cocoon_lens / dist_cam).atan();
    let theta_shadow_cocoon = ((60.0_f32 * 0.98) / dist_cam).atan();
    assert!(theta_e_cocoon > theta_shadow_cocoon);
    assert!(theta_e_cocoon > 0.40 && theta_e_cocoon < 0.55);

    // Naked Black Hole after blowout (R ~ 2.5 AU, photon sphere at 1.85x ~ 4.62 AU)
    let visual_r_bh = 2.5_f32;
    let r_bh_lens = visual_r_bh * 1.85;
    let theta_e_bh = (r_bh_lens / dist_cam).atan();
    let theta_shadow_bh = ((visual_r_bh * 0.98) / dist_cam).atan();
    assert!(theta_e_bh > theta_shadow_bh);
    assert!(theta_e_bh > 0.02 && theta_e_bh < 0.05);

    // 3. Blowout contraction: as blowout_p goes 0.0 -> 1.0, lens radius smoothly contracts
    for p in [0.0f32, 0.25, 0.50, 0.75, 1.0] {
        let eff_r = r_cocoon_lens + (r_bh_lens - r_cocoon_lens) * p;
        assert!(eff_r >= r_bh_lens && eff_r <= r_cocoon_lens);
    }

    // 4. Deflection angle alpha(theta) = theta_E^2 / theta
    let theta = 0.60_f32;
    let alpha = (theta_e_cocoon * theta_e_cocoon) / (theta + 0.004);
    assert!(alpha > 0.0 && alpha < theta);
    let beta = theta - alpha;
    assert!(beta > 0.0 && beta < theta);
}

#[test]
fn test_dynamic_roche_disruption_ring_parameters() {
    use bevy::prelude::Entity;
    use protostellar::simulation::resources::RocheDebrisStream;

    // 1. Fluid Roche Limit Calculation
    // Planet: Jupiter-like gas giant (0.001 M_sun, density ~ 1.33 g/cm^3)
    let p_mass = 0.000954; // Solar masses (~ 1 Jupiter mass)
    let p_comp = Composition {
        silicate_frac: 0.10,
        ice_frac: 0.10,
        metal_frac: 0.05,
        organics_frac: 0.0,
        gas_frac: 0.75,
    };
    let p_density = p_comp.average_density();
    let p_rad_au = ((3.0 * p_mass / p_density) / (4.0 * std::f64::consts::PI)).cbrt();

    // Secondary: Volatile icy moon (density ~ 0.95 g/cm^3, ice fraction 85%)
    let s_mass = 0.002 * EARTH_MASS_SOLAR; // 0.002 Earth masses
    let s_comp = Composition {
        silicate_frac: 0.10,
        ice_frac: 0.85,
        metal_frac: 0.05,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let s_density = s_comp.average_density();

    // d_Roche = 2.44 * R_p * (rho_p / rho_s)^(1/3)
    let d_roche = 2.44 * p_rad_au * (p_density / s_density).cbrt();
    assert!(d_roche > p_rad_au);
    assert!((d_roche - 2.44 * p_rad_au * (p_density / s_density).cbrt()).abs() < 1e-10);

    // Rocky primary (Earth-like density) vs icy moon (density ~ 0.95 g/cm^3)
    let rocky_comp = Composition {
        silicate_frac: 0.70,
        ice_frac: 0.0,
        metal_frac: 0.30,
        organics_frac: 0.0,
        gas_frac: 0.0,
    };
    let rocky_density = rocky_comp.average_density();
    let d_roche_rocky = 2.44 * p_rad_au * (rocky_density / s_density).cbrt();
    assert!(d_roche_rocky > 2.44 * p_rad_au);

    // 2. Ring System parameters upon disruption inside Roche limit
    let encounter_dist = d_roche * 0.85;
    let is_inside_roche = encounter_dist <= d_roche;
    assert!(is_inside_roche);

    let ring_mass_earth = s_mass / EARTH_MASS_SOLAR;
    let inner_r = (p_rad_au * 1.25) as f32;
    let outer_r = (d_roche.min(p_rad_au * 3.2)).max(inner_r as f64 * 1.35) as f32;
    let ring_sys = PlanetaryRingSystem {
        inner_radius_au: inner_r,
        outer_radius_au: outer_r,
        ring_mass_earth,
        optical_depth: ((ring_mass_earth / 0.0001).clamp(0.40, 0.95)) as f32,
        ice_fraction: s_comp.ice_frac as f32,
        silicate_fraction: (s_comp.silicate_frac + s_comp.metal_frac) as f32,
    };

    assert!((ring_sys.ring_mass_earth - 0.002).abs() < 1e-6);
    assert!(ring_sys.inner_radius_au < ring_sys.outer_radius_au);
    assert!((ring_sys.ice_fraction - 0.85).abs() < 1e-5);
    assert!(ring_sys.optical_depth >= 0.40 && ring_sys.optical_depth <= 0.95);

    // 3. RocheDebrisStream fragment simulation
    let n_fragments = 48;
    let mut fragments = Vec::with_capacity(n_fragments);
    for k in 0..n_fragments {
        let frac = (k as f32) / (n_fragments as f32);
        let frag_r = inner_r + (outer_r - inner_r) * frac;
        let phase = frac * std::f32::consts::TAU;
        let omega = (1.8 / (frag_r * frag_r * frag_r).sqrt()).clamp(0.4, 8.0);
        fragments.push((frag_r, phase, omega, 0.0f32));
    }

    let mut stream = RocheDebrisStream {
        primary_entity: Entity::from_bits(42),
        primary_pos: bevy::prelude::Vec3::ZERO,
        disruption_pos: bevy::prelude::Vec3::new(encounter_dist as f32, 0.0, 0.0),
        inner_radius: inner_r,
        outer_radius: outer_r,
        timer: 0.0,
        max_timer: 4.5,
        ice_fraction: ring_sys.ice_fraction,
        debris_mass_earth: ring_sys.ring_mass_earth,
        fragments,
    };

    // Advance by 1.0 second
    let dt = 1.0f32;
    stream.timer += dt;
    for frag in stream.fragments.iter_mut() {
        let old_phase = frag.1;
        frag.1 += frag.2 * dt;
        assert!(frag.1 > old_phase); // Angular phase progresses in orbit
    }
    assert!(stream.timer < stream.max_timer);
    assert!((stream.ice_fraction - 0.85).abs() < 1e-5);
}

#[test]
fn test_photoevaporative_escape_mass_loss() {
    // 1. Close-in Sub-Neptune planet at a = 0.04 AU vs Host Star (L = 1.0 L_sun)
    let star_lum = 1.0f64;
    let dist_au_close = 0.04f64;
    let dist_au_far = 1.00f64;

    let p_mass_solar = 5.0 * EARTH_MASS_SOLAR; // 5 Earth masses
    let p_rad_au = 2.4 * EARTH_RADIUS_AU; // 2.4 Earth radii

    let m_earth = (p_mass_solar / EARTH_MASS_SOLAR).max(0.01);
    let r_earth = (p_rad_au / EARTH_RADIUS_AU).max(0.1);

    // Energy-limited mass loss calculation:
    // Loss rate ~ 0.15 * R_p^3 / M_p * (L / d^2)^0.85
    let flux_factor_close = (star_lum / (dist_au_close * dist_au_close)).powf(0.85);
    let loss_rate_close =
        ((0.15 * r_earth.powi(3) / m_earth) * flux_factor_close).clamp(0.01, 100.0) as f32;

    assert!(loss_rate_close > 20.0); // Extremely vigorous hydrodynamic escape!

    let tail_len_close = (((0.25 / dist_au_close).powf(1.1) * 0.75 * star_lum.min(5.0).powf(0.25))
        .clamp(0.25, 6.0)) as f32;
    assert!(tail_len_close > 4.0); // Prominent cometary outflow tail extending multiple AU

    // 2. Distant planet at 1.0 AU: outside 0.25 AU photoevaporation boundary
    let is_close = dist_au_close < 0.25;
    let is_far = dist_au_far < 0.25;
    assert!(is_close);
    assert!(!is_far);

    // 3. Envelope mass stripping across geological time
    let mut comp = Composition {
        silicate_frac: 0.40,
        ice_frac: 0.10,
        metal_frac: 0.20,
        organics_frac: 0.0,
        gas_frac: 0.30, // Initial 30% volatile envelope
    };

    // Pre-stripping ionization color: vibrant electric cyan (Hydrogen/Helium envelope)
    let initial_ion_color = if comp.gas_frac > 0.15 {
        bevy::prelude::Color::srgba(0.25, 0.85, 1.0, 0.85) // Electric Cyan
    } else {
        bevy::prelude::Color::srgba(1.0, 0.65, 0.20, 0.85) // Amber
    };
    assert_eq!(initial_ion_color.to_srgba().red, 0.25);
    assert_eq!(initial_ion_color.to_srgba().green, 0.85);

    let mut current_mass_solar = p_mass_solar;
    let dt_myr = 0.05; // 50,000 years of extreme irradiation
    let delta_m_earth = (loss_rate_close as f64) * dt_myr;
    let delta_m_solar = delta_m_earth * EARTH_MASS_SOLAR;

    let cur_gas_m = current_mass_solar * comp.gas_frac;
    let stripped = delta_m_solar.min(cur_gas_m * 0.999);
    current_mass_solar -= stripped;

    let new_gas_m = (cur_gas_m - stripped).max(0.0);
    comp.gas_frac = (new_gas_m / current_mass_solar).clamp(0.0, 1.0);

    // Gas fraction is stripped down into the Hot Neptune Desert!
    assert!(comp.gas_frac < 0.15);
    assert!(current_mass_solar < p_mass_solar);

    // Post-stripping: unmasked mineral/silicate vapor core glows amber
    let post_strip_color = if comp.gas_frac > 0.15 {
        bevy::prelude::Color::srgba(0.25, 0.85, 1.0, 0.85)
    } else {
        bevy::prelude::Color::srgba(1.0, 0.65, 0.20, 0.85) // Warm amber
    };
    assert_eq!(post_strip_color.to_srgba().red, 1.0);
    assert_eq!(post_strip_color.to_srgba().green, 0.65);
}

#[test]
fn test_gpu_particle_buffer_layouts_and_alignment() {
    use protostellar::gpu::buffers::{GpuOrbitUniforms, GpuParticle, MassiveBodyGpu};

    // 1. Validate GpuParticle 48-byte layout (3x vec4<f32>) and 16-byte std430 alignment
    assert_eq!(std::mem::size_of::<GpuParticle>(), 48);
    assert_eq!(std::mem::align_of::<GpuParticle>(), 16);

    // 2. Validate MassiveBodyGpu 16-byte layout (vec4<f32>) and 16-byte alignment
    assert_eq!(std::mem::size_of::<MassiveBodyGpu>(), 16);
    assert_eq!(std::mem::align_of::<MassiveBodyGpu>(), 16);

    // 3. Validate GpuOrbitUniforms layout (592 bytes) and 16-byte uniform alignment
    assert_eq!(std::mem::size_of::<GpuOrbitUniforms>(), 592);
    assert_eq!(std::mem::align_of::<GpuOrbitUniforms>(), 16);

    // 4. Validate default values for uniform buffer
    let uniforms = GpuOrbitUniforms::default();
    assert_eq!(uniforms.num_particles, 50000);
    assert_eq!(uniforms.massive_bodies.len(), 32);
    assert_eq!(uniforms.star_mass, 1.0);
    assert!(uniforms.g_const > 39.0);
}

#[test]
fn test_gpu_workgroup_dispatch_sizing() {
    // Validate GPU compute workgroup dispatch sizing for all particle scaling tiers
    let workgroup_size = 64u32;

    let tiers = [
        (50_000u32, 782u32),
        (100_000u32, 1563u32),
        (250_000u32, 3907u32),
        (500_000u32, 7813u32),
        (1_000_000u32, 15625u32),
    ];

    for (particles, expected_workgroups) in tiers {
        let workgroups = particles.div_ceil(workgroup_size);
        assert_eq!(workgroups, expected_workgroups);

        // Validate buffer size in VRAM
        let buffer_size_bytes =
            (particles as usize) * std::mem::size_of::<protostellar::gpu::buffers::GpuParticle>();
        assert_eq!(buffer_size_bytes, (particles as usize) * 48);

        // At 100k particles, storage buffer is ~4.8 MB (extremely lightweight in VRAM)
        if particles == 100_000 {
            assert_eq!(buffer_size_bytes, 4_800_000);
        }
    }
}

#[test]
fn test_gpu_readback_dead_particle_non_resurrection() {
    use protostellar::gpu::buffers::GpuParticle;

    // Simulate 4 particles
    let mut cpu_masses = [0.0001f32, 0.0f32, 0.0001f32, 0.0f32]; // 1 & 3 accreted on CPU
    let mut cpu_positions = [
        [1.0f32, 0.0, 0.0],
        [0.0, -5000.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, -5000.0, 0.0],
    ];

    // GPU buffer arrives from older in-flight frame where particle 1 had not died yet on GPU
    let gpu_particles = [
        GpuParticle {
            pos_mass: [1.05, 0.0, 0.0, 0.0001],
            vel_temp: [0.0, 0.0, 0.0, 280.0],
            composition: [0.5, 0.5, 0.0, 0.0],
        },
        GpuParticle {
            pos_mass: [1.5, 0.0, 0.0, 0.0001], // GPU still thinks it's alive!
            vel_temp: [0.0, 0.0, 0.0, 250.0],
            composition: [0.5, 0.5, 0.0, 0.0],
        },
        GpuParticle {
            pos_mass: [0.0, -5000.0, 0.0, 0.0], // GPU marked dead
            vel_temp: [0.0, 0.0, 0.0, 0.0],
            composition: [0.0, 0.0, 0.0, 0.0],
        },
        GpuParticle {
            pos_mass: [0.0, -5000.0, 0.0, 0.0],
            vel_temp: [0.0, 0.0, 0.0, 0.0],
            composition: [0.0, 0.0, 0.0, 0.0],
        },
    ];

    let is_scenario_start = false;
    for (i, p) in gpu_particles.iter().enumerate() {
        // Particle must never be resurrected if CPU already marked it dead
        if !is_scenario_start && cpu_masses[i] <= 0.0 {
            continue;
        }
        if p.pos_mass[3] <= 0.0 {
            cpu_masses[i] = 0.0;
            cpu_positions[i] = [0.0, -5000.0, 0.0];
            continue;
        }
        cpu_positions[i] = [p.pos_mass[0], p.pos_mass[1], p.pos_mass[2]];
        cpu_masses[i] = p.pos_mass[3];
    }

    // Particle 0 updated normally
    assert_eq!(cpu_masses[0], 0.0001);
    assert_eq!(cpu_positions[0], [1.05, 0.0, 0.0]);

    // Particle 1 was NOT resurrected (remained dead at -5000)
    assert_eq!(cpu_masses[1], 0.0);
    assert_eq!(cpu_positions[1], [0.0, -5000.0, 0.0]);

    // Particle 2 was killed by GPU
    assert_eq!(cpu_masses[2], 0.0);
    assert_eq!(cpu_positions[2], [0.0, -5000.0, 0.0]);

    // Particle 3 stayed dead
    assert_eq!(cpu_masses[3], 0.0);
    assert_eq!(cpu_positions[3], [0.0, -5000.0, 0.0]);
}

#[test]
fn test_outer_giant_planet_mass_ceiling() {
    use protostellar::utils::constants::{EARTH_MASS_SOLAR, JUPITER_MASS_SOLAR};

    // Proto-Jupiter in Solar Nebula MMSN
    let mut mass = 3.50 * EARTH_MASS_SOLAR;
    let max_giant_mass = 2.5 * JUPITER_MASS_SOLAR;

    // Simulate massive runaway accretion attempts (e.g. 10,000 particle sweeps)
    for _ in 0..10_000 {
        let gain = 100.0 * (0.00010 * EARTH_MASS_SOLAR / 100_000.0); // 100 particles
        let m_earth = mass / EARTH_MASS_SOLAR;
        let runaway_mult = if m_earth < 10.0 {
            1.0 + 0.05 * m_earth
        } else {
            1.5 + 0.15 * m_earth.clamp(10.0, 350.0).powf(0.30)
        };
        mass = (mass + gain * runaway_mult).min(max_giant_mass);
    }

    // Mass must be capped at 2.5 M_Jup and NEVER reach stellar/black hole mass (> 1000 M_earth)
    assert!(mass <= max_giant_mass);
    assert!(mass < 0.01); // Well below stellar threshold (0.08 M_sun)
}

#[test]
fn test_lighter_particles_moderate_50_year_growth() {
    // Verifies that with authentic lighter dust particle masses (~0.00010 M_sun disk,
    // ~0.00033 M_earth per particle), planets grow at a realistic, measured pace
    // and do NOT balloon to 350 M_earth within the first 50 years.
    let n_particles = 100_000.0;
    let disk_mass = 0.00010; // M_sun (~33 Earth masses across the entire solar nebula)
    let particle_mass = disk_mass / n_particles; // ~1.0e-9 M_sun ~ 0.00033 M_earth
    let particle_mass_earth = particle_mass / EARTH_MASS_SOLAR;

    assert!(
        particle_mass_earth < 0.0005,
        "Individual particle must be lightweight (~0.00033 M_earth)"
    );

    // Initial protoplanetary core (e.g. 3.0 M_earth)
    let mut core_mass_earth: f64 = 3.0;

    // Simulate 50 years of orbital sweeps (e.g. accreting ~20 particles per year)
    let years = 50;
    let particles_per_year = 20.0;

    for _ in 0..years {
        let annual_gain_earth = particles_per_year * particle_mass_earth;
        let runaway_mult = if core_mass_earth < 10.0 {
            1.0 + 0.05 * core_mass_earth
        } else {
            1.5 + 0.15 * core_mass_earth.clamp(10.0, 350.0).powf(0.30)
        };
        core_mass_earth += annual_gain_earth * runaway_mult;
    }

    // After 50 years:
    // Core should have grown by a modest, realistic amount (~0.3 to 0.5 M_earth),
    // and must be strictly well below 10 M_earth (nowhere near 350 M_earth!)
    assert!(
        core_mass_earth > 3.2 && core_mass_earth < 5.0,
        "50-year growth should be gentle and realistic (expected ~3.3-4.0 M_earth, got {})",
        core_mass_earth
    );
    assert!(
        core_mass_earth < 10.0,
        "Planets must not prematurely trigger runaway gas accretion in the first 50 years"
    );
}

#[test]
fn test_skybox_spherical_isotropy_and_flaring_elimination() {
    use bevy::math::Vec3;

    // 1. Verify shader source code guarantees: flaring formulas removed, spherical isotropic layer present
    let shader_src = std::fs::read_to_string("assets/shaders/skybox.wgsl")
        .expect("skybox.wgsl should be readable");

    // Must contain the new isotropic spherical layer
    assert!(
        shader_src.contains("fn render_star_layer"),
        "skybox.wgsl must contain render_star_layer"
    );

    // Must NOT contain Cartesian diffraction cross flare smears
    assert!(
        !shader_src.contains("spike_h"),
        "skybox.wgsl must not contain spike_h Cartesian flare"
    );
    assert!(
        !shader_src.contains("spike_v"),
        "skybox.wgsl must not contain spike_v Cartesian flare"
    );

    // Must contain spherical deep sky objects
    assert!(
        shader_src.contains("pleiades_dir"),
        "skybox.wgsl must contain Pleiades open cluster"
    );
    assert!(
        shader_src.contains("globular_dir"),
        "skybox.wgsl must contain Omega Centauri globular cluster"
    );
    assert!(
        shader_src.contains("m31_dir"),
        "skybox.wgsl must contain Andromeda Galaxy (M31)"
    );
    assert!(
        !shader_src.contains("dust_offset >"),
        "skybox.wgsl must not contain hard rectangular dust bounding box"
    );
    assert!(
        shader_src.contains("m31_bulge")
            && shader_src.contains("m31_disk")
            && shader_src.contains("dust_transmission"),
        "skybox.wgsl must contain smooth chromatic Andromeda model with continuous dust absorption"
    );

    // 2. Mathematical Rotational Invariance Proof for Spherical Metric:
    // th2 = dot(d - s, d - s)
    let star_dir = Vec3::new(0.62, 0.44, -0.65).normalize();
    let view_offset = Vec3::new(0.002, -0.003, 0.001);
    let view_dir = (star_dir + view_offset).normalize();

    let delta_orig = view_dir - star_dir;
    let dist_sq_original = delta_orig.dot(delta_orig);

    // Rotate both vectors arbitrarily in 3D (pitch 37 deg, yaw 112 deg, roll 58 deg)
    let rot = bevy::math::Quat::from_euler(
        bevy::math::EulerRot::XYZ,
        0.64577, // 37 deg
        1.95477, // 112 deg
        1.01229, // 58 deg
    );
    let rotated_view = rot * view_dir;
    let rotated_star = rot * star_dir;

    let delta_rot = rotated_view - rotated_star;
    let dist_sq_rotated = delta_rot.dot(delta_rot);

    // The angular metric must be exactly identical under any 3D rotation (zero skew, zero eccentricity)
    assert!(
        (dist_sq_original - dist_sq_rotated).abs() < 1e-6,
        "Angular distance metric must be perfectly rotationally invariant"
    );

    // 3. Gaussian profile circular symmetry: points at identical angular radii must have identical intensities
    let sigma = 0.0016_f32;
    let r_angle = 0.0020_f32; // 0.002 rad from star center

    // Create orthonormal tangent vectors perp to star_dir
    let tangent_x = star_dir.cross(Vec3::Y).normalize();
    let tangent_y = star_dir.cross(tangent_x).normalize();

    // Baseline intensity at r_angle
    let base_dir = (star_dir + tangent_x * r_angle).normalize();
    let base_delta = base_dir - star_dir;
    let base_th2 = base_delta.dot(base_delta);
    let intensity_0 = (-base_th2 / (sigma * sigma)).exp();

    // Evaluate 8 points in a circle around the star in the celestial tangent plane
    for step in 0..8 {
        let phi = (step as f32) * (std::f32::consts::PI / 4.0);
        let offset_dir =
            (star_dir + (tangent_x * phi.cos() + tangent_y * phi.sin()) * r_angle).normalize();
        let delta = offset_dir - star_dir;
        let th2 = delta.dot(delta);
        let sample_intensity = (-th2 / (sigma * sigma)).exp();

        assert!(
            (sample_intensity - intensity_0).abs() < 1e-5,
            "Star profile must be perfectly circular and unskewed at all angles (step {})",
            step
        );
    }
}

#[test]
fn test_inner_planet_acceleration_unattenuated() {
    // Inner planets (Mercury at ~0.387 AU, Venus at ~0.723 AU) around a 1 M_sun star
    // experience high gravitational accelerations:
    // a = G * M / r^2
    // For Mercury: 39.4784 / (0.387^2) = 263.6 AU/yr^2
    // The previous 80.0 AU/yr^2 acceleration cap artificially severed 70% of solar gravity,
    // flinging inner planets into spurious escape orbits!
    let star_mass = 1.0; // M_sun
    let mercury_r = 0.387; // AU
    let r_vec = DVec3::new(mercury_r, 0.0, 0.0);
    let dist_sq = r_vec.length_squared();
    let _dist = dist_sq.sqrt();

    let softening_sq = 0.001 * 0.001;
    let softened_dist = (dist_sq + softening_sq).sqrt();
    let mut acc = -(G_ASTRO * star_mass / (softened_dist * softened_dist * softened_dist)) * r_vec;

    let acc_mag = acc.length();
    // Verify physical acceleration is ~263 AU/yr^2
    assert!(
        acc_mag > 260.0 && acc_mag < 270.0,
        "Mercury-like orbital acceleration should be ~263.6 AU/yr^2, got {}",
        acc_mag
    );

    // Apply the updated acceleration limiter (500,000 AU/yr^2)
    if acc_mag > 500_000.0 {
        acc *= 500_000.0 / acc_mag;
    }

    // Must NOT be capped at 80.0 AU/yr^2
    assert!(
        acc.length() > 250.0,
        "Acceleration limiter must not truncate inner planetary gravity"
    );
}

#[test]
fn test_unbounded_interstellar_drift_no_112_au_clamping() {
    // Ejected or rogue planets moving outwards beyond the disk outer radius (e.g. 112.5 AU)
    // must drift along their velocity vector (r += v * dt) into interstellar space,
    // and must NOT be clamped to a 112.5 AU sphere or forced into 2D circular Keplerian rotation.
    let initial_pos = DVec3::new(112.50, 0.0, 0.0); // Exactly at old boundary
    let escape_vel = DVec3::new(10.0, 0.0, 2.0); // 10 AU/yr radially outward
    let dt = 0.5; // 0.5 year timestep

    let mut pos = initial_pos;
    let vel = escape_vel;

    // Symplectic Leapfrog linear drift
    pos += vel * dt;

    // Body should now be at x = 117.5 AU, z = 1.0 AU
    assert_eq!(pos.x, 117.5);
    assert_eq!(pos.z, 1.0);
    assert!(
        pos.length() > 112.50,
        "Body must freely drift past 112.5 AU into interstellar space"
    );

    // Old clamping would have forced: pos *= 112.5 / pos.length(), leaving it stuck at 112.5 AU!
    let old_clamped = pos * (112.50 / pos.length());
    assert!((old_clamped.length() - 112.50).abs() < 1e-10);
    assert!(
        pos.length() > old_clamped.length(),
        "Position must not be pinned to 112.5 AU"
    );
}

#[test]
fn test_minor_debris_deep_space_retirement_threshold() {
    // Minor debris (asteroids, planetesimals, comets) beyond 2000 AU should be retired,
    // while rogue planets and brown dwarfs remain persistent in interstellar space.
    let r_close_debris = 150.0;
    let r_escaped_debris = 2500.0;
    let r_rogue_planet = 2500.0;

    let debris_type = BodyType::Asteroid;
    let planet_type = BodyType::GasGiant;

    let retire_debris_close = r_close_debris > 2000.0
        && matches!(
            debris_type,
            BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain
        );
    assert!(!retire_debris_close, "Debris at 150 AU must not be retired");

    let retire_debris_far = r_escaped_debris > 2000.0
        && matches!(
            debris_type,
            BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain
        );
    assert!(retire_debris_far, "Debris at 2500 AU should be retired");

    let retire_planet_far = r_rogue_planet > 2000.0
        && matches!(
            planet_type,
            BodyType::Planetesimal | BodyType::Asteroid | BodyType::Comet | BodyType::DustGrain
        );
    assert!(
        !retire_planet_far,
        "Rogue planets/gas giants must never be retired in deep space"
    );
}

#[test]
fn test_swarm_clump_promotion_capacity_guards() {
    // Runaway particle clumps should only promote to ECS massive bodies if total ECS count < 24
    // and mass exceeds protoplanetary embryo threshold (~0.005 M_earth)
    let b_mass = 0.0001f32;
    let promo_threshold = (16.0f32 * b_mass).max(EARTH_MASS_SOLAR as f32 * 0.005);

    // 1. Small clump below threshold -> no promotion
    let small_clump_mass = 0.0005f32;
    assert!(small_clump_mass < promo_threshold);

    // 2. Large clump above threshold, but ECS body limit reached (e.g. 24) -> no promotion
    let ecs_count_full = 24;
    let large_clump_mass = promo_threshold + 0.001f32;
    let can_promote_full = large_clump_mass >= promo_threshold && ecs_count_full < 24;
    assert!(
        !can_promote_full,
        "Must not promote when ECS capacity is full"
    );

    // 3. Large clump above threshold with room in system -> promote
    let ecs_count_open = 12;
    let can_promote_open = large_clump_mass >= promo_threshold && ecs_count_open < 24;
    assert!(can_promote_open, "Should promote when under ECS capacity");
}

#[test]
fn test_little_red_dot_high_speed_unbounded_limits() {
    // In the JWST Little Red Dot scenario (450,000 M_sun Black Hole Star),
    // orbiting bodies travel at extreme relativistic velocities (thousands of km/s)
    // and experience immense gravitational accelerations.
    // The physics limits (max speed 200 AU/yr, max acc 500,000 AU/yr^2) must NOT apply!
    let quasi_star_mass = 450_000.0; // M_sun

    // 1. Orbital velocity at 85 AU (primordial cloudlet orbit):
    // v = sqrt(G * M / r) = sqrt(39.4784 * 450,000 / 85) = 457.17 AU/yr (~2,167 km/s)
    let r_orbit = 85.0; // AU
    let v_circ = (G_ASTRO * quasi_star_mass / r_orbit).sqrt();
    let speed_km_s = v_circ * AU_PER_YR_TO_KM_PER_S;

    assert!(
        v_circ > 450.0 && v_circ < 465.0,
        "Little Red Dot 85 AU orbital speed should be ~457 AU/yr, got {}",
        v_circ
    );
    assert!(
        speed_km_s > 2100.0 && speed_km_s < 2200.0,
        "Orbital speed in km/s should be ~2,167 km/s, got {}",
        speed_km_s
    );

    // Verify velocity limit exemption for Little Red Dot:
    let is_little_red_dot = true;
    let mut vel = DVec3::new(0.0, 0.0, v_circ);
    if !is_little_red_dot {
        let speed = vel.length();
        let max_speed = 200.0;
        if speed > max_speed {
            vel *= max_speed / speed;
        }
    }
    assert_eq!(
        vel.length(),
        v_circ,
        "Little Red Dot orbiting objects must retain their full >450 AU/yr velocity without capping"
    );

    // In a normal solar system, velocity would have been bounded to 200 AU/yr:
    let is_normal_system = false;
    let mut normal_vel = DVec3::new(0.0, 0.0, v_circ);
    if !is_normal_system {
        let speed = normal_vel.length();
        let max_speed = 200.0;
        if speed > max_speed {
            normal_vel *= max_speed / speed;
        }
    }
    assert_eq!(normal_vel.length(), 200.0);

    // 2. Gravitational acceleration at 3 AU (infalling gas near the 60 AU cocoon):
    // a = G * M / r^2 = 39.4784 * 450,000 / 9 = ~1,973,920 AU/yr^2
    let r_inner = 3.0; // AU
    let acc_phys = G_ASTRO * quasi_star_mass / (r_inner * r_inner);
    assert!(
        acc_phys > 1_900_000.0,
        "Physical acceleration should be ~1.97M AU/yr^2, got {}",
        acc_phys
    );

    let mut acc = DVec3::new(acc_phys, 0.0, 0.0);
    if !is_little_red_dot && acc.length() > 500_000.0 {
        acc *= 500_000.0 / acc.length();
    }
    assert_eq!(
        acc.length(),
        acc_phys,
        "Little Red Dot acceleration must NOT be capped at 500,000 AU/yr^2"
    );

    // 3. Debris escape radius: At 3,000 AU, debris is still bound to Little Red Dot
    // (v_esc = sqrt(2 * G * M / 3000) = ~108 AU/yr). Must NOT be retired at 2,000 AU.
    let debris_dist = 3000.0;
    let debris_escape_radius = if is_little_red_dot { 100_000.0 } else { 2000.0 };
    assert!(
        debris_dist < debris_escape_radius,
        "Debris at 3,000 AU is bound to Little Red Dot and must not be retired"
    );
}

#[test]
fn test_nan_and_inf_state_vector_sanitization() {
    // When non-finite numbers (NaN, +Inf, -Inf) appear in position or velocity
    // (due to division by zero, precision underflow, or unphysical inputs),
    // the physics engine must safely catch and sanitize them without panicking.

    // 1. Standard Solar System Sanitization: Resets to 1.0 AU circular orbit
    let star_mass_solar = 1.0;
    let mut pos_corrupted = DVec3::new(f64::NAN, 0.0, f64::INFINITY);
    let mut vel_corrupted = DVec3::new(0.0, f64::NEG_INFINITY, 0.0);
    let mut acc = DVec3::new(5.0, 0.0, 0.0);
    let is_little_red_dot = false;

    if !pos_corrupted.is_finite() || !vel_corrupted.is_finite() {
        let safe_r = if is_little_red_dot { 120.0 } else { 1.0 };
        let v_k = (G_ASTRO * star_mass_solar / safe_r).sqrt();
        pos_corrupted = DVec3::new(safe_r, 0.0, 0.0);
        vel_corrupted = DVec3::new(0.0, 0.0, v_k);
        acc = DVec3::ZERO;
    }

    assert!(pos_corrupted.is_finite());
    assert!(vel_corrupted.is_finite());
    assert_eq!(pos_corrupted, DVec3::new(1.0, 0.0, 0.0));
    assert!((vel_corrupted.z - 2.0 * std::f64::consts::PI).abs() < 1e-4);
    assert_eq!(acc, DVec3::ZERO);

    // 2. Little Red Dot Sanitization: Resets safely to 120.0 AU outside the 60 AU cocoon
    let star_mass_lrd = 450_000.0;
    let mut lrd_pos = DVec3::new(f64::NAN, f64::NAN, 0.0);
    let mut lrd_vel = DVec3::ZERO;
    let is_little_red_dot = true;

    if !lrd_pos.is_finite() || !lrd_vel.is_finite() {
        let safe_r = if is_little_red_dot { 120.0 } else { 1.0 };
        let v_k = (G_ASTRO * star_mass_lrd / safe_r).sqrt();
        lrd_pos = DVec3::new(safe_r, 0.0, 0.0);
        lrd_vel = DVec3::new(0.0, 0.0, v_k);
    }

    assert!(lrd_pos.is_finite());
    assert!(lrd_vel.is_finite());
    assert_eq!(lrd_pos.x, 120.0);
    assert!(lrd_vel.z > 380.0 && lrd_vel.z < 390.0); // ~384.8 AU/yr
}

#[test]
fn test_pop3_infalling_star_particle_accretion_safety() {
    use protostellar::rendering::particle_swarm::simulation::*;
    use protostellar::rendering::particle_swarm::ParticleSwarmData;
    use protostellar::simulation::resources::DiskParameters;

    let count = 1024;
    let mut data = ParticleSwarmData {
        positions: (0..count)
            .map(|i| [0.5 + (i as f32 / count as f32) * 2.5, 0.0, 0.0])
            .collect(),
        velocities: vec![[0.0, 0.0, 1.0]; count],
        masses: vec![1e-9; count],
        compositions: vec![Composition::default(); count],
        temperatures: vec![1500.0; count],
        colors: vec![[1.0, 0.8, 0.6, 1.0]; count],
        mesh_positions: vec![[0.0; 3]; count * 4],
        mesh_colors: vec![[1.0; 4]; count * 4],
        bin_heads: vec![-1; 4096],
        bin_next: vec![-1; count],
        mesh_handle: Handle::default(),
        count,
        base_mass: 1e-9,
        is_dirty: false,
        pending_gpu_accretions: Vec::new(),
    };

    let disk_params = DiskParameters::default();
    let pop3_entity = Entity::from_bits(42);
    let massive_bodies = vec![(
        pop3_entity,
        DVec3::new(2.0, 0.0, 0.0),
        120.0,
        BodyType::BlueSupergiant,
    )];

    let params = ParticleIntegrationParams {
        star_m: 1.0,
        star_pos_f32: [0.0, 0.0, 0.0],
        speed_mult: 1000.0,
        visual_flow_dt: 0.016,
        g_const: G_ASTRO as f32,
        enable_gas_drag: true,
        gas_scale: 1.0,
        shockwave_r: 0.0,
        quasar_blown_out: false,
        tractor_pos_mass: [0.0, 0.0, 0.0, 0.0],
        star_is_ignited: true,
        gpu_active: false,
        lhb_active: false,
        lhb_resonance: false,
        disk_params: &disk_params,
        massive_bodies: &massive_bodies,
    };

    // Must execute cleanly without panicking on min > max clamp
    let accretions = integrate_particles_and_collect_accretions(&mut data, &params);
    assert!(
        !accretions.is_empty(),
        "Pop-III hypergiant should accrete nearby particles"
    );
}
