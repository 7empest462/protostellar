//! HUD top-center Crisis Warning & Active Planetary Defense Alert Banner.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use super::types::*;
use super::update::{set_display_if_changed, set_text_if_changed};
use super::widgets::hud_font_source;
use crate::simulation::economy::{GameMode, ProgressionState};
use crate::simulation::events::{
    CosmicEventManager, CosmicEventPhase, CosmicEventType, CrisisCountermeasureKind,
};

/// Root container marker for the Crisis Alert Banner.
#[derive(Component)]
pub struct CrisisBannerContainer;

/// Marker for the warning/active badge label on the Crisis Banner.
#[derive(Component)]
pub struct CrisisBannerBadgeText;

/// Marker for the crisis title & target world header on the Crisis Banner.
#[derive(Component)]
pub struct CrisisBannerTitleText;

/// Marker for live threat count, mitigation, and ETA text on the Crisis Banner.
#[derive(Component)]
pub struct CrisisBannerMetricsText;

/// Marker for the recommended countermeasure button label on the Crisis Banner.
#[derive(Component)]
pub struct CrisisBannerCountermeasureText;

/// Spawns the top-center Crisis Warning & Active Defense Alert Banner.
pub fn spawn_crisis_alert_banner(root: &mut ChildSpawnerCommands) {
    root.spawn((
        CrisisBannerContainer,
        ZIndex(45),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(46.0),
            left: Val::Percent(50.0),
            width: Val::Px(500.0),
            margin: UiRect::left(Val::Px(-250.0)),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            display: Display::None,
            ..default()
        },
        BackgroundColor(Color::srgba(0.14, 0.03, 0.03, 0.94)),
        BorderColor::all(Color::srgba(1.0, 0.35, 0.25, 0.92)),
        bevy::ui::FocusPolicy::Block,
        bevy::picking::hover::Hovered::default(),
    ))
    .with_children(|banner| {
        banner
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|top| {
                top.spawn((
                    CrisisBannerBadgeText,
                    Text::new("🚨 CRISIS WARNING"),
                    TextFont {
                        font: hud_font_source(),
                        font_size: FontSize::Px(10.5),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.78, 0.30)),
                ));
                top.spawn((
                    CrisisBannerTitleText,
                    Text::new("CRISIS // TARGET"),
                    TextFont {
                        font: hud_font_source(),
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.92, 0.90)),
                ));
            });

        banner
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|bot| {
                bot.spawn((
                    CrisisBannerMetricsText,
                    Text::new("Mitigation: 0% | ETA: 0.0 yr"),
                    TextFont {
                        font: hud_font_source(),
                        font_size: FontSize::Px(9.5),
                        ..default()
                    },
                    TextColor(Color::srgb(0.95, 0.80, 0.65)),
                ));

                bot.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|btns| {
                    create_compact_button(
                        btns,
                        UiButtonAction::FocusActiveCrisis,
                        "⌖ Focus Threat",
                        Color::srgba(0.22, 0.12, 0.05, 0.92),
                        Color::srgb(1.0, 0.72, 0.30),
                    );
                    btns.spawn((
                        HudButton,
                        UiButtonAction::ExecuteCrisisCountermeasure,
                        Node {
                            padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.06, 0.22, 0.34, 0.92)),
                        BorderColor::all(Color::srgb(0.35, 0.90, 1.0)),
                    ))
                    .with_children(|btn| {
                        btn.spawn((
                            CrisisBannerCountermeasureText,
                            Text::new("🚀 Kinetic Deflector"),
                            TextFont {
                                font: hud_font_source(),
                                font_size: FontSize::Px(9.5),
                                ..default()
                            },
                            TextColor(Color::srgb(0.88, 0.98, 1.0)),
                            Pickable::IGNORE,
                        ));
                    });
                });
            });
    });
}

fn crisis_banner_signature(
    ev: &crate::simulation::events::ActiveCosmicEvent,
    mode: GameMode,
) -> u64 {
    let mut h = DefaultHasher::new();
    ev.event_type.hash(&mut h);
    (ev.phase as u8).hash(&mut h);
    ev.target_name.hash(&mut h);
    ev.threats_total.hash(&mut h);
    ev.threats_neutralized.hash(&mut h);
    ev.impacts_suffered.hash(&mut h);
    ((ev.mitigation_score * 100.0).round() as i32).hash(&mut h);
    ((ev.warning_remaining_years * 10.0).round() as i64).hash(&mut h);
    ((ev.elapsed_years * 10.0).round() as i64).hash(&mut h);
    (mode as u8).hash(&mut h);
    h.finish()
}

