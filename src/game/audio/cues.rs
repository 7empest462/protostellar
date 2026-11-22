//! Dynamic sink modulation (space ambient + surface wind/surf loops) and event cue routing.

use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::simulation::components::BodyType;
use crate::utils::constants::EARTH_MASS_SOLAR;

use super::sonification::SurfaceAmbienceProfile;
use super::{
    CosmicAmbientPlayer, CosmicAudioEventReaders, CosmicAudioHandles, CosmicAudioState,
    SurfaceOceanPlayer, SurfaceWindPlayer,
};

pub type SurfaceLoopItem = (Entity, Option<&'static mut AudioSink>);
pub type SurfaceLoopFilter<M, Ex1, Ex2> = (With<M>, Without<Ex1>, Without<Ex2>);

/// Disjoint queries for the persistent space drone and dynamically spawned surface audio loops.
#[derive(SystemParam)]
pub struct AmbientAudioSinks<'w, 's> {
    pub space_ambient: Query<
        'w,
        's,
        &'static mut AudioSink,
        SurfaceLoopFilter<CosmicAmbientPlayer, SurfaceWindPlayer, SurfaceOceanPlayer>,
    >,
    pub surface_wind: Query<
        'w,
        's,
        SurfaceLoopItem,
        SurfaceLoopFilter<SurfaceWindPlayer, CosmicAmbientPlayer, SurfaceOceanPlayer>,
    >,
    pub surface_ocean: Query<
        'w,
        's,
        SurfaceLoopItem,
        SurfaceLoopFilter<SurfaceOceanPlayer, CosmicAmbientPlayer, SurfaceWindPlayer>,
    >,
}

fn sync_surface_loop_channel<M: Component + Default>(
    commands: &mut Commands,
    handle: Option<&Handle<AudioSource>>,
    active_query: &mut Query<(Entity, Option<&mut AudioSink>), impl bevy::ecs::query::QueryFilter>,
    should_play: bool,
    target_vol: f32,
    target_speed: f32,
) {
    if !should_play || handle.is_none() {
        for (ent, _) in active_query.iter() {
            commands.entity(ent).despawn();
        }
        return;
    }

    let mut found = false;
    for (ent, opt_sink) in active_query.iter_mut() {
        if found {
            commands.entity(ent).despawn();
            continue;
        }
        found = true;
        if let Some(mut sink) = opt_sink {
            if sink.is_muted() {
                sink.unmute();
            }
            if (sink.volume().to_linear() - target_vol).abs() > 1e-4 {
                sink.set_volume(Volume::Linear(target_vol));
            }
            if (sink.speed() - target_speed).abs() > 1e-4 {
                sink.set_speed(target_speed);
            }
        }
    }

    if !found && let Some(src) = handle {
        commands.spawn((
            AudioPlayer::new(src.clone()),
            PlaybackSettings::LOOP
                .with_volume(Volume::Linear(target_vol))
                .with_speed(target_speed),
            M::default(),
        ));
    }
}

/// Synchronizes space continuum sink speed/ducking and dynamically spawns/despawns surface loops.
pub(crate) fn sync_ambient_and_surface_sinks(
    commands: &mut Commands,
    handles: &CosmicAudioHandles,
    audio_state: &CosmicAudioState,
    surface: SurfaceAmbienceProfile,
    sinks: &mut AmbientAudioSinks,
) {
    let duck = if surface.active {
        surface.space_duck
    } else {
        1.0
    };
    for mut sink in sinks.space_ambient.iter_mut() {
        if audio_state.enabled {
            if sink.is_muted() {
                sink.unmute();
            }
            let target_speed = ((audio_state.carrier_freq_hz / 55.0) * audio_state.time_warp_pitch)
                .clamp(0.45, 3.0);
            if (sink.speed() - target_speed).abs() > 1e-4 {
                sink.set_speed(target_speed);
            }
            let linear_vol = (audio_state.master_volume
                * (0.30 + 0.55 * audio_state.strain_or_flux + 0.25 * audio_state.proximity_boost)
                * duck)
                .clamp(0.0, 1.0);
            if (sink.volume().to_linear() - linear_vol).abs() > 1e-4 {
                sink.set_volume(Volume::Linear(linear_vol));
            }
        } else if !sink.is_muted() {
            sink.mute();
        }
    }

    let surface_warp = 1.0 + (audio_state.time_warp_pitch - 1.0) * 0.45;
    let wind_play = audio_state.enabled && surface.active && surface.wind_gain > 0.01;
    let wind_vol = (audio_state.master_volume * surface.wind_gain).clamp(0.0, 1.0);
    let wind_speed = (surface.wind_pitch * surface_warp).clamp(0.55, 1.85);
    sync_surface_loop_channel::<SurfaceWindPlayer>(
        commands,
        handles.surface_wind_loop.as_ref(),
        &mut sinks.surface_wind,
        wind_play,
        wind_vol,
        wind_speed,
    );

    let ocean_play = audio_state.enabled && surface.active && surface.ocean_gain > 0.01;
    let ocean_vol = (audio_state.master_volume * surface.ocean_gain).clamp(0.0, 1.0);
    let ocean_speed = (surface.ocean_pitch * surface_warp).clamp(0.60, 1.65);
    sync_surface_loop_channel::<SurfaceOceanPlayer>(
        commands,
        handles.surface_ocean_loop.as_ref(),
        &mut sinks.surface_ocean,
        ocean_play,
        ocean_vol,
        ocean_speed,
    );
}

#[inline]
fn drain_any<M: bevy::ecs::message::Message>(
    reader: &mut Option<MessageReader<'_, '_, M>>,
) -> bool {
    reader.as_mut().is_some_and(|r| r.read().count() > 0)
}

fn detect_new_quasar_blowout(
    events: &CosmicAudioEventReaders,
    played_blowouts: &mut rustc_hash::FxHashSet<Entity>,
) -> bool {
    let Some(ref pool) = events.supernova_pool else {
        return false;
    };
    played_blowouts.retain(|e| {
        pool.explosions.iter().any(|exp| {
            exp.star_entity == *e
                && exp.explosion_type
                    == crate::rendering::effects::supernova::SupernovaType::QuasarBlowout
        })
    });
    let mut found = false;
    for exp in &pool.explosions {
        if exp.explosion_type == crate::rendering::effects::supernova::SupernovaType::QuasarBlowout
            && played_blowouts.insert(exp.star_entity)
        {
            found = true;
        }
    }
    found
}

fn collect_major_impact_cue(events: &mut CosmicAudioEventReaders) -> Option<(f32, f32)> {
    let mut best_mass_e: f64 = 0.0;
    let reader = events.accretion_merge.as_mut()?;
    for ev in reader.read() {
        let qualifies = !ev.new_body_type.is_star_or_remnant()
            && (ev.merged_mass >= 0.05 * EARTH_MASS_SOLAR || ev.energy_released >= 1.0e31);
        if qualifies {
            best_mass_e = best_mass_e.max(ev.merged_mass / EARTH_MASS_SOLAR);
        }
    }
    if best_mass_e <= 0.0 {
        return None;
    }
    let t = (best_mass_e.max(0.05) as f32 / 5.0).clamp(0.0, 1.0);
    Some((0.65 + 0.23 * t, 1.08 - 0.26 * t))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Coordinates event draining, reload-gen suppression, cooldown throttling, and cue spawning"
)]
pub(crate) fn dispatch_cosmic_audio_cues(
    commands: &mut Commands,
    handles: &CosmicAudioHandles,
    audio_state: &CosmicAudioState,
    events: &mut CosmicAudioEventReaders,
    visual_time: f32,
    last_cue_time: &mut f32,
    played_blowouts: &mut rustc_hash::FxHashSet<Entity>,
    last_reload_gen: &mut u64,
) {
    let has_new_blowout = detect_new_quasar_blowout(events, played_blowouts);

    let curr_reload_gen = events.reload_gen.as_deref().map_or(0, |r| r.0);
    let suppress_probe = curr_reload_gen != *last_reload_gen;
    if suppress_probe {
        *last_reload_gen = curr_reload_gen;
    }
    let has_probe_launch = !suppress_probe && !events.launched_probes.is_empty();

    let supernova_speed = events.supernova.as_mut().and_then(|r| {
        r.read().last().map(|ev| match ev.remnant_type {
            BodyType::WhiteDwarf => 0.78_f32,
            BodyType::BlackHole => 0.86_f32,
            _ => 1.0_f32,
        })
    });

    let (mut has_kilonova, mut has_bbh_chirp) = (false, false);
    if let Some(ref mut r) = events.gw_merger {
        for ev in r.read() {
            if ev.remnant_type == BodyType::BlackHole && ev.radiated_gw_mass_solar > 0.20 {
                has_bbh_chirp = true;
            } else {
                has_kilonova = true;
            }
        }
    }

    let has_ignition = drain_any(&mut events.star_ignition);
    let impact_params = collect_major_impact_cue(events);
    let has_disruption = drain_any(&mut events.tde) || drain_any(&mut events.roche);
    let has_pulse = drain_any(&mut events.cme)
        || drain_any(&mut events.pulsar_glitch)
        || drain_any(&mut events.starquake);
    let has_crisis_start = drain_any(&mut events.crisis_civ_events.0);
    let crisis_resolved_averted = events.crisis_civ_events.1.as_mut().and_then(|r| {
        r.read().last().map(|ev| {
            matches!(
                ev.outcome,
                crate::simulation::events::CrisisOutcome::Averted
                    | crate::simulation::events::CrisisOutcome::Mitigated
            )
        })
    });
    let has_civ_milestone = drain_any(&mut events.crisis_civ_events.2);
    let has_countermeasure = drain_any(&mut events.crisis_civ_events.3);

    if !audio_state.enabled {
        return;
    }

    if has_new_blowout && let Some(ref cue) = handles.blowout_cue {
        let vol = (audio_state.master_volume * 0.95).clamp(0.0, 1.0);
        commands.spawn((
            AudioPlayer::new(cue.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(vol)),
        ));
        *last_cue_time = visual_time;
        return;
    }

    if (visual_time - *last_cue_time).abs() < 0.35 {
        return;
    }

    let selected: Option<(&Handle<AudioSource>, f32, f32)> = if let Some(sn_speed) = supernova_speed
    {
        handles.supernova_cue.as_ref().map(|h| (h, 0.90, sn_speed))
    } else if has_kilonova {
        handles.kilonova_cue.as_ref().map(|h| (h, 0.85, 1.0))
    } else if has_bbh_chirp {
        handles.chirp_cue.as_ref().map(|h| (h, 0.80, 0.90))
    } else if has_ignition || has_civ_milestone || crisis_resolved_averted == Some(true) {
        handles
            .star_ignition_cue
            .as_ref()
            .map(|h| (h, 0.72, if has_civ_milestone { 1.18 } else { 1.0 }))
    } else if has_countermeasure {
        handles.probe_launch_cue.as_ref().map(|h| (h, 0.70, 1.15))
    } else if has_crisis_start {
        handles.pulsar_cue.as_ref().map(|h| (h, 0.68, 1.25))
    } else if crisis_resolved_averted == Some(false) {
        handles.chirp_cue.as_ref().map(|h| (h, 0.65, 1.0))
    } else if let Some((imp_scale, imp_speed)) = impact_params {
        handles
            .giant_impact_cue
            .as_ref()
            .map(|h| (h, imp_scale, imp_speed))
    } else if has_disruption {
        handles.chirp_cue.as_ref().map(|h| (h, 0.70, 1.0))
    } else if has_probe_launch {
        handles.probe_launch_cue.as_ref().map(|h| (h, 0.62, 1.0))
    } else if has_pulse {
        handles.pulsar_cue.as_ref().map(|h| (h, 0.55, 1.0))
    } else {
        None
    };

    if let Some((cue, scale, speed)) = selected {
        let vol = (audio_state.master_volume * scale * (1.0 + 0.2 * audio_state.proximity_boost))
            .clamp(0.0, 1.0);
        commands.spawn((
            AudioPlayer::new(cue.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(vol))
                .with_speed(speed),
        ));
        *last_cue_time = visual_time;
    }
}
