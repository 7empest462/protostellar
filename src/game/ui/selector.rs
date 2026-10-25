//! Dynamic body selector bar, celestial world palette classification, and custom planet builder spawning.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::probes::{ProbeStatus, ProbeType, SpaceProbe};
use crate::simulation::resources::*;

use super::types::*;

pub fn body_to_button_label(name: &str, body_type: BodyType, is_star: bool) -> String {
    let lower = name.to_lowercase();
    if lower.contains("little red dot") || body_type == BodyType::QuasiStar {
        "🔴 Little Red Dot".to_string()
    } else if lower.contains("trappist-1") {
        if is_star || body_type.is_star_or_remnant() {
            "TRAPPIST-1".to_string()
        } else if lower.contains("trappist-1e") {
            "1e (HZ)".to_string()
        } else if lower.contains("trappist-1f") {
            "1f (HZ)".to_string()
        } else if lower.contains("trappist-1g") {
            "1g (HZ)".to_string()
        } else if lower.contains("trappist-1b") {
            "1b".to_string()
        } else if lower.contains("trappist-1c") {
            "1c".to_string()
        } else if lower.contains("trappist-1d") {
            "1d".to_string()
        } else if lower.contains("trappist-1h") {
            "1h".to_string()
        } else if let Some(suffix) = lower.strip_prefix("trappist-1") {
            format!("1{}", suffix.trim())
        } else {
            name.to_string()
        }
    } else if lower.contains("kepler-16") {
        if lower.contains("kepler-16a") {
            "Kepler-16A".to_string()
        } else if lower.contains("kepler-16b (secondary")
            || lower.contains("kepler-16b (secondary m-dwarf)")
            || (is_star && lower.contains("16b"))
        {
            "Kepler-16B".to_string()
        } else if lower.contains("kepler-16b") {
            "Kepler-16b".to_string()
        } else {
            name.to_string()
        }
    } else if lower.contains("nemesis") || lower.contains("rogue") {
        "Nemesis-X".to_string()
    } else if lower.contains("hot jupiter") || lower.contains("hd 209458") {
        "Hot Jupiter".to_string()
    } else if lower.contains("proto-earth") || (lower.contains("earth") && lower.contains("inner"))
    {
        "Proto-Earth".to_string()
    } else if lower.contains("proto-jupiter") {
        "Proto-Jupiter".to_string()
    } else if lower.contains("proto-mercury") {
        "Mercury".to_string()
    } else if lower.contains("proto-venus") {
        "Venus".to_string()
    } else if lower.contains("proto-mars") {
        "Mars".to_string()
    } else if lower.contains("theia") {
        "🌑 Theia".to_string()
    } else if lower.contains("proto-saturn") {
        "Saturn".to_string()
    } else if lower.contains("proto-uranus") {
        "Uranus".to_string()
    } else if lower.contains("proto-neptune") {
        "Neptune".to_string()
    } else if lower.contains("pluto") {
        "🧊 Pluto".to_string()
    } else if lower.contains("planet nine") || lower.contains("planet 9") {
        "🪐 Planet 9".to_string()
    } else if is_embryo_body(name, body_type) {
        if lower.contains("theia") {
            "Theia".to_string()
        } else if let Some(suffix) = name.strip_prefix("Embryo-") {
            format!("🌱 {suffix}")
        } else if let Some(suffix) = name.strip_prefix("Embryo #") {
            format!("🌱 #{suffix}")
        } else {
            format!("🌱 {name}")
        }
    } else if lower.contains("kuiper") {
        "Kuiper".to_string()
    } else if lower.contains("ceres") {
        "Ceres".to_string()
    } else if let Some(suffix) = name.strip_prefix("Protostar Jeans-") {
        format!("✨ Jeans-{suffix}")
    } else if let Some(suffix) = name.strip_prefix("Protostar ") {
        let clean = suffix.split('(').next().unwrap_or(suffix).trim();
        format!("✨ {clean}")
    } else if lower.contains("genesis") {
        "Genesis Protostar".to_string()
    } else if is_star && lower.contains("sun") {
        "Sun".to_string()
    } else if lower.contains("protostar") {
        "✨ Protostar".to_string()
    } else if lower.contains("host star") {
        "Host Star".to_string()
    } else if lower.starts_with("asteroid-") {
        name.strip_prefix("Asteroid-")
            .map_or_else(|| name.to_string(), |s| format!("Ast-{s}"))
    } else if lower.starts_with("dust-") {
        name.strip_prefix("Dust-")
            .map_or_else(|| name.to_string(), |s| format!("Dust-{s}"))
    } else if let Some(prefix) = name.get(..12) {
        if name.len() > 14 {
            format!("{prefix}…")
        } else {
            name.to_string()
        }
    } else if name.chars().count() > 14 {
        let prefix: String = name.chars().take(12).collect();
        format!("{prefix}…")
    } else {
        name.to_string()
    }
}

