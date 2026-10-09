use crate::simulation::accretion::events::{AccretionMergeEvent, TidalDisruptionEvent};
use crate::simulation::components::*;
use crate::simulation::resources::*;
use bevy::prelude::*;
use hashbrown::HashSet;

pub fn update_black_hole_accretion_jets(
    mut query: Query<&mut RelativisticJetState>,
    mut merge_reader: MessageReader<AccretionMergeEvent>,
    mut tde_reader: MessageReader<TidalDisruptionEvent>,
    time: Res<SimTime>,
) {
    let dt_yr = time.current_dt_yr as f32;

    // Decrease timers
    for mut jet in query.iter_mut() {
        if jet.accretion_timer_years > 0.0 {
            jet.accretion_timer_years -= dt_yr;
            if jet.accretion_timer_years <= 0.0 {
                jet.accretion_timer_years = 0.0;
                jet.is_accreting = false;
            }
        }
    }

    // Set timers on events
    let mut accreting_entities = HashSet::new();
    for ev in merge_reader.read() {
        if ev.new_body_type == BodyType::BlackHole {
            accreting_entities.insert(ev.primary_entity);
            accreting_entities.insert(ev.secondary_entity);
        }
    }

    for ev in tde_reader.read() {
        accreting_entities.insert(ev.bh_entity);
    }

    for entity in accreting_entities {
        if let Ok(mut jet) = query.get_mut(entity) {
            jet.is_accreting = true;
            jet.accretion_timer_years = 5.0; // Stays active for 5 simulation years
        }
    }
}
