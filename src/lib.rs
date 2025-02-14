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
pub struct SystemBridge
{
    listeners: HashMap<String, VecDeque<Function>>,
    queues: HashMap<String, VecDeque<JsValue>>,
    queue_limit: usize,
    postpone_registry: HashMap<String, PostponeData>,
    pace_registry: HashMap<String, PaceData>,
    combined_streams: HashMap<(String, String), String>,
}

#[wasm_bindgen]
impl SystemBridge
{
    #[wasm_bindgen(constructor)]
    pub fn new(capacity: usize) -> SystemBridge
    {
        SystemBridge
        {
            listeners: HashMap::new(),
            queues: HashMap::new(),
            queue_limit: capacity,
            postpone_registry: HashMap::new(),
            pace_registry: HashMap::new(),
            combined_streams: HashMap::new(),
        }
    }

    #[wasm_bindgen]
    pub fn add_listener(&mut self, evt: String, callback: Function)
    {
        self.listeners
            .entry(evt)
            .or_insert_with(VecDeque::new)
            .push_back(callback);
    }

    #[wasm_bindgen]
    pub fn remove_listener(&mut self, evt: String, callback: Function)
    {
        if let Some(list) = self.listeners.get_mut(&evt) {
            list.retain(|cb| !cb.equals(&callback));
        }
    }

    #[wasm_bindgen]
    pub fn broadcast(&mut self, evt: String, payload: JsValue)
    {
        if let Some(combined) = self.probe_combination(&evt)
        {
            self.broadcast(combined, payload);
            return;
        }
        let queue = self.queues.entry(evt.clone()).or_insert_with(VecDeque::new);
        if queue.len() >= self.queue_limit
        {
            queue.pop_front();
        }
        queue.push_back(payload);

        if self.postpone_registry.contains_key(&evt)
        {
            self.queue_postpone(evt);
            return;
        }

        if let Some(pace) = self.pace_registry.get_mut(&evt)
        {
            if pace.cooldown {
                return;
            } else {
                self.flush_queue(&evt);
                self.queue_pace(evt);
                return;
            }
        }

        self.flush_queue(&evt);
    }

    fn flush_queue(&mut self, evt: &str)
    {
        if let Some(callbacks) = self.listeners.get(evt) {
            if let Some(queue) = self.queues.get_mut(evt) {
                while let Some(data) = queue.pop_front() {
                    for cb in callbacks {
                        let _ = cb.call1(&JsValue::NULL, &data);
                    }
                }
            }
        }
    }

    #[wasm_bindgen]
    pub fn broadcast_async(&self, evt: String, payload: JsValue) -> Promise
    {
        let local_listeners = match self.listeners.get(&evt)
        {
            Some(c) => c.clone(),
            None => VecDeque::new(),
        };

        js_sys::Promise::new(&mut |resolve, reject| {
            let resolve = js_sys::Function::from(resolve);
            let reject = js_sys::Function::from(reject);

            wasm_bindgen_futures::spawn_local(async move {
                for cb in local_listeners {
                    let p: Promise = match cb.call1(&JsValue::NULL, &payload) {
                        Ok(val) => val.into(),
                        Err(e) => {
                            let _ = reject.call1(&JsValue::NULL, &e);
                            return;
                        }
                    };
                    let future = JsFuture::from(p);
                    if let Err(err) = future.await {
                        let _ = reject.call1(&JsValue::NULL, &err);
                        return;
                    }
                }
                let _ = resolve.call0(&JsValue::NULL);
            });
        })
    }

    #[wasm_bindgen]
    pub fn transform(&mut self, evt: String, transformer: Function)
    {
        if let Some(callbacks) = self.listeners.get_mut(&evt) {
            let mut adjusted = VecDeque::new();
            for cb in callbacks.iter() {
                let new_cb = transformer.call1(&JsValue::NULL, cb).unwrap().into();
                adjusted.push_back(new_cb);
            }
            *callbacks = adjusted;
        }
    }

    #[wasm_bindgen]
    pub fn conditional_filter(&mut self, evt: String, condition: Function)
    {
        if let Some(callbacks) = self.listeners.get_mut(&evt) {
            callbacks.retain(|cb| {
                condition.call1(&JsValue::NULL, cb)
                    .unwrap()
                    .as_bool()
                    .unwrap_or(false)
            });
        }
    }

    #[wasm_bindgen]
    pub fn pick(&mut self, evt: String, limit: u32)
    {
        if let Some(callbacks) = self.listeners.get_mut(&evt) {
            callbacks.truncate(limit as usize);
        }
    }

    #[wasm_bindgen]
    pub fn delayed_broadcast(&mut self, evt: String, ms: u32)
    {
        let info = PostponeData {
            timer_ref: -1,
            interval_ms: ms,
        };
        self.postpone_registry.insert(evt, info);
    }

    fn queue_postpone(&mut self, evt: String)
    {
        if let Some(postpone_info) = self.postpone_registry.get_mut(&evt) {
            if postpone_info.timer_ref >= 0 {
                cancel_scheduled(postpone_info.timer_ref);
            }
            let cloned_evt = evt.clone();
            let delay = postpone_info.interval_ms as i32;
            let closure = Closure::wrap(Box::new(move || {
                if let Some(bridge) = fetch_bridge() {
                    bridge.flush_queue(&cloned_evt);
                }
            }) as Box<dyn FnMut()>);

            let timer_id = schedule_execution(closure.as_ref().unchecked_ref(), delay);
            postpone_info.timer_ref = timer_id;
            closure.forget();
        }
    }

    #[wasm_bindgen]
    pub fn paced_broadcast(&mut self, evt: String, ms: u32)
    {
        let info = PaceData {
            cooldown: false,
            interval_ms: ms,
            timer_ref: -1,
        };
        self.pace_registry.insert(evt, info);
    }

    fn queue_pace(&mut self, evt: String)
    {
        if let Some(pace_info) = self.pace_registry.get_mut(&evt) {
            pace_info.cooldown = true;
            let delay = pace_info.interval_ms as i32;
            let cloned_evt = evt.clone();
            let closure = Closure::wrap(Box::new(move || {
                if let Some(bridge) = fetch_bridge() {
                    if let Some(pace_local) = bridge.pace_registry.get_mut(&cloned_evt) {
                        pace_local.cooldown = false;
                    }
                }
            }) as Box<dyn FnMut()>);

            let timer_id = schedule_execution(closure.as_ref().unchecked_ref(), delay);
            pace_info.timer_ref = timer_id;
            closure.forget();
        }
    }

    #[wasm_bindgen]
    pub fn unify(&mut self, evt_a: String, evt_b: String, target: String)
    {
        self.combined_streams.insert((evt_a, evt_b), target);
    }

    fn probe_combination(&self, current_evt: &String) -> Option<String>
    {
        for ((a, b), dest) in &self.combined_streams
        {
            if current_evt == a || current_evt == b {
                return Some(dest.clone());
            }
        }
        None
    }
}

static mut GLOBAL_BRIDGE: Option<SystemBridge> = None;

#[wasm_bindgen]
pub fn assign_bridge(bridge: SystemBridge) {
    unsafe {
        GLOBAL_BRIDGE = Some(bridge);
    }
}

fn fetch_bridge() -> Option<&'static mut SystemBridge> {
    unsafe { GLOBAL_BRIDGE.as_mut() }
}