pub fn body_to_button_colors(name: &str, body_type: BodyType, is_star: bool) -> (Color, Color) {
    let lower = name.to_lowercase();
    if is_star || body_type.is_star_or_remnant() {
        if lower.contains("little red dot") || body_type == BodyType::QuasiStar {
            (
                Color::srgba(0.28, 0.04, 0.08, 0.95),
                Color::srgb(1.0, 0.35, 0.45),
            )
        } else if lower.contains("red") || lower.contains("trappist") || lower.contains("16b") {
            (
                Color::srgba(0.28, 0.08, 0.04, 0.9),
                Color::srgb(1.0, 0.45, 0.3),
            )
        } else {
            (
                Color::srgba(0.28, 0.20, 0.04, 0.9),
                Color::srgb(1.0, 0.8, 0.2),
            )
        }
    } else if lower.contains("hz")
        || lower.contains("earth")
        || lower.contains("habitable")
        || lower.contains("1e")
        || lower.contains("1f")
        || lower.contains("1g")
    {
        (
            Color::srgba(0.04, 0.22, 0.16, 0.9),
            Color::srgb(0.3, 0.9, 0.6),
        )
    } else if lower.contains("rogue") || lower.contains("nemesis") {
        (
            Color::srgba(0.28, 0.05, 0.12, 0.95),
            Color::srgb(1.0, 0.3, 0.5),
        )
    } else if lower.contains("pluto") {
        (
            Color::srgba(0.10, 0.14, 0.22, 0.9),
            Color::srgb(0.7, 0.85, 0.95),
        )
    } else if lower.contains("planet nine") || lower.contains("planet 9") {
        (
            Color::srgba(0.04, 0.12, 0.25, 0.9),
            Color::srgb(0.2, 0.75, 0.9),
        )
    } else if lower.contains("theia") {
        (
            Color::srgba(0.24, 0.12, 0.10, 0.95),
            Color::srgb(1.0, 0.65, 0.45),
        )
    } else if is_embryo_body(name, body_type) {
        (
            Color::srgba(0.06, 0.18, 0.10, 0.9),
            Color::srgb(0.3, 0.95, 0.65),
        )
    } else if lower.contains("hot jupiter")
        || body_type == BodyType::GasGiant
        || lower.contains("jupiter")
        || lower.contains("saturn")
    {
        (
            Color::srgba(0.20, 0.12, 0.06, 0.9),
            Color::srgb(0.95, 0.65, 0.3),
        )
    } else if body_type == BodyType::IceGiant
        || lower.contains("neptune")
        || lower.contains("uranus")
        || lower.contains("kuiper")
    {
        (
            Color::srgba(0.06, 0.14, 0.26, 0.9),
            Color::srgb(0.4, 0.7, 1.0),
        )
    } else if body_type == BodyType::Comet || lower.contains("comet") {
        (
            Color::srgba(0.05, 0.14, 0.24, 0.90),
            Color::srgb(0.4, 0.8, 1.0),
        )
    } else if body_type == BodyType::Asteroid
        || body_type == BodyType::Planetesimal
        || lower.contains("asteroid")
        || lower.contains("ceres")
        || lower.contains("vesta")
        || lower.contains("pallas")
    {
        (
            Color::srgba(0.18, 0.12, 0.08, 0.90),
            Color::srgb(0.85, 0.65, 0.35),
        )
    } else {
        (
            Color::srgba(0.08, 0.12, 0.18, 0.9),
            Color::srgb(0.5, 0.7, 0.85),
        )
    }
}

