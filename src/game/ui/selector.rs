//! Dynamic body selector bar, celestial world palette classification, and custom planet builder spawning.

use bevy::prelude::*;

use crate::simulation::components::*;
use crate::simulation::probes::{ProbeStatus, ProbeType, SpaceProbe};
use crate::simulation::resources::*;

use crate::utils::math::{
    contains_any_ignore_ascii_case, contains_ignore_ascii_case, eq_any_ignore_ascii_case,
    starts_with_ignore_ascii_case, strip_prefix_ignore_ascii_case,
};

use super::types::*;

fn format_moon_button_label(name: &str) -> String {
    let trimmed = name.trim();
    if eq_any_ignore_ascii_case(trimmed, &["the moon", "moon"]) {
        return "🌙 The Moon".to_string();
    }
    // Strip trailing "(Moon)" and parenthetical mass/class tags like "(1.5 M_J)" or "(Gas Giant)"
    let base = name
        .strip_suffix("(Moon)")
        .or_else(|| name.strip_suffix("(moon)"))
        .unwrap_or(name)
        .trim();
    let mut compact = String::with_capacity(base.len());
    let mut depth = 0usize;
    for ch in base.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => compact.push(ch),
            _ => {}
        }
    }
    let compact = compact
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("Super-Jupiter", "Sup-Jup")
        .replace("Captured ", "")
        .replace("Planet-", "Pl-");
    let full = format!("🌙 {compact}");
    if full.chars().count() > 14 {
        let trunc: String = full.chars().take(13).collect();
        format!("{trunc}…")
    } else {
        full
    }
}

fn format_super_jupiter_button_label(name: &str) -> String {
    if let (Some(start), Some(end)) = (name.find('('), name.find(')'))
        && let Some(raw_tag) = name.get(start + 1..end)
    {
        let tag = raw_tag.trim().replace(" M_J", "J");
        let candidate = format!("🪐 Sup-Jup {tag}");
        if candidate.chars().count() <= 16 {
            return candidate;
        }
    }
    "🪐 Super-Jup".to_string()
}

fn strip_trailing_classification_tag(name: &str) -> &str {
    let trimmed = name.trim();
    if trimmed.ends_with(')')
        && let Some(idx) = trimmed.rfind(" (")
    {
        return trimmed.get(..idx).map_or(trimmed, str::trim);
    }
    trimmed
}

fn format_the_star_label(name: &str) -> Option<&'static str> {
    const STAR_TAGS: &[(&str, &str)] = &[
        ("black dwarf", "🌑 Black Dwarf"),
        ("white dwarf", "⚪ White Dwarf"),
        ("planetary nebula", "💨 Nebula Core"),
        ("carbon star", "🔴 Carbon Star"),
        ("agb", "🔥 AGB Giant"),
        ("red supergiant", "🔴 Supergiant"),
        ("red giant", "🔴 Red Giant"),
        ("hypergiant", "💥 Hypergiant"),
        ("yellow dwarf", "☀️ Yellow Dwarf"),
        ("blue supergiant", "🔵 Supergiant"),
        ("pulsar", "⚡ Pulsar"),
        ("magnetar", "🧲 Magnetar"),
        ("black hole", "🕳️ Black Hole"),
    ];
    STAR_TAGS
        .iter()
        .find(|(k, _)| contains_ignore_ascii_case(name, k))
        .map(|(_, label)| *label)
}

