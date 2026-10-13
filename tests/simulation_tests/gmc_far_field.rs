use bevy::math::DVec3;
use bevy::prelude::*;
use protostellar::simulation::components::*;
use protostellar::simulation::physics::{
    compute_single_body_acc, grid_cell, GmcNearField, PhysicsBodyEntry,
};
use protostellar::simulation::resources::SimulationConfig;
use protostellar::utils::constants::G_ASTRO;

fn star(entity: Entity, pos: DVec3, central: bool) -> PhysicsBodyEntry {
    PhysicsBodyEntry {
        entity,
        mass: 1.0,
        pos,
        vel: DVec3::ZERO,
        acc: DVec3::ZERO,
        radius: 0.005,
        body_type: BodyType::YellowDwarf,
        satellite: None,
        is_central_star: central,
        name: "S".to_string(),
    }
}

/// Near-field grid + far-field acceleration must reproduce the exact all-pairs result.
#[test]
fn test_near_field_grid_plus_far_field_matches_all_pairs() {
    let near_radius = 50.0_f64;
    let soft_sq = 2.25;
    let mut bodies = Vec::new();
    // Deterministic pseudo-random cluster inside a 400 AU cube; body 0 is the central star.
    let mut seed = 12345_u64;
    let mut rnd = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64) - 0.5
    };
    for i in 0..120 {
        let pos = if i == 0 {
            DVec3::ZERO
        } else {
            DVec3::new(rnd(), rnd(), rnd()) * 400.0
        };
        bodies.push(star(
            Entity::from_raw_u32(i as u32 + 1).unwrap(),
            pos,
            i == 0,
        ));
    }
    let massive: Vec<(DVec3, DVec3, f64, f64, usize)> = bodies
        .iter()
        .enumerate()
        .map(|(i, b)| (b.pos, b.vel, b.mass, b.radius, i))
        .collect();
    let config = SimulationConfig::default();
    let star_index = Some(0);
    let cluster = (DVec3::ZERO, 100.0);

    // Build the grid exactly like the physics step.
    let inv_cell = 1.0 / near_radius;
    let mut grid: hashbrown::HashMap<(i32, i32, i32), Vec<usize>> = hashbrown::HashMap::new();
    for (k, m) in massive.iter().enumerate() {
        if k != 0 {
            grid.entry(grid_cell(m.0, inv_cell)).or_default().push(k);
        }
    }

    // Emulate the GPU far field with a CPU reference (same formula as the shader).
    let far_for = |i: usize| -> DVec3 {
        let tp = bodies[i].pos;
        let mut acc = DVec3::ZERO;
        for (j, m) in massive.iter().enumerate() {
            if j == 0 || (m.0 - tp).length_squared() < near_radius * near_radius {
                continue;
            }
            let d = m.0 - tp;
            let r2 = d.length_squared() + soft_sq;
            acc += d * (G_ASTRO * m.2 / (r2 * r2.sqrt()));
            let s2 = m.0.length_squared() + soft_sq;
            acc -= m.0 * (G_ASTRO * m.2 / (s2 * s2.sqrt()));
        }
        acc
    };
    let far_acc: hashbrown::HashMap<Entity, DVec3> = bodies
        .iter()
        .enumerate()
        .map(|(i, b)| (b.entity, far_for(i)))
        .collect();
    let nf = GmcNearField {
        inv_cell,
        near_sq: near_radius * near_radius,
        grid: &grid,
        star_slot: Some(0),
        far_acc: &far_acc,
    };

    for (i, b) in bodies.iter().enumerate().skip(1) {
        let exact = compute_single_body_acc(
            i, b, &config, 0.0, star_index, 0.0, &massive, None, false, true, cluster, 0.0, None,
        );
        let fast = compute_single_body_acc(
            i,
            b,
            &config,
            0.0,
            star_index,
            0.0,
            &massive,
            None,
            false,
            true,
            cluster,
            0.0,
            Some(&nf),
        );
        let err = (exact - fast).length();
        assert!(
            err <= 1e-6 * exact.length().max(1e-6),
            "body {i}: exact {exact:?} vs fast {fast:?} (err {err})"
        );
    }
}