pub fn body_to_button_label_and_colors(
    name: &str,
    body_type: BodyType,
    is_star: bool,
) -> (String, Color, Color) {
    let label = body_to_button_label(name, body_type, is_star);
    let (bg, border) = body_to_button_colors(name, body_type, is_star);
    (label, bg, border)
}

#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
fn collect_embryo_and_minor_bodies(
    bodies: impl Iterator<Item = (Entity, CelestialBody, SimPosition, Mass, Radius, bool)>,
    sort_by_mass: bool,
) -> (
    Vec<(Entity, String, BodyType, bool, f64, f64)>,
    Vec<(Entity, String, BodyType, bool, f64, f64)>,
) {
    let mut embryo_bodies = Vec::new();
    let mut minor_bodies = Vec::new();

    for (ent, body, pos, mass, _radius, is_star) in bodies {
        let dist = if pos.0.is_finite() {
            pos.0.length()
        } else {
            0.0
        };
        if is_embryo_body(&body.name, body.body_type) {
            embryo_bodies.push((
                ent,
                body.name.clone(),
                body.body_type,
                is_star,
                dist,
                mass.0,
            ));
        } else if !is_major_body(&body.name, body.body_type, is_star, mass.0) {
            minor_bodies.push((
                ent,
                body.name.clone(),
                body.body_type,
                is_star,
                dist,
                mass.0,
            ));
        }
    }

    if sort_by_mass {
        embryo_bodies.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap_or(std::cmp::Ordering::Equal));
        minor_bodies.sort_by(|a, b| b.5.partial_cmp(&a.5).unwrap_or(std::cmp::Ordering::Equal));
    } else {
        embryo_bodies.sort_by(|a, b| a.4.partial_cmp(&b.4).unwrap_or(std::cmp::Ordering::Equal));
        minor_bodies.sort_by(|a, b| a.4.partial_cmp(&b.4).unwrap_or(std::cmp::Ordering::Equal));
    }

    (embryo_bodies, minor_bodies)
}

fn populate_minimized_quick_bar(
    btn_row: &mut ChildSpawnerCommands,
    major_count: usize,
    embryo_count: usize,
    minor_count: usize,
) {
    let mut parts = vec![format!("🪐 Worlds ({major_count})")];
    if embryo_count > 0 {
        parts.push(format!("🌱 Embryos ({embryo_count})"));
    }
    if minor_count > 0 {
        parts.push(format!("☄️ Minor ({minor_count})"));
    }
    let label = format!("{} ▼ Expand [H]", parts.join(" | "));
    create_compact_button(
        btn_row,
        UiButtonAction::ToggleMinimizeQuickBar,
        &label,
        Color::srgba(0.08, 0.14, 0.24, 0.90),
        Color::srgb(0.4, 0.8, 1.0),
    );
    create_compact_button(
        btn_row,
        UiButtonAction::CycleTarget,
        "Cycle [Tab]",
        Color::srgba(0.12, 0.16, 0.26, 0.85),
        Color::srgb(0.5, 0.7, 1.0),
    );
}

fn populate_expanded_quick_bar(
    btn_row: &mut ChildSpawnerCommands,
    major_worlds: &[SystemWorld],
    embryo_bodies: &[(Entity, String, BodyType, bool, f64, f64)],
    minor_bodies: &[(Entity, String, BodyType, bool, f64, f64)],
    probes: &[(Entity, ProbeType, ProbeStatus, String)],
    selected_entity: Option<Entity>,
    quick_bar_state: &QuickBarState,
) {
    create_compact_button(
        btn_row,
        UiButtonAction::ToggleMinimizeQuickBar,
        "🗕 [H]",
        Color::srgba(0.16, 0.08, 0.12, 0.85),
        Color::srgb(0.9, 0.4, 0.6),
    );

    let sort_label = if quick_bar_state.sort_by_mass {
        "⚖️ Sort: Mass"
    } else {
        "📏 Sort: Dist"
    };
    create_compact_button(
        btn_row,
        UiButtonAction::ToggleListSortMode,
        sort_label,
        Color::srgba(0.12, 0.16, 0.22, 0.85),
        Color::srgb(0.5, 0.7, 1.0),
    );

    populate_major_worlds(btn_row, major_worlds, selected_entity);
    populate_probes(btn_row, probes, selected_entity);
    populate_embryos(btn_row, embryo_bodies, selected_entity, quick_bar_state);
    populate_minor_body_belts(btn_row, minor_bodies, selected_entity, quick_bar_state);

    create_compact_button(
        btn_row,
        UiButtonAction::CycleTarget,
        "Cycle [Tab]",
        Color::srgba(0.12, 0.16, 0.26, 0.85),
        Color::srgb(0.5, 0.7, 1.0),
    );
}

