//! Clickable Cosmic Event Feed & Jump-to-Action panel for tracking major system milestones.

use bevy::prelude::*;

use crate::simulation::components::{BiosphereState, CelestialBody};
use crate::simulation::resources::SimTime;
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::types::{
    CosmicEventEntry, CosmicEventLog, HudButton, HudDynamicText, HudPanelElement,
    HudVisibilityState, NotificationToast, UiButtonAction,
};
use super::widgets::hud_font_source;

/// Marker for the scrollable list container holding clickable [`UiButtonAction::JumpToEvent`] rows.
#[derive(Component)]
pub struct EventFeedListContainer;

/// Formats simulation elapsed years into a compact scannable timestamp for event feed rows.
pub fn format_event_timestamp_yr(years: f64) -> String {
    let yr = years.max(0.0);
    if yr < 1_000.0 {
        format!("{yr:.0}y")
    } else if yr < 1_000_000.0 {
        format!("{:.1}ky", yr / 1_000.0)
    } else if yr < 1_000_000_000.0 {
        format!("{:.1}My", yr / 1_000_000.0)
    } else {
        format!("{:.2}Gy", yr / 1_000_000_000.0)
    }
}

fn format_concise_event_row(entry: &CosmicEventEntry) -> String {
    let ts = format_event_timestamp_yr(entry.timestamp_yr);
    let clean_headline = entry
        .headline
        .strip_prefix(entry.icon)
        .map_or(entry.headline.as_str(), str::trim_start);
    let max_chars = 40;
    let body_text: String = if clean_headline.chars().count() > max_chars {
        let prefix: String = clean_headline.chars().take(max_chars - 1).collect();
        format!("{prefix}…")
    } else {
        clean_headline.to_string()
    };
    let jump_hint = if entry.target_entity.is_some() {
        " ⌖"
    } else {
        ""
    };
    format!("[{ts}] {} {body_text}{jump_hint}", entry.icon)
}

/// Spawns the collapsible Cosmic Event Feed pill and drawer inside the top-left HUD column.
pub fn spawn_event_feed_panel(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|wrap| {
            wrap.spawn((
                HudButton,
                UiButtonAction::ToggleEventFeed,
                HudPanelElement::EventFeedPill,
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    display: Display::Flex,
                    border: UiRect::all(Val::Px(1.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.08, 0.85)),
                BorderColor::all(Color::srgba(0.25, 0.5, 0.8, 0.6)),
            ))
            .with_children(|pill| {
                pill.spawn((
                    Text::new("📡 Events (0) [Shift-E]"),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.5, 0.85, 1.0)),
                    HudDynamicText::EventFeedBadge,
                    Pickable::IGNORE,
                ));
            });

            wrap.spawn((
                HudPanelElement::EventFeedPanel,
                Node {
                    display: Display::None,
                    width: Val::Px(296.0),
                    max_height: Val::Px(220.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(7.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.04, 0.09, 0.88)),
                BorderColor::all(Color::srgba(0.25, 0.55, 0.88, 0.55)),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    width: Val::Percent(100.0),
                    margin: UiRect::bottom(Val::Px(4.0)),
                    ..default()
                })
                .with_children(|hdr| {
                    hdr.spawn((
                        Text::new("📡 COSMIC EVENT FEED"),
                        TextFont {
                            font_size: FontSize::Px(10.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.4, 0.8, 1.0)),
                    ));
                    hdr.spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(3.0),
                        ..default()
                    })
                    .with_children(|btns| {
                        super::types::create_compact_button(
                            btns,
                            UiButtonAction::ClearEventLog,
                            "Clear",
                            Color::srgba(0.14, 0.10, 0.20, 0.85),
                            Color::srgb(0.7, 0.6, 0.9),
                        );
                        super::types::create_compact_button(
                            btns,
                            UiButtonAction::ToggleEventFeed,
                            "🗕",
                            Color::srgba(0.14, 0.08, 0.16, 0.85),
                            Color::srgb(0.9, 0.4, 0.6),
                        );
                    });
                });

                card.spawn((
                    EventFeedListContainer,
                    ScrollPosition::default(),
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.5),
                        max_height: Val::Px(172.0),
                        overflow: Overflow::scroll_y(),
                        width: Val::Percent(100.0),
                        ..default()
                    },
                ));
            });
        });
}

