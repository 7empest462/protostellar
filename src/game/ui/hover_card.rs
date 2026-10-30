//! Floating holographic hover card and real-time planetary telemetry preview.
//!
//! Provides immediate, tactile visual feedback when hovering over any celestial body
//! in the 3D viewport, displaying body classification, mass, radius, distance,
//! orbital velocity, and dynamic astrophysical status badges.

use bevy::math::DVec3;
use bevy::prelude::*;

use super::telemetry_format::{format_body_radius_display, format_mass_string};
use crate::rendering::camera::{IgnoreUiPicking, PanOrbitCamera};
use crate::simulation::components::*;
use crate::simulation::resources::*;
use crate::simulation::tides::TidalState;
use crate::utils::constants::AU_PER_YR_TO_KM_PER_S;

/// Root marker for the 2D floating holographic hover card.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct HoverCardRoot;

/// Field selector enum identifying text components within the hover card.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverCardField {
    Title,
    Subhead,
    Metrics,
    Badges,
}

/// Spawns the glassmorphic floating hover card node hierarchy.
pub fn spawn_hover_card(root: &mut ChildSpawnerCommands) {
    root.spawn((
        HoverCardRoot,
        IgnoreUiPicking,
        Node {
            position_type: PositionType::Absolute,
            display: Display::None,
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
            min_width: Val::Px(240.0),
            max_width: Val::Px(320.0),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius {
                top_left: Val::Px(2.0),
                top_right: Val::Px(6.0),
                bottom_right: Val::Px(2.0),
                bottom_left: Val::Px(6.0),
            },
            row_gap: Val::Px(3.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.05, 0.11, 0.94)),
        BorderColor::all(Color::srgba(0.20, 0.80, 0.96, 0.75)),
    ))
    .with_children(|card| {
        // Title (Name + Icon)
        card.spawn((
            HoverCardField::Title,
            IgnoreUiPicking,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(12.5),
                ..default()
            },
            TextColor(Color::srgb(0.95, 0.98, 1.0)),
        ));

        // Subhead (Classification / Archetype)
        card.spawn((
            HoverCardField::Subhead,
            IgnoreUiPicking,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(10.0),
                ..default()
            },
            TextColor(Color::srgb(0.35, 0.85, 1.0)),
        ));

        // Divider
        card.spawn((
            IgnoreUiPicking,
            Node {
                height: Val::Px(1.0),
                width: Val::Percent(100.0),
                margin: UiRect::axes(Val::Px(0.0), Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.25, 0.82, 0.98, 0.35)),
        ));

        // Metrics (Distance, Speed, Mass, Radius, Temp)
        card.spawn((
            HoverCardField::Metrics,
            IgnoreUiPicking,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(9.5),
                ..default()
            },
            TextColor(Color::srgb(0.80, 0.90, 0.98)),
        ));

        // Badges (Habitability, Volcanism, Magnetosphere, Locks, Rings)
        card.spawn((
            HoverCardField::Badges,
            IgnoreUiPicking,
            Text::new(""),
            TextFont {
                font_size: FontSize::Px(9.0),
                ..default()
            },
            TextColor(Color::srgb(0.40, 0.95, 0.70)),
        ));

        // Action hint
        card.spawn((
            IgnoreUiPicking,
            Text::new("🎯 Left Click to Focus & Inspect"),
            TextFont {
                font_size: FontSize::Px(8.5),
                ..default()
            },
            TextColor(Color::srgba(0.50, 0.75, 0.95, 0.75)),
        ));
    });
}