fn populate_major_worlds(
    btn_row: &mut ChildSpawnerCommands,
    major_worlds: &[SystemWorld],
    selected_entity: Option<Entity>,
) {
    for world in major_worlds {
        let (raw_label, base_bg, base_border) =
            body_to_button_label_and_colors(&world.name, world.body_type, world.is_central_star);

        let button_label = if world.is_central_star {
            format!("[0] {raw_label}")
        } else {
            format!("[{}] {raw_label}", world.index)
        };

        let is_selected = selected_entity == Some(world.entity);
        let bg_color = if is_selected {
            Color::srgba(0.20, 0.45, 0.85, 0.95)
        } else {
            base_bg
        };
        let border_color = if is_selected {
            Color::srgb(1.0, 1.0, 1.0)
        } else {
            base_border
        };

        create_compact_selectable_button(
            btn_row,
            UiButtonAction::SelectEntity(world.entity),
            &button_label,
            bg_color,
            border_color,
            base_bg,
            base_border,
        );
    }
}

fn populate_probes(
    btn_row: &mut ChildSpawnerCommands,
    probes: &[(Entity, ProbeType, ProbeStatus, String)],
    selected_entity: Option<Entity>,
) {
    for (probe_ent, probe_type, probe_status, probe_name) in probes {
        let (base_bg, base_border) = match probe_type {
            ProbeType::Orbiter => (
                Color::srgba(0.06, 0.22, 0.38, 0.92),
                Color::srgb(0.3, 0.85, 1.0),
            ),
            ProbeType::Rover => (
                Color::srgba(0.35, 0.20, 0.05, 0.92),
                Color::srgb(1.0, 0.75, 0.25),
            ),
        };
        let label = if *probe_status == ProbeStatus::Landed {
            probe_name.replacen("🚀", "🛬", 1)
        } else {
            probe_name.clone()
        };
        let is_selected = selected_entity == Some(*probe_ent);
        let bg_color = if is_selected {
            Color::srgba(0.20, 0.45, 0.85, 0.95)
        } else {
            base_bg
        };
        let border_color = if is_selected {
            Color::srgb(1.0, 1.0, 1.0)
        } else {
            base_border
        };
        create_compact_selectable_button(
            btn_row,
            UiButtonAction::SelectEntity(*probe_ent),
            &label,
            bg_color,
            border_color,
            base_bg,
            base_border,
        );
    }
    if probes.iter().any(|(_, pt, _, _)| *pt == ProbeType::Rover) {
        create_compact_button(
            btn_row,
            UiButtonAction::ToggleSurfaceHorizonCamera,
            "🎥 Rover Cam [Shift-V]",
            Color::srgba(0.06, 0.24, 0.18, 0.92),
            Color::srgb(0.35, 1.0, 0.78),
        );
    }
}

fn populate_embryos(
    btn_row: &mut ChildSpawnerCommands,
    embryo_bodies: &[(Entity, String, BodyType, bool, f64, f64)],
    selected_entity: Option<Entity>,
    quick_bar_state: &QuickBarState,
) {
    if !embryo_bodies.is_empty() {
        if quick_bar_state.show_embryos {
            for (ent, name, b_type, is_star, _, _) in embryo_bodies {
                let (raw_label, base_bg, base_border) =
                    body_to_button_label_and_colors(name, *b_type, *is_star);
                let is_selected = selected_entity == Some(*ent);
                let bg_color = if is_selected {
                    Color::srgba(0.20, 0.45, 0.85, 0.95)
                } else {
                    base_bg
                };
                let border_color = if is_selected {
                    Color::srgb(1.0, 1.0, 1.0)
                } else {
                    base_border
                };
                create_compact_selectable_button(
                    btn_row,
                    UiButtonAction::SelectEntity(*ent),
                    &raw_label,
                    bg_color,
                    border_color,
                    base_bg,
                    base_border,
                );
            }
            create_compact_button(
                btn_row,
                UiButtonAction::ToggleEmbryos,
                "🌱 Hide",
                Color::srgba(0.12, 0.08, 0.16, 0.85),
                Color::srgb(0.8, 0.5, 0.9),
            );
        } else {
            let label = format!("🌱 +{}", embryo_bodies.len());
            create_compact_button(
                btn_row,
                UiButtonAction::ToggleEmbryos,
                &label,
                Color::srgba(0.06, 0.14, 0.10, 0.85),
                Color::srgb(0.3, 0.8, 0.5),
            );
        }
    }
}

