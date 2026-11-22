//! Procedural multi-part 3D aerospace & megastructure geometry builders with UVs,
//! outward normals, vertex material zones, and baked crevice ambient occlusion.

use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

struct BoxColors {
    face_pos_y: [f32; 4],
    face_neg_y: [f32; 4],
    sides: [f32; 4],
}

impl BoxColors {
    const fn uniform(col: [f32; 4]) -> Self {
        Self {
            face_pos_y: col,
            face_neg_y: col,
            sides: col,
        }
    }
}

struct TechMeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl TechMeshBuilder {
    fn with_capacity(verts: usize, tris: usize) -> Self {
        Self {
            positions: Vec::with_capacity(verts),
            normals: Vec::with_capacity(verts),
            uvs: Vec::with_capacity(verts),
            colors: Vec::with_capacity(verts),
            indices: Vec::with_capacity(tris * 3),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Low-level quad primitive takes 4 vertices, normal, UV scale, and color"
    )]
    fn push_quad(
        &mut self,
        a: Vec3,
        b: Vec3,
        c: Vec3,
        d: Vec3,
        normal: Vec3,
        uv_scale: Vec2,
        color: [f32; 4],
    ) {
        let base = self.positions.len() as u32;
        let n = normal.normalize_or_zero();
        let face_n = (b - a).cross(c - a);
        let flip = face_n.dot(n) < 0.0;

        self.positions
            .extend_from_slice(&[a.to_array(), b.to_array(), c.to_array(), d.to_array()]);
        self.normals.extend_from_slice(&[n.to_array(); 4]);
        self.uvs.extend_from_slice(&[
            [0.0, 0.0],
            [uv_scale.x, 0.0],
            [uv_scale.x, uv_scale.y],
            [0.0, uv_scale.y],
        ]);
        self.colors.extend_from_slice(&[color; 4]);
        if flip {
            self.indices
                .extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        } else {
            self.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Oriented box builder takes basis axes, half-extents, UV scale, and face colors"
    )]
    fn push_oriented_box(
        &mut self,
        center: Vec3,
        right: Vec3,
        up: Vec3,
        forward: Vec3,
        half: Vec3,
        uv_scale: Vec2,
        cols: BoxColors,
    ) {
        let rx = right * half.x;
        let uy = up * half.y;
        let fz = forward * half.z;

        let p000 = center - rx - uy - fz;
        let p100 = center + rx - uy - fz;
        let p110 = center + rx + uy - fz;
        let p010 = center - rx + uy - fz;
        let p001 = center - rx - uy + fz;
        let p101 = center + rx - uy + fz;
        let p111 = center + rx + uy + fz;
        let p011 = center - rx + uy + fz;

        self.push_quad(p011, p111, p110, p010, up, uv_scale, cols.face_pos_y);
        self.push_quad(p000, p100, p101, p001, -up, uv_scale, cols.face_neg_y);
        self.push_quad(p100, p110, p111, p101, right, uv_scale, cols.sides);
        self.push_quad(p001, p011, p010, p000, -right, uv_scale, cols.sides);
        self.push_quad(p001, p101, p111, p011, forward, uv_scale, cols.sides);
        self.push_quad(p100, p000, p010, p110, -forward, uv_scale, cols.sides);
    }