pub fn body_to_button_label(name: &str, body_type: BodyType, is_star: bool) -> String {
    if body_type == BodyType::Moon || contains_ignore_ascii_case(name, "(moon)") {
        return format_moon_button_label(name);
    }
    if starts_with_ignore_ascii_case(name, "super-jupiter") && body_type == BodyType::GasGiant {
        return format_super_jupiter_button_label(name);
    }
    if contains_ignore_ascii_case(name, "little red dot") || body_type == BodyType::QuasiStar {
        "🔴 Little Red Dot".to_string()
    } else if contains_ignore_ascii_case(name, "trappist-1") {
        if is_star || body_type.is_star_or_remnant() {
            "TRAPPIST-1".to_string()
        } else if contains_ignore_ascii_case(name, "trappist-1e") {
            "1e (HZ)".to_string()
        } else if contains_ignore_ascii_case(name, "trappist-1f") {
            "1f (HZ)".to_string()
        } else if contains_ignore_ascii_case(name, "trappist-1g") {
            "1g (HZ)".to_string()
        } else if contains_ignore_ascii_case(name, "trappist-1b") {
            "1b".to_string()
        } else if contains_ignore_ascii_case(name, "trappist-1c") {
            "1c".to_string()
        } else if contains_ignore_ascii_case(name, "trappist-1d") {
            "1d".to_string()
        } else if contains_ignore_ascii_case(name, "trappist-1h") {
            "1h".to_string()
        } else if let Some(suffix) = strip_prefix_ignore_ascii_case(name, "trappist-1") {
            format!("1{}", suffix.trim())
        } else {
            name.to_string()
        }
    } else if contains_ignore_ascii_case(name, "kepler-16") {
        if contains_ignore_ascii_case(name, "kepler-16a") {
            "Kepler-16A".to_string()
        } else if contains_any_ignore_ascii_case(
            name,
            &["kepler-16b (secondary", "kepler-16b (secondary m-dwarf)"],
        ) || (is_star && contains_ignore_ascii_case(name, "16b"))
        {
            "Kepler-16B".to_string()
        } else if contains_ignore_ascii_case(name, "kepler-16b") {
            "Kepler-16b".to_string()
        } else {
            name.to_string()
        }
    } else if contains_any_ignore_ascii_case(name, &["nemesis", "rogue"]) {
        "Nemesis-X".to_string()
    } else if contains_any_ignore_ascii_case(name, &["hot jupiter", "hd 209458"]) {
        "Hot Jupiter".to_string()
    } else if contains_ignore_ascii_case(name, "proto-earth")
        || (contains_ignore_ascii_case(name, "earth") && contains_ignore_ascii_case(name, "inner"))
    {
        "Proto-Earth".to_string()
    } else if contains_ignore_ascii_case(name, "proto-jupiter") {
        "Proto-Jupiter".to_string()
    } else if contains_ignore_ascii_case(name, "proto-mercury") {
        "Mercury".to_string()
    } else if contains_ignore_ascii_case(name, "proto-venus") {
        "Venus".to_string()
    } else if contains_ignore_ascii_case(name, "proto-mars") {
        "Mars".to_string()
    } else if contains_ignore_ascii_case(name, "theia") {
        "🌑 Theia".to_string()
    } else if contains_ignore_ascii_case(name, "proto-saturn") {
        "Saturn".to_string()
    } else if contains_ignore_ascii_case(name, "proto-uranus") {
        "Uranus".to_string()
    } else if contains_ignore_ascii_case(name, "proto-neptune") {
        "Neptune".to_string()
    } else if contains_ignore_ascii_case(name, "pluto") {
        "🧊 Pluto".to_string()
    } else if contains_any_ignore_ascii_case(name, &["planet nine", "planet 9"]) {
        "🪐 Planet 9".to_string()
    } else if is_embryo_body(name, body_type) {
        if contains_ignore_ascii_case(name, "theia") {
            "Theia".to_string()
        } else if let Some(suffix) = name.strip_prefix("Embryo-") {
            format!("🌱 {suffix}")
        } else if let Some(suffix) = name.strip_prefix("Embryo #") {
            format!("🌱 #{suffix}")
        } else {
            format!("🌱 {name}")
        }
    } else if contains_ignore_ascii_case(name, "kuiper") {
        "Kuiper".to_string()
    } else if contains_ignore_ascii_case(name, "ceres") {
        "Ceres".to_string()
    } else if let Some(suffix) = name.strip_prefix("Protostar Jeans-") {
        format!("✨ Jeans-{suffix}")
    } else if let Some(suffix) = name.strip_prefix("Protostar ") {
        let clean = suffix.split('(').next().unwrap_or(suffix).trim();
        format!("✨ {clean}")
    } else if contains_ignore_ascii_case(name, "genesis") {
        "Genesis Protostar".to_string()
    } else if is_star && contains_ignore_ascii_case(name, "sun") {
        "Sun".to_string()
    } else if contains_ignore_ascii_case(name, "protostar") {
        "✨ Protostar".to_string()
    } else if contains_ignore_ascii_case(name, "host star") {
        "Host Star".to_string()
    } else if starts_with_ignore_ascii_case(name, "the star")
        && let Some(short) = format_the_star_label(name)
    {
        short.to_string()
    } else if let Some(planet_short) = [
        ("planet (terrestrial", "Terrestrial"),
        ("planet (super-earth", "Super-Earth"),
        ("planet (gas giant", "Gas Giant"),
        ("planet (ice giant", "Ice Giant"),
    ]
    .iter()
    .find(|(pfx, _)| starts_with_ignore_ascii_case(name, pfx))
    .map(|(_, s)| *s)
    {
        planet_short.to_string()
    } else if starts_with_ignore_ascii_case(name, "planet-") {
        let stripped = strip_trailing_classification_tag(name);
        let rest = strip_prefix_ignore_ascii_case(stripped, "planet-").unwrap_or(stripped);
        let short = format!("Pl-{rest}");
        if short.chars().count() > 14 {
            let p: String = short.chars().take(13).collect();
            format!("{p}…")
        } else {
            short
        }
    } else if starts_with_ignore_ascii_case(name, "asteroid-") {
        strip_prefix_ignore_ascii_case(name, "asteroid-")
            .map_or_else(|| name.to_string(), |s| format!("Ast-{s}"))
    } else if starts_with_ignore_ascii_case(name, "dust-") {
        strip_prefix_ignore_ascii_case(name, "dust-")
            .map_or_else(|| name.to_string(), |s| format!("Dust-{s}"))
    } else {
        let clean = strip_trailing_classification_tag(name);
        if clean.chars().count() > 14 {
            let prefix: String = clean.chars().take(12).collect();
            format!("{prefix}…")
        } else {
            clean.to_string()
        }
    }
}