fn populate_minor_body_belts(
    btn_row: &mut ChildSpawnerCommands,
    minor_bodies: &[(Entity, String, BodyType, bool, f64, f64)],
    selected_entity: Option<Entity>,
    quick_bar_state: &QuickBarState,
) {
    if minor_bodies.is_empty() {
        return;
    }

    if !quick_bar_state.show_minor_bodies {
        let label = format!("☄️ Belts +{}", minor_bodies.len());
        create_compact_button(
            btn_row,
            UiButtonAction::ToggleMinorBodies,
            &label,
            Color::srgba(0.10, 0.08, 0.14, 0.85),
            Color::srgb(0.65, 0.5, 0.85),
        );
        return;
    }

    create_compact_button(
        btn_row,
        UiButtonAction::ToggleMinorBodies,
        "☄️ Belts Hide",
        Color::srgba(0.12, 0.08, 0.16, 0.85),
        Color::srgb(0.8, 0.5, 0.9),
    );

    let all_expanded = BeltZone::all()
        .iter()
        .all(|z| quick_bar_state.expanded_belts.contains(z));
    if all_expanded {
        create_compact_button(
            btn_row,
            UiButtonAction::CollapseAllBelts,
            "⇱ All",
            Color::srgba(0.10, 0.08, 0.14, 0.80),
            Color::srgb(0.70, 0.60, 0.85),
        );
    } else {
        create_compact_button(
            btn_row,
            UiButtonAction::ExpandAllBelts,
            "⇲ All",
            Color::srgba(0.10, 0.08, 0.14, 0.80),
            Color::srgb(0.70, 0.60, 0.85),
        );
    }

    for zone in BeltZone::all() {
        let zone_members: Vec<_> = minor_bodies
            .iter()
            .filter(|b| BeltZone::from_distance_au(b.4) == *zone)
            .collect();
        if zone_members.is_empty() {
            continue;
        }

        let is_expanded = quick_bar_state.expanded_belts.contains(zone);
        let (base_bg, base_border) = zone.button_colors();

        if is_expanded {
            let label = format!(
                "▼ {} {}: {}",
                zone.icon(),
                zone.short_name(),
                zone_members.len()
            );
            create_compact_button(
                btn_row,
                UiButtonAction::ToggleBelt(*zone),
                &label,
                Color::srgba(0.18, 0.22, 0.30, 0.95),
                Color::srgb(1.0, 1.0, 1.0),
            );

            for (ent, name, b_type, is_star, _, _) in zone_members {
                let (raw_label, b_bg, b_border) =
                    body_to_button_label_and_colors(name, *b_type, *is_star);
                let is_selected = selected_entity == Some(*ent);
                let bg = if is_selected {
                    Color::srgba(0.20, 0.45, 0.85, 0.95)
                } else {
                    b_bg
                };
                let border = if is_selected {
                    Color::srgb(1.0, 1.0, 1.0)
                } else {
                    b_border
                };
                create_compact_selectable_button(
                    btn_row,
                    UiButtonAction::SelectEntity(*ent),
                    &raw_label,
                    bg,
                    border,
                    b_bg,
                    b_border,
                );
            }
        } else {
            let label = format!(
                "▶ {} {}: {}",
                zone.icon(),
                zone.short_name(),
                zone_members.len()
            );
            create_compact_button(
                btn_row,
                UiButtonAction::ToggleBelt(*zone),
                &label,
                base_bg,
                base_border,
            );
        }
    }
}

type QuickBarCacheState = (
    Vec<Entity>,
    usize,
    usize,
    Vec<(Entity, ProbeStatus)>,
    bool,
    bool,
    bool,
    Vec<BeltZone>,
);

