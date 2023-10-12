use std::cell::RefCell;

use gloo_utils::format::JsValueSerdeExt;
use log::*;
use rand::{rngs::SmallRng, SeedableRng};
use screeps::{game, objects::Creep, prelude::*};
use wasm_bindgen::prelude::*;

use crate::creep::CreepMemory;

mod creep;
mod logging;

thread_local! {
    static RNG: RefCell<SmallRng> = RefCell::new(rand::rngs::SmallRng::seed_from_u64(0xcafebeef));
}

// add wasm_bindgen to any function you would like to expose for call from js
#[wasm_bindgen]
pub fn setup() {
    logging::setup_logging(logging::Info);
}

// to use a reserved name as a function name, use `js_name`:
#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
    debug!("loop starting! CPU: {}", game::cpu::get_used());

    debug!("running creeps");
    for creep in game::creeps().values() {
        run_creep(&creep);
    }

    let creep_name_list = game::creeps()
        .values()
        .map(|e| e.name())
        .collect::<Vec<_>>();
    for ent in js_sys::Object::entries(&screeps::memory::ROOT.to_owned()) {
        let arr = JsValue::dyn_into::<js_sys::Array>(ent).unwrap();
        let name: String = JsValue::into_serde(&arr.get(0)).unwrap();
        if creep_name_list.iter().find(|&e| *e == name) == None {
            js_sys::Reflect::delete_property(&screeps::memory::ROOT.to_owned(), &arr.get(0))
                .unwrap();
        }
    }

    debug!("running spawns");
    let mut additional = 0;
    for spawn in game::spawns().values() {
        creep::spawn(&spawn, &mut additional);
    }

    info!("done! cpu: {}", game::cpu::get_used())
}

fn run_creep(creep: &Creep) {
    if creep.spawning() {
        return;
    }
    let name = creep.name();
    debug!("running creep {}", name);

    let mut memory: CreepMemory = creep.memory().into_serde().unwrap();

    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        creep::run(creep, &mut memory, &mut rng);
    });

    creep.set_memory(&JsValue::from_serde(&memory).unwrap());
}
