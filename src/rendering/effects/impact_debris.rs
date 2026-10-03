use bevy::prelude::*;

use crate::simulation::accretion::events::*;
use crate::simulation::components::*;
use crate::simulation::resources::*;

pub fn update_impact_ejecta(
    time: Res<Time>,
    mut ejecta_pool: ResMut<ImpactEjectaPool>,
    mut moon_reader: MessageReader<MoonFormationEvent>,
    bodies_query: Query<&SimPosition>,
) {
    let dt = time.delta_secs();

    // Spawn a debris disk for each MoonFormationEvent
    for ev in moon_reader.read() {
        let n_fragments = 36;
        let mut fragments = Vec::with_capacity(n_fragments);

        let orbit_r = ev.orbital_radius_au as f32;
        let inner_r = orbit_r * 0.4;
        let outer_r = orbit_r * 1.5;

        for k in 0..n_fragments {
            let frac = (k as f32) / (n_fragments as f32);
            let frag_r = inner_r + (outer_r - inner_r) * frac * frac.sqrt();
            let phase = frac * std::f32::consts::TAU * 3.0;
            let omega = (2.0 / (frag_r.max(0.01))).clamp(0.1, 5.0);
            let z_off = ((k as f32 * 2.3).sin() * 0.05) * orbit_r;

            fragments.push((frag_r, phase, omega, z_off));
        }

        ejecta_pool.rings.push(ImpactEjectaRing {
            primary_entity: ev.parent_entity,
            primary_pos: Vec3::ZERO, // updated below
            impact_pos: Vec3::ZERO,
            timer: 0.0,
            max_timer: 15.0, // Lasts for a while, slowly accreting
            ring_mass: ev.moon_mass,
            color: Color::srgba(1.0, 0.4, 0.1, 0.95), // Glowing hot magma ejecta
            fragments,
        });
    }

    ejecta_pool.rings.retain_mut(|ring| {
        ring.timer += dt;

        if let Ok(pos) = bodies_query.get(ring.primary_entity) {
            ring.primary_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        }

        for frag in &mut ring.fragments {
            frag.1 += frag.2 * dt;
        }

        ring.timer < ring.max_timer
    });
}

pub fn draw_impact_ejecta(gizmos: &mut Gizmos, ejecta_pool: &ImpactEjectaPool) {
    for ring in &ejecta_pool.rings {
        let p = (ring.timer / ring.max_timer).clamp(0.0, 1.0);

        // Coalescing into a single point (moon) over time
        let coalesce = p * p;
        let alpha = if p < 0.1 {
            p / 0.1
        } else if p > 0.8 {
            (1.0 - p) / 0.2
        } else {
            1.0
        };

        let spark_col = Color::srgba(
            ring.color.to_srgba().red,
            ring.color.to_srgba().green,
            ring.color.to_srgba().blue,
            alpha * 0.9,
        );
        let line_col = Color::srgba(1.0, 0.2, 0.0, alpha * 0.4);

        for (i, &(r, phi, _, z)) in ring.fragments.iter().enumerate() {
            // As time progresses, orbit tightens and z normalizes
            let current_r = r * (1.0 - coalesce * 0.5);
            let current_z = z * (1.0 - coalesce * 0.9);

            let pos1 = ring.primary_pos
                + Vec3::new(current_r * phi.cos(), current_z, current_r * phi.sin());

            gizmos.sphere(
                Isometry3d::from_translation(pos1),
                0.012 + 0.005 * (1.0 - p),
                spark_col,
            );

            if let Some(&(r2, phi2, _, z2)) = ring.fragments.get(i + 1) {
                let cr2 = r2 * (1.0 - coalesce * 0.5);
                let cz2 = z2 * (1.0 - coalesce * 0.9);
                let pos2 = ring.primary_pos + Vec3::new(cr2 * phi2.cos(), cz2, cr2 * phi2.sin());
                gizmos.line(pos1, pos2, line_col);
            }
        }
    }
}

pub fn draw_impact_ejecta_system(mut gizmos: Gizmos, ejecta_pool: Option<Res<ImpactEjectaPool>>) {
    if let Some(pool) = ejecta_pool {
        if !pool.rings.is_empty() {
            draw_impact_ejecta(&mut gizmos, &pool);
        }
    }
}