    fn push_truss_bar(&mut self, a: Vec3, b: Vec3, thickness: f32, up_hint: Vec3, color: [f32; 4]) {
        let dir = b - a;
        let len = dir.length();
        if len < 1e-6 {
            return;
        }
        let fwd = dir / len;
        let mut right = fwd.cross(up_hint).normalize_or_zero();
        if right.length_squared() < 1e-6 {
            right = fwd.cross(Vec3::X).normalize_or_zero();
        }
        let up = right.cross(fwd).normalize_or_zero();
        self.push_oriented_box(
            (a + b) * 0.5,
            right,
            up,
            fwd,
            Vec3::new(thickness, thickness, len * 0.5),
            Vec2::new(1.0, (len / thickness.max(0.002)).clamp(1.0, 12.0)),
            BoxColors::uniform(color),
        );
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Hexagonal prism primitive takes orientation basis, dimensions, and face/rim colors"
    )]
    fn push_hex_prism(
        &mut self,
        center: Vec3,
        right: Vec3,
        up: Vec3,
        normal: Vec3,
        radius: f32,
        half_thickness: f32,
        front_col: [f32; 4],
        back_col: [f32; 4],
        rim_col: [f32; 4],
    ) {
        let front_pts: [Vec3; 6] = std::array::from_fn(|i| {
            let ang = (i as f32 / 6.0) * TAU + std::f32::consts::FRAC_PI_6;
            let radial = right * ang.cos() + up * ang.sin();
            center + radial * radius + normal * half_thickness
        });
        let back_pts: [Vec3; 6] = std::array::from_fn(|i| {
            let ang = (i as f32 / 6.0) * TAU + std::f32::consts::FRAC_PI_6;
            let radial = right * ang.cos() + up * ang.sin();
            center + radial * (radius * 0.94) - normal * half_thickness
        });

        let front_center_idx = self.positions.len() as u32;
        self.positions
            .push((center + normal * half_thickness).to_array());
        self.normals.push(normal.to_array());
        self.uvs.push([0.5, 0.5]);
        self.colors.push(front_col);

        for (i, front_pt) in front_pts.iter().enumerate() {
            let ang = (i as f32 / 6.0) * TAU + std::f32::consts::FRAC_PI_6;
            self.positions.push(front_pt.to_array());
            self.normals.push(normal.to_array());
            self.uvs
                .push([0.5 + 0.5 * ang.cos(), 0.5 + 0.5 * ang.sin()]);
            self.colors.push(front_col);
        }
        for i in 0..6 {
            let next = (i + 1) % 6;
            if let (Some(&f0), Some(&f1)) = (front_pts.get(i), front_pts.get(next)) {
                let va = f0 - (center + normal * half_thickness);
                let vb = f1 - (center + normal * half_thickness);
                let flip = va.cross(vb).dot(normal) < 0.0;
                if flip {
                    self.indices.extend_from_slice(&[
                        front_center_idx,
                        front_center_idx + 1 + next as u32,
                        front_center_idx + 1 + i as u32,
                    ]);
                } else {
                    self.indices.extend_from_slice(&[
                        front_center_idx,
                        front_center_idx + 1 + i as u32,
                        front_center_idx + 1 + next as u32,
                    ]);
                }
            }
        }

        let back_center_idx = self.positions.len() as u32;
        self.positions
            .push((center - normal * half_thickness).to_array());
        self.normals.push((-normal).to_array());
        self.uvs.push([0.5, 0.5]);
        self.colors.push(back_col);
        for (i, back_pt) in back_pts.iter().enumerate() {
            let ang = (i as f32 / 6.0) * TAU + std::f32::consts::FRAC_PI_6;
            self.positions.push(back_pt.to_array());
            self.normals.push((-normal).to_array());
            self.uvs
                .push([0.5 + 0.5 * ang.cos(), 0.5 + 0.5 * ang.sin()]);
            self.colors.push(back_col);
        }
        for i in 0..6 {
            let next = (i + 1) % 6;
            if let (Some(&b0), Some(&b1), Some(&f0), Some(&f1)) = (
                back_pts.get(i),
                back_pts.get(next),
                front_pts.get(i),
                front_pts.get(next),
            ) {
                let va = b1 - (center - normal * half_thickness);
                let vb = b0 - (center - normal * half_thickness);
                let flip = va.cross(vb).dot(-normal) < 0.0;
                if flip {
                    self.indices.extend_from_slice(&[
                        back_center_idx,
                        back_center_idx + 1 + i as u32,
                        back_center_idx + 1 + next as u32,
                    ]);
                } else {
                    self.indices.extend_from_slice(&[
                        back_center_idx,
                        back_center_idx + 1 + next as u32,
                        back_center_idx + 1 + i as u32,
                    ]);
                }
                let side_dir = ((f0 + f1) * 0.5 - center).normalize_or_zero();
                let side_n = (side_dir - normal * side_dir.dot(normal)).normalize_or_zero();
                self.push_quad(b0, b1, f1, f0, side_n, Vec2::new(1.0, 0.25), rim_col);
            }
        }
    }

    fn finish(self) -> Mesh {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, self.colors);
        mesh.insert_indices(Indices::U32(self.indices));
        mesh
    }
}

