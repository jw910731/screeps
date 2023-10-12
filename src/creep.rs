use log::*;
use rand::{rngs::SmallRng, seq::SliceRandom, Rng};
use screeps::{
    constants::Part,
    enums::StructureObject,
    find, game,
    local::ObjectId,
    objects::{Creep, Source, StructureController},
    prelude::*,
    ConstructionSite, ErrorCode, MoveToOptions, PolyStyle, ResourceType, StructureExtension,
    StructureSpawn,
};
use serde::{Deserialize, Serialize};

const CREEP_THRESHOLD: usize = 6;

// this enum will represent a creep's lock on a specific target object, storing a js reference
// to the object id so that we can grab a fresh reference to the object each successive tick,
// since screeps game objects become 'stale' and shouldn't be used beyond the tick they were fetched
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreepTarget {
    Upgrade(ObjectId<StructureController>),
    Harvest(ObjectId<Source>),
    Charge,
    Construct(ObjectId<ConstructionSite>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreepMemory {
    target: Option<CreepTarget>,
}

pub fn run(creep: &Creep, memory: &mut CreepMemory, rng: &mut SmallRng) {
    match &memory.target {
        Some(target) => {
            match target {
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
                                    memory.target = None;
                                }
                            });
                    } else {
                        memory.target = None;
                    }
                }
                CreepTarget::Harvest(source_id)
                    if creep.store().get_free_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(source) = source_id.resolve() {
                        if creep.pos().is_near_to(source.pos()) {
                            creep.harvest(&source).unwrap_or_else(|e| {
                                warn!("couldn't harvest: {:?}", e);
                                memory.target = None;
                            });
                        } else {
                            let _ = creep.move_to(&source);
                        }
                    } else {
                        memory.target = None;
                    }
                }
                CreepTarget::Charge
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    let room = creep.room().expect("couldn't resolve creep room");
                    let spawns = room.find(find::MY_SPAWNS, None);
                    let spawn = spawns.first().unwrap();
                    if room.energy_available() >= 300 {
                        let structures = room.find(find::STRUCTURES, None);
                        let mut extension: Option<StructureExtension> = None;
                        for structure in structures {
                            if let StructureObject::StructureExtension(ext) = structure {
                                extension = Some(ext);
                                break;
                            }
                        }
                        if let Some(extension) = extension {
                            creep
                                .transfer(
                                    &extension,
                                    ResourceType::Energy,
                                    Some(
                                        creep.store().get_used_capacity(Some(ResourceType::Energy)),
                                    ),
                                )
                                .unwrap_or_else(|e| match e {
                                    ErrorCode::NotInRange => {
                                        let _ = creep.move_to(&extension);
                                    }
                                    _ => {
                                        warn!("couldn't transfer energy to spawn: {:?}", e);
                                        memory.target = None;
                                    }
                                });
                        }
                    } else {
                        creep
                            .transfer(
                                spawn,
                                ResourceType::Energy,
                                Some(creep.store().get_used_capacity(Some(ResourceType::Energy))),
                            )
                            .unwrap_or_else(|e| match e {
                                ErrorCode::NotInRange => {
                                    let _ = creep.move_to(spawn);
                                }
                                _ => {
                                    warn!("couldn't transfer energy to spawn: {:?}", e);
                                    memory.target = None;
                                }
                            });
                    }
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) <= 0 {
                        memory.target = None;
                    }
                }
                CreepTarget::Construct(construct_id)
                    if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 =>
                {
                    if let Some(site) = construct_id.resolve() {
                        creep.build(&site).unwrap_or_else(|e| match e {
                            ErrorCode::NotInRange => {
                                let opt = MoveToOptions::new()
                                    .visualize_path_style(PolyStyle::default().stroke("#ffffff"));
                                let _ = creep.move_to_with_options(&site, Some(opt));
                            }
                            _ => {
                                warn!("couldn't build: {:?}", e);
                                memory.target = None;
                            }
                        });
                    } else {
                        memory.target = None;
                    }
                }
                _ => {
                    memory.target = None;
                }
            };
        }
        None => {
            // no target, let's find one depending on if we have energy
            let room = creep.room().expect("couldn't resolve creep room");
            if creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0 {
                let chance = if room.find(find::CONSTRUCTION_SITES, None).len() > 0 {
                    0.3
                } else {
                    0.7
                };
                if rng.gen_bool(chance) {
                    // upgrade controller
                    for structure in room.find(find::STRUCTURES, None).iter() {
                        if let StructureObject::StructureController(controller) = structure {
                            memory.target = Some(CreepTarget::Upgrade(controller.id()));
                            break;
                        }
                    }
                } else {
                    if rng.gen_bool(0.3) {
                        memory.target = Some(CreepTarget::Charge);
                    }
                    // exist construction site
                    else if let Some(site) = room.find(find::CONSTRUCTION_SITES, None).first() {
                        memory.target = Some(CreepTarget::Construct(site.try_id().unwrap()));
                    }
                }
            } else if let Some(source) = room.find(find::SOURCES_ACTIVE, None).choose(rng) {
                memory.target = Some(CreepTarget::Harvest(source.id()));
            }
        }
    }
}

pub fn spawn(spawn: &StructureSpawn, additional: &mut i32) {
    debug!("running spawn {}", String::from(spawn.name()));

    let body = [Part::Move, Part::Move, Part::Carry, Part::Work];
    if spawn.room().unwrap().energy_available() >= body.iter().map(|p| p.cost()).sum()
        && game::creeps().values().collect::<Vec<_>>().len() < CREEP_THRESHOLD
    {
        // create a unique name, spawn.
        let name_base = game::time();
        let name = format!("{}-{}", name_base, additional);
        // note that this bot has a fatal flaw; spawning a creep
        // creates Memory.creeps[creep_name] which will build up forever;
        // these memory entries should be prevented (todo doc link on how) or cleaned up
        match spawn.spawn_creep(&body, &name) {
            Ok(()) => *additional += 1,
            Err(e) => warn!("couldn't spawn: {:?}", e),
        }
    }
}