/// Rebuilds the clickable Cosmic Event Feed rows only when [`CosmicEventLog::revision`] changes.
pub fn update_cosmic_event_feed_ui(
    mut commands: Commands,
    mut event_log: Option<ResMut<CosmicEventLog>>,
    hud_visibility: Option<Res<HudVisibilityState>>,
    list_query: Query<Entity, With<EventFeedListContainer>>,
    mut last_revision: Local<u64>,
    mut initialized: Local<bool>,
) {
    let Some(ref mut log) = event_log else {
        return;
    };
    if hud_visibility
        .as_deref()
        .is_some_and(|v| !v.event_feed_minimized && !v.is_full_screen_clean)
        && log.unread_count > 0
    {
        log.mark_read();
    }

    if *initialized && log.revision == *last_revision {
        return;
    }

    let Ok(list_ent) = list_query.single() else {
        return;
    };

    *initialized = true;
    *last_revision = log.revision;

    let Ok(mut cmd) = commands.get_entity(list_ent) else {
        return;
    };
    cmd.despawn_children();
    cmd.with_children(|list| {
        if log.entries.is_empty() {
            list.spawn((
                Text::new("Listening for ignitions, collisions, supernovae, and life..."),
                TextFont {
                    font: hud_font_source(),
                    font_size: FontSize::Px(9.5),
                    ..default()
                },
                TextColor(Color::srgba(0.6, 0.72, 0.85, 0.75)),
                Pickable::IGNORE,
            ));
            return;
        }

        for entry in log.entries.iter().rev().take(12) {
            let row_text = format_concise_event_row(entry);
            list.spawn((
                HudButton,
                UiButtonAction::JumpToEvent(entry.target_entity),
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.11, 0.20, 0.86)),
                BorderColor::all(Color::srgba(0.24, 0.52, 0.82, 0.65)),
            ))
            .with_children(|btn| {
                btn.spawn((
                    Text::new(row_text),
                    TextFont {
                        font: hud_font_source(),
                        font_size: FontSize::Px(9.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.88, 0.96, 1.0)),
                    Pickable::IGNORE,
                ));
            });
        }
    });
}

/// Logs first-time organic biosphere emergence on habitable planets into [`CosmicEventLog`].
#[allow(clippy::too_many_arguments, reason = "Biosphere event detector system")]
pub fn detect_biosphere_emergence_events(
    biospheres: Query<(Entity, &CelestialBody, &BiosphereState)>,
    sim_time: Option<Res<SimTime>>,
    reload_gen: Option<Res<crate::simulation::scenarios::ScenarioReloadGeneration>>,
    mut event_log: Option<ResMut<CosmicEventLog>>,
    mut toast: Option<ResMut<NotificationToast>>,
    mut seen_gen: Local<u64>,
    mut seen_entities: Local<rustc_hash::FxHashSet<Entity>>,
) {
    let curr_gen = reload_gen.as_deref().map_or(0, |r| r.0);
    if curr_gen != *seen_gen {
        *seen_gen = curr_gen;
        seen_entities.clear();
    }

    let elapsed_yr = sim_time.as_deref().map_or(0.0, |st| st.elapsed_years);
    for (ent, body, bio) in biospheres.iter() {
        if bio.biomass_coverage_frac <= 0.01 || seen_entities.contains(&ent) {
            continue;
        }
        seen_entities.insert(ent);
        if elapsed_yr <= 0.01 {
            continue;
        }
        let msg = format!("🌱 LIFE EMERGED! Biosphere detected on \"{}\"!", body.name);
        if let Some(ref mut log) = event_log {
            log.push(elapsed_yr, "🌱", msg.clone(), Some(ent));
        }
        if let Some(ref mut t) = toast {
            t.message = msg;
            t.timer = 6.0;
        }
    }
}