/// Builds a 36-module robotic Dyson Swarm collector satellite & structural truss ring at unit radius.
#[must_use]
pub fn build_dyson_collector_ring_mesh() -> Mesh {
    const NODES: usize = 36;
    let mut b = TechMeshBuilder::with_capacity(NODES * 110, NODES * 80);

    let titanium = [0.16, 0.0, 0.0, 0.78];
    let solar_gold = [1.0, 0.0, 1.0, 1.0];
    let back_plate = [0.22, 0.0, 0.05, 0.72];
    let relay_core = [0.25, 0.85, 0.0, 1.0];

    for i in 0..NODES {
        let theta = (i as f32 / NODES as f32) * TAU;
        let next_theta = ((i + 1) as f32 / NODES as f32) * TAU;
        let p0 = Vec3::new(theta.cos(), 0.0, theta.sin());
        let p1 = Vec3::new(next_theta.cos(), 0.0, next_theta.sin());
        let rad_in = -p0;
        let tan = Vec3::new(-theta.sin(), 0.0, theta.cos());
        let up = Vec3::Y;

        // Central spacecraft bus & glowing microwave transmitter core
        b.push_oriented_box(
            p0,
            tan,
            up,
            rad_in,
            Vec3::new(0.011, 0.006, 0.008),
            Vec2::ONE,
            BoxColors::uniform(titanium),
        );
        b.push_oriented_box(
            p0 + rad_in * 0.007,
            tan,
            up,
            rad_in,
            Vec3::new(0.0045, 0.004, 0.004),
            Vec2::ONE,
            BoxColors::uniform(relay_core),
        );

        // Canted twin solar collector wings facing inward toward star (+Z local = rad_in)
        let wing_n = (rad_in * 0.94 + up * 0.34).normalize();
        let wing_up = wing_n.cross(tan).normalize();
        for side in [-1.0_f32, 1.0] {
            let wc = p0 + tan * (side * 0.032) + rad_in * 0.002;
            b.push_oriented_box(
                wc,
                tan,
                wing_n,
                wing_up,
                Vec3::new(0.020, 0.002, 0.017),
                Vec2::new(2.0, 1.0),
                BoxColors {
                    face_pos_y: solar_gold,
                    face_neg_y: back_plate,
                    sides: titanium,
                },
            );
            b.push_truss_bar(p0 + tan * (side * 0.009), wc, 0.0018, up, titanium);
        }

        // Twin orbital truss girders + cross-strut linking adjacent collectors
        let next_tan = Vec3::new(-next_theta.sin(), 0.0, next_theta.cos());
        let a_hi = p0 + tan * 0.010 + up * 0.004;
        let b_hi = p1 - next_tan * 0.010 + up * 0.004;
        let a_lo = p0 + tan * 0.010 - up * 0.004;
        let b_lo = p1 - next_tan * 0.010 - up * 0.004;
        b.push_truss_bar(a_hi, b_hi, 0.0018, up, titanium);
        b.push_truss_bar(a_lo, b_lo, 0.0018, up, titanium);
        b.push_truss_bar(
            a_lo,
            (a_hi + b_hi) * 0.5,
            0.0013,
            p0,
            [0.12, 0.0, 0.0, 0.68],
        );
    }

    b.finish()
}