pub fn body_to_button_colors(name: &str, body_type: BodyType, is_star: bool) -> (Color, Color) {
    if is_star || body_type.is_star_or_remnant() {
        if contains_ignore_ascii_case(name, "little red dot") || body_type == BodyType::QuasiStar {
            (
                Color::srgba(0.28, 0.04, 0.08, 0.95),
                Color::srgb(1.0, 0.35, 0.45),
            )
        } else if contains_any_ignore_ascii_case(name, &["red", "trappist", "16b"]) {
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
    } else if contains_any_ignore_ascii_case(name, &["hz", "earth", "habitable", "1e", "1f", "1g"])
    {
        (
            Color::srgba(0.04, 0.22, 0.16, 0.9),
            Color::srgb(0.3, 0.9, 0.6),
        )
    } else if contains_any_ignore_ascii_case(name, &["rogue", "nemesis"]) {
        (
            Color::srgba(0.28, 0.05, 0.12, 0.95),
            Color::srgb(1.0, 0.3, 0.5),
        )
    } else if contains_ignore_ascii_case(name, "pluto") {
        (
            Color::srgba(0.10, 0.14, 0.22, 0.9),
            Color::srgb(0.7, 0.85, 0.95),
        )
    } else if contains_any_ignore_ascii_case(name, &["planet nine", "planet 9"]) {
        (
            Color::srgba(0.04, 0.12, 0.25, 0.9),
            Color::srgb(0.2, 0.75, 0.9),
        )
    } else if contains_ignore_ascii_case(name, "theia") {
        (
            Color::srgba(0.24, 0.12, 0.10, 0.95),
            Color::srgb(1.0, 0.65, 0.45),
        )
    } else if body_type == BodyType::Moon || contains_ignore_ascii_case(name, "(moon)") {
        (
            Color::srgba(0.14, 0.16, 0.22, 0.90),
            Color::srgb(0.72, 0.78, 0.88),
        )
    } else if is_embryo_body(name, body_type) {
        (
            Color::srgba(0.06, 0.18, 0.10, 0.9),
            Color::srgb(0.3, 0.95, 0.65),
        )
    } else if body_type == BodyType::GasGiant
        || contains_any_ignore_ascii_case(name, &["hot jupiter", "jupiter", "saturn"])
    {
        (
            Color::srgba(0.20, 0.12, 0.06, 0.9),
            Color::srgb(0.95, 0.65, 0.3),
        )
    } else if body_type == BodyType::IceGiant
        || contains_any_ignore_ascii_case(name, &["neptune", "uranus", "kuiper"])
    {
        (
            Color::srgba(0.06, 0.14, 0.26, 0.9),
            Color::srgb(0.4, 0.7, 1.0),
        )
    } else if body_type == BodyType::Comet || contains_ignore_ascii_case(name, "comet") {
        (
            Color::srgba(0.05, 0.14, 0.24, 0.90),
            Color::srgb(0.4, 0.8, 1.0),
        )
    } else if body_type == BodyType::Asteroid
        || body_type == BodyType::Planetesimal
        || contains_any_ignore_ascii_case(name, &["asteroid", "ceres", "vesta", "pallas"])
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
fn collect_embryo_and_minor_bodies<'a, I>(
    bodies: I,
    sort_by_mass: bool,
) -> (
    Vec<(Entity, String, BodyType, bool, f64, f64)>,
    Vec<(Entity, String, BodyType, bool, f64, f64)>,
)
where
    I: IntoIterator<
        Item = (
            Entity,
            &'a CelestialBody,
            &'a SimPosition,
            &'a Mass,
            &'a Radius,
            bool,
        ),
    >,
{
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
        "⚖️ Mass"
    } else {
        "📏 Dist"
    };
    create_compact_button(
        btn_row,
        UiButtonAction::ToggleListSortMode,
        sort_label,
        Color::srgba(0.12, 0.16, 0.22, 0.85),
        Color::srgb(0.5, 0.7, 1.0),
    );
    create_compact_button(
        btn_row,
        UiButtonAction::ToggleQuickBarFilter,
        "🔍",
        Color::srgba(0.08, 0.16, 0.28, 0.85),
        Color::srgb(0.35, 0.85, 1.0),
    );
    let wrap_label = if quick_bar_state.wrap_rows {
        "▬"
    } else {
        "⊞"
    };
    create_compact_button(
        btn_row,
        UiButtonAction::ToggleQuickBarWrap,
        wrap_label,
        Color::srgba(0.10, 0.14, 0.24, 0.85),
        Color::srgb(0.45, 0.75, 1.0),
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
            if *zone == BeltZone::OortCloud {
                create_compact_button(
                    btn_row,
                    UiButtonAction::SpawnOortCloudSwarm,
                    "☄️ + Oort Swarm [Shift+O]",
                    Color::srgba(0.12, 0.08, 0.22, 0.85),
                    Color::srgb(0.65, 0.60, 1.0),
                );
            }
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
    bool,
    Vec<BeltZone>,
    String,
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
        With<HudButton>,
    >,
) {
    if !player_state.is_changed() {
        return;
    }
    let sel = player_state.selected_entity;
    for (action, base, mut bg, mut border) in btn_query.iter_mut() {
        if let UiButtonAction::SelectEntity(ent) = action {
            let (next_bg, next_border) = if Some(*ent) == sel {
                (
                    BackgroundColor(Color::srgba(0.20, 0.45, 0.85, 0.95)),
                    BorderColor::all(Color::srgb(1.0, 1.0, 1.0)),
                )
            } else {
                (BackgroundColor(base.bg), BorderColor::all(base.border))
            };
            bg.set_if_neq(next_bg);
            border.set_if_neq(next_border);
        }
    }
}

use super::world_list::{
    QuickBarBodiesQuery, QuickBarFingerprint, QuickBarMinorEntry, QuickBarProbeEntry,
    compute_quick_bar_fingerprint, retain_filtered_quick_bar_items,
    should_defer_quick_bar_hover_rebuild,
};

#[allow(clippy::too_many_arguments, reason = "Quick bar rebuild helper")]
fn rebuild_quick_bar_contents(
    commands: &mut Commands,
    bar_ent: Entity,
    quick_bar_state: &QuickBarState,
    major_worlds: &[SystemWorld],
    embryo_bodies: &[QuickBarMinorEntry],
    minor_bodies: &[QuickBarMinorEntry],
    probes: &[QuickBarProbeEntry],
    selected_entity: Option<Entity>,
) {
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
                    major_worlds,
                    embryo_bodies,
                    minor_bodies,
                    probes,
                    selected_entity,
                    quick_bar_state,
                );
            }
        });
    }
}