/// Logs optional stellar ignition, planetary engulfment, and major accretion events into [`CosmicEventLog`].
pub(crate) fn record_optional_lifecycle_events(
    readers: &mut super::update::CosmicEventsReaders,
    toast: &mut NotificationToast,
    event_log: &mut CosmicEventLog,
    timestamp_yr: f64,
    bodies: &Query<&CelestialBody>,
) {
    if let Some(msgs) = readers.star_ignition.as_deref() {
        for ev in readers.star_ignition_cursor.read(msgs) {
            let name = bodies
                .get(ev.star_entity)
                .map_or("Protostar", |b| b.name.as_str());
            let msg = format!(
                "🔥 STAR IGNITED! \"{name}\" reached main sequence ({:.2} M☉, {:.0} K)!",
                ev.star_mass, ev.surface_temp_kelvin
            );
            event_log.push(timestamp_yr, "🔥", msg.clone(), Some(ev.star_entity));
            toast.message = msg;
            toast.timer = 6.0;
        }
    }

    if let Some(msgs) = readers.engulfment.as_deref() {
        for ev in readers.engulfment_cursor.read(msgs) {
            let msg = format!(
                "🌋 PLANET ENGULFED! \"{}\" ({:.2} M⊕) swallowed at {:.3} AU!",
                ev.planet_name, ev.planet_mass_earth, ev.distance_au
            );
            event_log.push(timestamp_yr, "🌋", msg.clone(), Some(ev.planet_entity));
            toast.message = msg;
            toast.timer = 6.0;
        }
    }

    if let Some(msgs) = readers.accretion_merge.as_deref() {
        for ev in readers.accretion_merge_cursor.read(msgs) {
            if ev.new_body_type.is_star_or_remnant()
                || (ev.merged_mass < 0.05 * EARTH_MASS_SOLAR && ev.energy_released < 1.0e31)
            {
                continue;
            }
            let name = bodies
                .get(ev.primary_entity)
                .map_or("Body", |b| b.name.as_str());
            let mass_earth = ev.merged_mass / EARTH_MASS_SOLAR;
            let msg = format!("☄️ GIANT IMPACT! \"{name}\" accreted to {mass_earth:.2} M⊕!");
            event_log.push(timestamp_yr, "☄️", msg.clone(), Some(ev.primary_entity));
            toast.message = msg;
            toast.timer = 6.0;
        }
    }
}

/// Logs crisis onset/resolution, civilization milestones, and off-world colonies into [`CosmicEventLog`].
#[allow(
    clippy::too_many_arguments,
    reason = "Reads crisis and civilization lifecycle message streams"
)]
pub fn log_crisis_and_civilization_events(
    sim_time: Option<Res<SimTime>>,
    mut event_log: Option<ResMut<CosmicEventLog>>,
    mut toast: Option<ResMut<NotificationToast>>,
    mut crisis_started: Option<MessageReader<crate::simulation::events::CosmicEventStartedEvent>>,
    mut crisis_resolved: Option<MessageReader<crate::simulation::events::CosmicEventResolvedEvent>>,
    mut civ_milestones: Option<
        MessageReader<crate::simulation::biology::CivilizationMilestoneEvent>,
    >,
    mut colonies: Option<MessageReader<crate::simulation::biology::ColonyFoundedEvent>>,
) {
    let Some(ref mut log) = event_log else {
        return;
    };
    let ts = sim_time.as_deref().map_or(0.0, |s| s.elapsed_years);

    if let Some(ref mut r) = crisis_started {
        for ev in r.read() {
            let msg = format!("🚨 CRISIS // {}", ev.event_type.title());
            log.push(ts, "🚨", msg, ev.target_entity);
        }
    }
    if let Some(ref mut r) = crisis_resolved {
        for ev in r.read() {
            let msg = format!(
                "✅ {} {} (+{:.0} Sci)",
                ev.event_type.title(),
                ev.outcome.label(),
                ev.science_reward
            );
            log.push(ts, "✅", msg, ev.target_entity);
        }
    }
    if let Some(ref mut r) = civ_milestones {
        for ev in r.read() {
            let msg = if ev.collapsed {
                format!(
                    "💀 COLLAPSE // {} civilization destabilized!",
                    ev.world_name
                )
            } else {
                format!(
                    "🏙️ {} reached {} (K{:.2})!",
                    ev.world_name,
                    ev.era.short_badge(),
                    ev.kardashev_level
                )
            };
            let icon = if ev.collapsed { "💀" } else { "🏙️" };
            log.push(ts, icon, msg.clone(), Some(ev.world_entity));
            if let Some(ref mut t) = toast {
                t.message = msg;
                t.timer = 5.0;
            }
        }
    }
    if let Some(ref mut r) = colonies {
        for ev in r.read() {
            let msg = format!(
                "🛸 COLONY FOUNDED // {} colonized {}!",
                ev.origin_name, ev.colony_name
            );
            log.push(ts, "🛸", msg.clone(), Some(ev.colony_entity));
            if let Some(ref mut t) = toast {
                t.message = msg;
                t.timer = 5.0;
            }
        }
    }
}