fn append_shkadov_backshell_trusses(
    b: &mut TechMeshBuilder,
    max_theta: f32,
    seg_theta: usize,
    back_r: f32,
) {
    let titanium = [0.18, 0.0, 0.0, 0.72];
    let radiator = [0.15, 0.65, 0.0, 0.85];
    let beacon = [0.22, 1.0, 0.0, 1.0];

    // 12 raised radial structural spine ribs on convex outer hull
    for r_i in 0..12 {
        let phi = (r_i as f32 / 12.0) * TAU;
        let (sp, cp) = phi.sin_cos();
        for j in 0..seg_theta {
            let t0 = (j as f32 / seg_theta as f32) * max_theta;
            let t1 = ((j + 1) as f32 / seg_theta as f32) * max_theta;
            let p0 = Vec3::new(t0.sin() * cp, t0.cos(), t0.sin() * sp) * (back_r + 0.008);
            let p1 = Vec3::new(t1.sin() * cp, t1.cos(), t1.sin() * sp) * (back_r + 0.008);
            b.push_truss_bar(p0, p1, 0.010, p0.normalize(), titanium);
        }
    }

    // 3 concentric structural ring trusses around backshell
    for ring_j in [2_usize, 5, 8] {
        let t = (ring_j as f32 / seg_theta as f32) * max_theta;
        let (st, ct) = t.sin_cos();
        for i in 0..24 {
            let p_a = (i as f32 / 24.0) * TAU;
            let p_b = ((i + 1) as f32 / 24.0) * TAU;
            let a = Vec3::new(st * p_a.cos(), ct, st * p_a.sin()) * (back_r + 0.011);
            let c = Vec3::new(st * p_b.cos(), ct, st * p_b.sin()) * (back_r + 0.011);
            b.push_truss_bar(a, c, 0.0075, Vec3::Y, titanium);
        }
    }

    // 3 heavy counterweight pylons with thermal radiator louvers & 12 rim beacons
    for k in 0..12 {
        let phi = (k as f32 / 12.0) * TAU;
        let dir = Vec3::new(
            max_theta.sin() * phi.cos(),
            max_theta.cos(),
            max_theta.sin() * phi.sin(),
        );
        let tan = Vec3::new(-phi.sin(), 0.0, phi.cos());
        let bi_n = dir.cross(tan).normalize();
        b.push_oriented_box(
            dir * (back_r + 0.006),
            tan,
            dir,
            bi_n,
            Vec3::new(0.016, 0.010, 0.016),
            Vec2::ONE,
            BoxColors::uniform(beacon),
        );
        if k % 4 == 0 {
            let pylon_pos = dir * (back_r + 0.030);
            b.push_truss_bar(dir * back_r, pylon_pos, 0.014, tan, titanium);
            b.push_oriented_box(
                pylon_pos,
                tan,
                dir,
                bi_n,
                Vec3::new(0.042, 0.022, 0.028),
                Vec2::new(2.0, 2.0),
                BoxColors {
                    face_pos_y: radiator,
                    face_neg_y: titanium,
                    sides: titanium,
                },
            );
        }
    }
}

