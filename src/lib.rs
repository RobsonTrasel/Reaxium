use wasm_bindgen::prelude::*;
use std::collections::{HashMap, VecDeque};
use js_sys::{Function, Promise};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen::JsValue;
use web_sys::window;

#[wasm_bindgen]
extern "C"
{
    #[wasm_bindgen(js_name = setTimeout)]
    fn schedule_execution(exec: &JsValue, ms: i32) -> i32;
    #[wasm_bindgen(js_name = clearTimeout)]
    fn cancel_scheduled(timer_ref: i32);
}

#[derive(Default)]
struct PostponeData
{
    timer_ref: i32,
    interval_ms: u32
}

#[derive(Default)]
struct PaceData
{
    cooldown: bool,
    interval_ms: u32,
    timer_ref: i32,
}

#[wasm_bindgen]
pub struct SystemBridge {
    listeners: HashMap<String, VecDeque<Function>>,
    queues: HashMap<String, VecDeque<JsValue>>,
    queue_limit: usize,
    postpone_registry: HashMap<String, PostponeData>,
    pace_registry: HashMap<String, PaceData>,
    combined_streams: HashMap<(String, String), String>,
}