/// Dynamically updates background and border highlighting on Quick Bar buttons when selection changes
/// without needing to despawn and reconstruct the UI tree.
pub fn update_quick_bar_highlights(
    player_state: Res<PlayerInteractionState>,
    mut btn_query: Query<
        (
            &UiButtonAction,
            &QuickBarButtonBaseColor,
            &mut BackgroundColor,
            &mut BorderColor,
        ),
        With<Button>,
    >,
) {
    if !player_state.is_changed() {
        return;
    }
    let sel = player_state.selected_entity;
    for (action, base, mut bg, mut border) in btn_query.iter_mut() {
        if let UiButtonAction::SelectEntity(ent) = action {
            if Some(*ent) == sel {
                *bg = BackgroundColor(Color::srgba(0.20, 0.45, 0.85, 0.95));
                *border = BorderColor::all(Color::srgb(1.0, 1.0, 1.0));
            } else {
                *bg = BackgroundColor(base.bg);
                *border = BorderColor::all(base.border);
            }
        }
    }
}

#[allow(clippy::type_complexity, reason = "bevy ECS query is complex")]
pub fn update_quick_body_selector_bar(
    mut commands: Commands,
    bar_query: Query<Entity, With<QuickBodySelectorBar>>,
    bodies_query: Query<
        (
            Entity,
            &CelestialBody,
            &SimPosition,
            &Mass,
            &Radius,
            Option<&CentralStar>,
        ),
        With<CelestialBody>,
    >,
    probes_query: Query<(Entity, &SpaceProbe, &CelestialBody)>,
    mut quick_bar_state: ResMut<QuickBarState>,
    player_state: Res<PlayerInteractionState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut last_state: Local<QuickBarCacheState>,
) {
    let Ok(bar_ent) = bar_query.single() else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyH) {
        quick_bar_state.is_minimized = !quick_bar_state.is_minimized;
    }

    let (embryo_bodies, minor_bodies) = collect_embryo_and_minor_bodies(
        bodies_query.iter().map(|(e, b, p, m, r, s)| {
            (
                e,
                b.clone(),
                *p,
                *m,
                *r,
                s.is_some() || b.body_type.is_star_or_remnant(),
            )
        }),
        quick_bar_state.sort_by_mass,
    );

    let major_worlds =
        collect_sorted_system_worlds(bodies_query.iter(), quick_bar_state.sort_by_mass);
    let probes: Vec<(Entity, ProbeType, ProbeStatus, String)> = probes_query
        .iter()
        .map(|(e, p, b)| (e, p.probe_type, p.status, b.name.clone()))
        .collect();
    let probe_states: Vec<(Entity, ProbeStatus)> =
        probes.iter().map(|(e, _, s, _)| (*e, *s)).collect();

    let current_entities: Vec<Entity> = major_worlds.iter().map(|b| b.entity).collect();
    let current_state = (
        current_entities,
        embryo_bodies.len(),
        minor_bodies.len(),
        probe_states,
        quick_bar_state.is_minimized,
        quick_bar_state.show_embryos,
        quick_bar_state.show_minor_bodies,
        quick_bar_state
            .expanded_belts
            .iter()
            .copied()
            .collect::<Vec<_>>(),
    );

    if *last_state == current_state && !major_worlds.is_empty() {
        return;
    }

    *last_state = current_state;

    if let Ok(mut bar_cmd) = commands.get_entity(bar_ent) {
        bar_cmd.despawn_children();
        bar_cmd.with_children(|btn_row| {
            if quick_bar_state.is_minimized {
                populate_minimized_quick_bar(
                    btn_row,
                    major_worlds.len(),
                    embryo_bodies.len(),
                    minor_bodies.len(),
                );
            } else {
                populate_expanded_quick_bar(
                    btn_row,
                    &major_worlds,
                    &embryo_bodies,
                    &minor_bodies,
                    &probes,
                    player_state.selected_entity,
                    &quick_bar_state,
                );
            }
        });
    }
}

pub use super::builder_spawner::{
    compute_builder_spawn_coords, resolve_builder_body_type_and_temperature,
    spawn_custom_builder_world, update_planet_builder_ui,
};