/// Builds a closed double-walled parabolic Shkadov Stellar Engine canopy with hex-mirror inner face,
/// armored titanium backshell, radial spine trusses, and counterweight radiator pylons.
#[must_use]
pub fn build_shkadov_engine_mesh() -> Mesh {
    const SEG_PHI: usize = 36;
    const SEG_THETA: usize = 8;
    let max_theta = 0.78_f32;
    let r_in = 1.0_f32;
    let r_out = 1.024_f32;

    let mut b = TechMeshBuilder::with_capacity(2800, 2200);
    let mirror_col = [1.0, 0.0, 1.0, 1.0];
    let back_col = [0.22, 0.0, 0.05, 0.88];
    let rim_col = [0.16, 0.0, 0.20, 0.90];

    let sample = |j: usize, i: usize, r: f32| -> (Vec3, Vec3, Vec2) {
        let v = (j as f32 / SEG_THETA as f32) * max_theta;
        let u = (i as f32 / SEG_PHI as f32) * TAU;
        let (sv, cv) = v.sin_cos();
        let (su, cu) = u.sin_cos();
        let dir = Vec3::new(sv * cu, cv, sv * su);
        (
            dir * r,
            dir,
            Vec2::new(i as f32 / SEG_PHI as f32, j as f32 / SEG_THETA as f32),
        )
    };

    for j in 0..SEG_THETA {
        for i in 0..SEG_PHI {
            let (p00, n00, _) = sample(j, i, r_in);
            let (p10, n10, _) = sample(j, i + 1, r_in);
            let (p11, n11, _) = sample(j + 1, i + 1, r_in);
            let (p01, n01, _) = sample(j + 1, i, r_in);
            let n_mid_in = -((n00 + n10 + n11 + n01) * 0.25).normalize();
            b.push_quad(p00, p01, p11, p10, n_mid_in, Vec2::ONE, mirror_col);

            let (o00, no00, _) = sample(j, i, r_out);
            let (o10, no10, _) = sample(j, i + 1, r_out);
            let (o11, no11, _) = sample(j + 1, i + 1, r_out);
            let (o01, no01, _) = sample(j + 1, i, r_out);
            let n_mid_out = ((no00 + no10 + no11 + no01) * 0.25).normalize();
            b.push_quad(o00, o10, o11, o01, n_mid_out, Vec2::ONE, back_col);
        }
    }

    // Closed outer perimeter rim collar
    for i in 0..SEG_PHI {
        let (in0, n0, _) = sample(SEG_THETA, i, r_in);
        let (in1, n1, _) = sample(SEG_THETA, i + 1, r_in);
        let (out0, _, _) = sample(SEG_THETA, i, r_out);
        let (out1, _, _) = sample(SEG_THETA, i + 1, r_out);
        let rim_tangent_down = Vec3::new(
            max_theta.cos() * n0.x,
            -max_theta.sin(),
            max_theta.cos() * n0.z,
        )
        .normalize();
        let _ = n1;
        b.push_quad(
            in0,
            in1,
            out1,
            out0,
            rim_tangent_down,
            Vec2::new(1.0, 0.25),
            rim_col,
        );
    }

    append_shkadov_backshell_trusses(&mut b, max_theta, SEG_THETA, r_out);
    b.finish()
}

/// Builds a titanium Orbital Habitat Ring with glowing viewport deck bands, mag-rail trusses, and 8 station docking hubs.
#[must_use]
pub fn build_orbital_habitat_ring_mesh() -> Mesh {
    const SEGS: usize = 48;
    let mut b = TechMeshBuilder::with_capacity(SEGS * 90, SEGS * 68);

    let hull_metal = [0.85, 0.0, 0.05, 0.94];
    let deck_lit = [0.80, 0.65, 0.0, 0.96];
    let truss_dark = [0.16, 0.0, 0.0, 0.72];
    let solar_wing = [1.0, 0.0, 1.0, 1.0];
    let beacon = [0.20, 1.0, 0.0, 1.0];

    for i in 0..SEGS {
        let t0 = (i as f32 / SEGS as f32) * TAU;
        let t1 = ((i + 1) as f32 / SEGS as f32) * TAU;
        let tm = f32::midpoint(t0, t1);
        let chord = Vec3::new(t1.cos() - t0.cos(), 0.0, t1.sin() - t0.sin()).length();
        let c = Vec3::new(tm.cos(), 0.0, tm.sin());
        let rad = c.normalize();
        let tan = Vec3::new(-tm.sin(), 0.0, tm.cos());

        b.push_oriented_box(
            c,
            rad,
            Vec3::Y,
            tan,
            Vec3::new(0.011, 0.0065, chord * 0.51),
            Vec2::new(1.0, 2.0),
            BoxColors {
                face_pos_y: hull_metal,
                face_neg_y: hull_metal,
                sides: deck_lit,
            },
        );

        let a_in = Vec3::new(t0.cos() * 0.976, 0.0, t0.sin() * 0.976);
        let b_in = Vec3::new(t1.cos() * 0.976, 0.0, t1.sin() * 0.976);
        let a_out = Vec3::new(t0.cos() * 1.024, 0.0, t0.sin() * 1.024);
        let b_out = Vec3::new(t1.cos() * 1.024, 0.0, t1.sin() * 1.024);
        b.push_truss_bar(a_in, b_in, 0.0022, Vec3::Y, truss_dark);
        b.push_truss_bar(a_out, b_out, 0.0022, Vec3::Y, truss_dark);
        if i % 2 == 0 {
            b.push_truss_bar(c * 0.976, c * 1.024, 0.0019, Vec3::Y, truss_dark);
        }

        if i % 6 == 0 {
            b.push_oriented_box(
                c,
                rad,
                Vec3::Y,
                tan,
                Vec3::new(0.026, 0.014, 0.034),
                Vec2::new(2.0, 1.5),
                BoxColors::uniform([0.45, 0.0, 0.05, 0.85]),
            );
            b.push_truss_bar(
                c + Vec3::Y * 0.013,
                c + Vec3::Y * 0.042,
                0.0025,
                rad,
                truss_dark,
            );
            b.push_oriented_box(
                c + Vec3::Y * 0.044,
                rad,
                Vec3::Y,
                tan,
                Vec3::new(0.0045, 0.004, 0.0045),
                Vec2::ONE,
                BoxColors::uniform(beacon),
            );
            for side in [-1.0_f32, 1.0] {
                let fin_c = c + Vec3::Y * (side * 0.028) + rad * 0.016;
                b.push_oriented_box(
                    fin_c,
                    rad,
                    tan,
                    Vec3::Y,
                    Vec3::new(0.016, 0.0016, 0.009),
                    Vec2::new(2.0, 1.0),
                    BoxColors {
                        face_pos_y: solar_wing,
                        face_neg_y: truss_dark,
                        sides: truss_dark,
                    },
                );
            }
        }
    }

    b.finish()
}

