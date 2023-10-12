use std::collections::hash_map::OccupiedEntry;

use log::*;
use screeps::{objects::Creep, prelude::*, ErrorCode, ResourceType};

use crate::CreepTarget;

pub fn run(creep: &Creep, entry: OccupiedEntry<String, CreepTarget>) {
    let creep_target = entry.get();
    match creep_target {
        CreepTarget::Upgrade(controller_id)
            if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
        {
            if let Some(controller) = controller_id.resolve() {
                creep
                    .upgrade_controller(&controller)
                    .unwrap_or_else(|e| match e {
                        ErrorCode::NotInRange => {
                            let _ = creep.move_to(&controller);
                        }
                        _ => {
                            warn!("couldn't upgrade: {:?}", e);
                            entry.remove();
                        }
                    });
            } else {
                entry.remove();
            }
        }
        CreepTarget::Harvest(source_id)
            if creep.store().get_free_capacity(Some(ResourceType::Energy)) > 0 =>
        {
            if let Some(source) = source_id.resolve() {
                if creep.pos().is_near_to(source.pos()) {
                    creep.harvest(&source).unwrap_or_else(|e| {
                        warn!("couldn't harvest: {:?}", e);
                        entry.remove();
                    });
                } else {
                    let _ = creep.move_to(&source);
                }
            } else {
                entry.remove();
            }
        }
        _ => {
            entry.remove();
        }
    };
}