/// Resolves human-readable astronomical classification string for a given body type.
pub fn resolve_body_type_label(body_type: BodyType) -> &'static str {
    match body_type {
        BodyType::DustGrain => "Dust Grain",
        BodyType::Planetesimal => "Planetesimal",
        BodyType::Protoplanet => "Protoplanet",
        BodyType::TerrestrialPlanet => "Terrestrial Planet",
        BodyType::SuperEarth => "Super-Earth",
        BodyType::GasGiant => "Gas Giant",
        BodyType::IceGiant => "Ice Giant",
        BodyType::BrownDwarf => "Sub-Stellar Brown Dwarf",
        BodyType::RedDwarf => "M-Type Red Dwarf",
        BodyType::OrangeDwarf => "K-Type Orange Dwarf",
        BodyType::YellowDwarf => "G-Type Yellow Dwarf",
        BodyType::YellowWhiteDwarf => "F-Type Yellow-White Dwarf",
        BodyType::WhiteMainSequence => "A-Type White Star",
        BodyType::BlueMainSequence => "B-Type Blue Star",
        BodyType::BlueGiant => "Blue Giant",
        BodyType::BlueSupergiant => "Blue Supergiant",
        BodyType::RedGiant => "Red Giant",
        BodyType::RedSupergiant => "Red Supergiant",
        BodyType::YellowHypergiant => "Yellow Hypergiant",
        BodyType::Hypergiant => "Luminous Hypergiant",
        BodyType::CarbonStar => "Carbon Star",
        BodyType::LuminousBlueVariable => "Luminous Blue Variable",
        BodyType::WolfRayet => "Wolf-Rayet Star",
        BodyType::PopulationIIIStar => "Population III Hypergiant",
        BodyType::BlueStraggler => "Blue Straggler Star",
        BodyType::WhiteDwarf => "Degenerate White Dwarf",
        BodyType::BlackDwarf => "Degenerate Black Dwarf",
        BodyType::NeutronStar => "Neutron Star",
        BodyType::Pulsar => "Relativistic Pulsar",
        BodyType::Magnetar => "Extreme Magnetar",
        BodyType::QuarkStar => "Hypothetical Quark Star",
        BodyType::DarkStar => "Dark Matter Powered Star",
        BodyType::BosonStar => "Transparent Boson Star",
        BodyType::IronStar => "Cold Degenerate Iron Star",
        BodyType::ThorneZytkow => "Thorne-Żytkow Hybrid",
        BodyType::Protostar => "Collapsing Protostar",
        BodyType::TTauriStar => "Pre-Main-Sequence T Tauri",
        BodyType::HerbigAeBeStar => "Herbig Ae/Be Star",
        BodyType::MainSequenceStar => "Main Sequence Star",
        BodyType::BlackHole => "Kerr Black Hole",
        BodyType::QuasiStar => "Black Hole Powered Quasi-Star",
        BodyType::Asteroid => "Asteroid",
        BodyType::Comet => "Active Comet",
        BodyType::Moon => "Natural Moon",
        BodyType::DebrisRing => "Circumstellar Debris Ring",
    }
}

/// Formats core metric lines for the hover card (Distance, Speed, Mass, Radius, Temperature).
pub fn format_hover_metrics(pos: DVec3, vel: DVec3, mass: f64, rad: f64, temp: f64) -> String {
    let dist_au = if pos.is_finite() { pos.length() } else { 0.0 };
    let speed_km_s = if vel.is_finite() {
        vel.length() * AU_PER_YR_TO_KM_PER_S
    } else {
        0.0
    };
    let mass_str = format_mass_string(mass);
    let rad_disp = format_body_radius_display(rad);

    format!(
        "Distance: {dist_au:.2} AU  •  Speed: {speed_km_s:.1} km/s\nMass: {mass_str}\nRadius: {rad_disp}  •  Temp: {temp:.0} K"
    )
}

/// Collects dynamic real-time astrophysical status badges for a hovered body.
#[allow(
    clippy::too_many_arguments,
    reason = "Hover badge evaluation inspects climate, biosphere, volcanism, differentiation, and tides"
)]
pub fn collect_hover_badges(
    opt_climate: Option<&PlanetaryClimate>,
    opt_bio: Option<&BiosphereState>,
    opt_volc: Option<&VolcanicState>,
    opt_diff: Option<&InternalDifferentiation>,
    opt_tide: Option<&TidalState>,
    opt_rings: Option<&PlanetaryRingSystem>,
    opt_satellite: Option<&SatelliteOf>,
    is_probe: bool,
) -> String {
    let mut badges = Vec::with_capacity(6);

    if is_probe {
        badges.push("🛰️ Space Probe");
    }

    if opt_satellite.is_some() {
        badges.push("🌕 Natural Satellite");
    }

    if let Some(climate) = opt_climate {
        match climate.climate_regime {
            ClimateRegime::TemperateHabitable => badges.push("🟢 Habitable"),
            ClimateRegime::SnowballIceAge => badges.push("❄️ Snowball Earth"),
            ClimateRegime::RunawayVenusian => badges.push("🔥 Runaway Greenhouse"),
            ClimateRegime::GasGiantEnvelope => badges.push("🪐 Gas Envelope"),
            ClimateRegime::AirlessVacuum => badges.push("🪨 Airless Vacuum"),
        }
    }

    if let Some(bio) = opt_bio {
        if bio.biomass_coverage_frac > 0.05 {
            badges.push("🌿 Biosphere");
        }
        if bio.technosignature > 0.1 {
            badges.push("🏙️ Civilization");
        }
    }

    if let Some(volc) = opt_volc {
        match volc.activity_type {
            VolcanicActivityType::CryoWaterIce | VolcanicActivityType::CryoHydrocarbon => {
                badges.push("❄️ Cryovolcanic");
            }
            VolcanicActivityType::IoSulfurVolcanism | VolcanicActivityType::SilicateBasaltic => {
                badges.push("🌋 Active Volcanism");
            }
            VolcanicActivityType::Dormant => {}
        }
    }

    if let Some(diff) = opt_diff {
        if diff.magnetic_field_gauss >= 0.25 {
            badges.push("🧲 Magnetosphere");
        }
    }

    if let Some(tide) = opt_tide {
        if tide.is_tidally_locked {
            badges.push("🔒 Tidally Locked");
        }
    }

    if opt_rings.is_some() {
        badges.push("🪐 Rings");
    }

    if badges.is_empty() {
        "✨ Stable Orbit".to_string()
    } else {
        badges.join("  •  ")
    }
}

