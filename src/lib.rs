use wasm_bindgen::prelude::*;
use std::collections::{HashMap, VecDeque};
use js_sys::{Function, Promise};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen::JsValue;
use web_sys::window;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = setTimeout)]
    fn schedule_execution(exec: &JsValue, ms: i32) -> i32;
    #[wasm_bindgen(js_name = clearTimeout)]
    fn cancel_scheduled(timer_ref: i32);
}