/// Synchronizes Crisis Alert Banner visibility, phase colors, live threat metrics, and action button.
#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "Reads crisis manager, HUD visibility, and progression mode to update banner widgets"
)]
pub fn update_crisis_alert_banner_ui(
    opt_mgr: Option<Res<CosmicEventManager>>,
    opt_vis: Option<Res<HudVisibilityState>>,
    opt_prog: Option<Res<ProgressionState>>,
    mut container_q: Query<
        (&mut Node, &mut BackgroundColor, &mut BorderColor),
        With<CrisisBannerContainer>,
    >,
    mut badge_q: Query<
        &mut Text,
        (
            With<CrisisBannerBadgeText>,
            Without<CrisisBannerTitleText>,
            Without<CrisisBannerMetricsText>,
            Without<CrisisBannerCountermeasureText>,
        ),
    >,
    mut title_q: Query<
        &mut Text,
        (
            With<CrisisBannerTitleText>,
            Without<CrisisBannerBadgeText>,
            Without<CrisisBannerMetricsText>,
            Without<CrisisBannerCountermeasureText>,
        ),
    >,
    mut metrics_q: Query<
        &mut Text,
        (
            With<CrisisBannerMetricsText>,
            Without<CrisisBannerBadgeText>,
            Without<CrisisBannerTitleText>,
            Without<CrisisBannerCountermeasureText>,
        ),
    >,
    mut action_q: Query<
        &mut Text,
        (
            With<CrisisBannerCountermeasureText>,
            Without<CrisisBannerBadgeText>,
            Without<CrisisBannerTitleText>,
            Without<CrisisBannerMetricsText>,
        ),
    >,
    mut last_sig: Local<Option<u64>>,
) {
    let cinema = opt_vis.as_deref().is_some_and(|v| v.is_full_screen_clean);
    let active_ev = if cinema {
        None
    } else {
        opt_mgr
            .as_deref()
            .and_then(CosmicEventManager::primary_active_event)
    };

    let Ok((mut node, mut bg, mut border)) = container_q.single_mut() else {
        return;
    };

    let Some(ev) = active_ev else {
        set_display_if_changed(&mut node, Display::None);
        *last_sig = None;
        return;
    };
    set_display_if_changed(&mut node, Display::Flex);

    let mode = opt_prog.as_deref().map_or(GameMode::Sandbox, |p| p.mode);
    let sig = crisis_banner_signature(ev, mode);
    if *last_sig == Some(sig) {
        return;
    }
    *last_sig = Some(sig);

    let is_warn = ev.phase == CosmicEventPhase::Warning;
    bg.set_if_neq(BackgroundColor(if is_warn {
        Color::srgba(0.16, 0.10, 0.02, 0.94)
    } else {
        Color::srgba(0.16, 0.03, 0.04, 0.94)
    }));
    border.set_if_neq(BorderColor::all(if is_warn {
        Color::srgba(1.0, 0.72, 0.22, 0.92)
    } else {
        Color::srgba(1.0, 0.30, 0.25, 0.95)
    }));

    if let Ok(mut txt) = badge_q.single_mut() {
        let badge = if is_warn {
            format!("🚨 WARNING ({:.1} yr)", ev.warning_remaining_years)
        } else {
            "⚠️ ACTIVE CRISIS".to_string()
        };
        set_text_if_changed(&mut txt, badge);
    }
    if let Ok(mut txt) = title_q.single_mut() {
        let tgt = if ev.target_name.is_empty() {
            "System-Wide"
        } else {
            ev.target_name.as_str()
        };
        set_text_if_changed(&mut txt, format!("{} // {}", ev.event_type.title(), tgt));
    }
    if let Ok(mut txt) = metrics_q.single_mut() {
        let rem_yr = (ev.duration_years - ev.elapsed_years).max(0.0);
        let mit_pct = (ev.mitigation_score * 100.0).round();
        let line = if ev.event_type == CosmicEventType::PlanetKillerSwarm {
            format!(
                "Deflected: {}/{} | Hits: {} | Rem: {:.1}y",
                ev.threats_neutralized, ev.threats_total, ev.impacts_suffered, rem_yr
            )
        } else {
            format!("Mitigation: {mit_pct:.0}% | Remaining: {rem_yr:.1} yr")
        };
        set_text_if_changed(&mut txt, line);
    }
    if let Ok(mut txt) = action_q.single_mut() {
        let rec = CrisisCountermeasureKind::recommended_for(ev.event_type);
        let label = if mode == GameMode::Progression {
            format!("{} ({:.0} Sci)", rec.label(), rec.science_cost())
        } else {
            rec.label().to_string()
        };
        set_text_if_changed(&mut txt, label);
    }
}