/// Calculates clamped viewport coordinates for the floating hover card
/// ensuring it never clips outside the window or overlaps critical toolbars.
pub fn compute_clamped_card_position(screen_pos: Vec2, card_size: Vec2, window_size: Vec2) -> Vec2 {
    let mut target_x = screen_pos.x + 24.0;
    let mut target_y = screen_pos.y - 30.0;

    // Flip to left of target if overflowing right margin
    if target_x + card_size.x > window_size.x - 15.0 {
        target_x = screen_pos.x - card_size.x - 24.0;
    }
    // Flip upward if overflowing bottom toolbar margin
    if target_y + card_size.y > window_size.y - 75.0 {
        target_y = screen_pos.y - card_size.y - 12.0;
    }

    let clamped_x = target_x.clamp(10.0, (window_size.x - card_size.x - 10.0).max(10.0));
    let clamped_y = target_y.clamp(10.0, (window_size.y - card_size.y - 10.0).max(10.0));

    Vec2::new(clamped_x, clamped_y)
}

#[allow(
    clippy::type_complexity,
    reason = "Hover card UI update requires window, camera, target components, and node queries"
)]
pub fn update_hover_card_ui(
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<PanOrbitCamera>>,
    player_state: Res<PlayerInteractionState>,
    mut card_query: Query<(&mut Node, Option<&ComputedNode>), With<HoverCardRoot>>,
    mut text_query: Query<(&HoverCardField, &mut Text)>,
    bodies_query: Query<(
        &SimPosition,
        &SimVelocity,
        &Mass,
        &Radius,
        &Temperature,
        &Composition,
        &CelestialBody,
        Option<&InternalDifferentiation>,
        Option<&TidalState>,
        Option<&PlanetaryRingSystem>,
        Option<&PlanetaryClimate>,
        Option<&BiosphereState>,
        Option<&VolcanicState>,
        Option<&SatelliteOf>,
        Option<&crate::simulation::probes::SpaceProbe>,
    )>,
) {
    let Ok((mut card_node, opt_computed)) = card_query.single_mut() else {
        return;
    };

    let Some(hovered_entity) = player_state.hovered_entity else {
        card_node.display = Display::None;
        return;
    };

    let Ok((
        pos,
        vel,
        mass,
        rad,
        temp,
        _comp,
        body,
        opt_diff,
        opt_tide,
        opt_rings,
        opt_climate,
        opt_bio,
        opt_volc,
        opt_satellite,
        opt_probe,
    )) = bodies_query.get(hovered_entity)
    else {
        card_node.display = Display::None;
        return;
    };

    let Ok(window) = windows.single() else {
        card_node.display = Display::None;
        return;
    };

    let Ok((camera_comp, cam_gt)) = camera_query.single() else {
        card_node.display = Display::None;
        return;
    };

    let world_center = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
    let screen_pos = if let Ok(sp) = camera_comp.world_to_viewport(cam_gt, world_center) {
        sp
    } else {
        let cam_forward = cam_gt.forward();
        let to_target = world_center - cam_gt.translation();
        if to_target.dot(*cam_forward) <= 0.0 && *cam_forward != Vec3::ZERO {
            card_node.display = Display::None;
            return;
        }
        Vec2::new(window.width() * 0.5, window.height() * 0.5)
    };

    // Card dimensions for positioning
    let card_w = opt_computed.map_or(260.0, |c| c.size().x).max(220.0);
    let card_h = opt_computed.map_or(130.0, |c| c.size().y).max(100.0);
    let window_size = Vec2::new(window.width(), window.height());
    let clamped_pos =
        compute_clamped_card_position(screen_pos, Vec2::new(card_w, card_h), window_size);

    card_node.display = Display::Flex;
    card_node.left = Val::Px(clamped_pos.x);
    card_node.top = Val::Px(clamped_pos.y);

    // Format fields
    let title_str = body.name.clone();
    let subhead_str = format!("[ {} ]", resolve_body_type_label(body.body_type));
    let metrics_str = format_hover_metrics(pos.0, vel.0, mass.0, rad.0, temp.0);
    let badges_str = collect_hover_badges(
        opt_climate,
        opt_bio,
        opt_volc,
        opt_diff,
        opt_tide,
        opt_rings,
        opt_satellite,
        opt_probe.is_some(),
    );

    for (field, mut text) in &mut text_query {
        match field {
            HoverCardField::Title => text.0.clone_from(&title_str),
            HoverCardField::Subhead => text.0.clone_from(&subhead_str),
            HoverCardField::Metrics => text.0.clone_from(&metrics_str),
            HoverCardField::Badges => text.0.clone_from(&badges_str),
        }
    }
}