#[allow(clippy::too_many_arguments, reason = "Quick bar system queries")]
pub fn update_quick_body_selector_bar(
    mut commands: Commands,
    bar_query: Query<Entity, With<QuickBodySelectorBar>>,
    bodies_query: QuickBarBodiesQuery,
    probes_query: Query<(Entity, &SpaceProbe, &CelestialBody)>,
    mut quick_bar_state: ResMut<QuickBarState>,
    player_state: Res<PlayerInteractionState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut last_state: Local<QuickBarCacheState>,
    mut last_fp: Local<Option<QuickBarFingerprint>>,
    opt_scenario: Option<Res<crate::simulation::scenarios::ActiveScenarioState>>,
    (focus, editable_query): (
        Option<Res<bevy::input_focus::InputFocus>>,
        Query<(), With<bevy::text::EditableText>>,
    ),
    hovered_bar_buttons: Query<
        (
            &ChildOf,
            &bevy::picking::hover::Hovered,
            Has<bevy::ui::Pressed>,
        ),
        With<HudButton>,
    >,
) {
    let Ok(bar_ent) = bar_query.single() else {
        return;
    };

    let text_focused =
        crate::game::ui::widgets::is_text_input_focused(focus.as_deref(), &editable_query);

    let toggled_hotkey = !text_focused && keyboard.just_pressed(KeyCode::KeyH);
    if toggled_hotkey {
        quick_bar_state.is_minimized = !quick_bar_state.is_minimized;
    }

    let current_fp = compute_quick_bar_fingerprint(
        &bodies_query,
        &probes_query,
        &quick_bar_state,
        opt_scenario.as_deref(),
    );
    if !toggled_hotkey
        && !quick_bar_state.is_changed()
        && last_fp.as_ref() == Some(&current_fp)
        && !last_state.0.is_empty()
    {
        return;
    }

    let (mut embryo_bodies, mut minor_bodies) = collect_embryo_and_minor_bodies(
        bodies_query.iter().map(|(e, b, p, m, r, s)| {
            (
                e,
                b,
                p,
                m,
                r,
                s.is_some() || b.body_type.is_star_or_remnant(),
            )
        }),
        quick_bar_state.sort_by_mass,
    );

    let mut major_worlds =
        collect_sorted_system_worlds(bodies_query.iter(), quick_bar_state.sort_by_mass);
    let mut probes: Vec<(Entity, ProbeType, ProbeStatus, String)> = probes_query
        .iter()
        .map(|(e, p, b)| (e, p.probe_type, p.status, b.name.clone()))
        .collect();

    // Filter celestial worlds and probes case-insensitively when search query is active
    retain_filtered_quick_bar_items(
        quick_bar_state.filter_text.trim(),
        &mut major_worlds,
        &mut embryo_bodies,
        &mut minor_bodies,
        &mut probes,
    );

    let probe_states: Vec<(Entity, ProbeStatus)> =
        probes.iter().map(|(e, _, s, _)| (*e, *s)).collect();

    let current_entities: Vec<Entity> = major_worlds.iter().map(|b| b.entity).collect();
    let current_state = (
        current_entities.clone(),
        embryo_bodies.len(),
        minor_bodies.len(),
        probe_states,
        quick_bar_state.is_minimized,
        quick_bar_state.show_embryos,
        quick_bar_state.show_minor_bodies,
        quick_bar_state.wrap_rows,
        quick_bar_state
            .expanded_belts
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        quick_bar_state.filter_text.clone(),
    );

    if *last_state == current_state && !major_worlds.is_empty() {
        *last_fp = Some(current_fp);
        return;
    }

    // Do not despawn and recreate QuickBar buttons while the pointer is hovering or pressing
    // one unless the user toggled bar controls/filter or major world membership changed.
    // Otherwise recreating buttons resets their `ComputedNode` size for a frame and drops clicks.
    let pointer_on_quick_bar = hovered_bar_buttons
        .iter()
        .any(|(child_of, h, pressed)| child_of.parent() == bar_ent && (h.get() || pressed));
    if should_defer_quick_bar_hover_rebuild(
        pointer_on_quick_bar,
        quick_bar_state.is_changed(),
        toggled_hotkey,
        &last_state.0,
        &current_entities,
    ) {
        return;
    }

    *last_fp = Some(current_fp);
    *last_state = current_state;

    rebuild_quick_bar_contents(
        &mut commands,
        bar_ent,
        &quick_bar_state,
        &major_worlds,
        &embryo_bodies,
        &minor_bodies,
        &probes,
        player_state.selected_entity,
    );
}

pub use super::builder_spawner::{
    compute_builder_spawn_coords, compute_builder_spawn_coords_relative,
    resolve_builder_body_type_and_temperature, spawn_custom_builder_world,
    spawn_custom_builder_world_relative, update_planet_builder_ui,
};