/// Builds 8 free-flying JWST-style beveled gold-Kapton hexagonal heliostat mirror satellites in orbital formation.
#[must_use]
pub fn build_orbital_mirror_array_mesh() -> Mesh {
    const SATS: usize = 8;
    let mut b = TechMeshBuilder::with_capacity(SATS * 150, SATS * 120);

    let mirror_gold = [1.0, 0.0, 1.0, 1.0];
    let back_dark = [0.16, 0.0, 0.05, 0.78];
    let rim_metal = [0.55, 0.0, 0.25, 0.90];
    let truss_dark = [0.14, 0.0, 0.0, 0.72];
    let beacon = [0.22, 1.0, 0.0, 1.0];

    for i in 0..SATS {
        let t0 = (i as f32 / SATS as f32) * TAU;
        let p0 = Vec3::new(t0.cos(), 0.0, t0.sin());
        let rad_in = -p0;
        let tan = Vec3::new(-t0.sin(), 0.0, t0.cos());
        let mirror_n = (rad_in * 0.88 + Vec3::Y * 0.47).normalize();
        let mirror_up = mirror_n.cross(tan).normalize();

        // Main beveled hexagonal gold mirror collector
        b.push_hex_prism(
            p0,
            tan,
            mirror_up,
            mirror_n,
            0.092,
            0.010,
            mirror_gold,
            back_dark,
            rim_metal,
        );

        // Rear gimbal actuator pod + 4 X-frame stabilization struts
        let pod_center = p0 - mirror_n * 0.048;
        b.push_oriented_box(
            pod_center,
            tan,
            mirror_up,
            mirror_n,
            Vec3::new(0.022, 0.018, 0.018),
            Vec2::ONE,
            BoxColors::uniform(truss_dark),
        );
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                let anchor = p0 - mirror_n * 0.008 + (tan * sx + mirror_up * sy) * 0.052;
                b.push_truss_bar(pod_center, anchor, 0.0045, mirror_n, truss_dark);
            }
        }

        // Dorsal attitude thruster pod + cyan alignment LED
        let top_beacon = p0 + Vec3::Y * 0.102;
        b.push_truss_bar(pod_center, top_beacon, 0.0035, tan, truss_dark);
        b.push_oriented_box(
            top_beacon,
            tan,
            Vec3::Y,
            rad_in,
            Vec3::new(0.008, 0.007, 0.008),
            Vec2::ONE,
            BoxColors::uniform(beacon),
        );
    }

    b.finish()
}
