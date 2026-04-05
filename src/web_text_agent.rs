//! WASM virtual keyboard bridge.
//!
//! Thin Rust↔JS bridge using CustomEvents:
//! - Dispatches "vkb-open" to JS with field config (type, value, min, max, required)
//! - Listens for "vkb-submit" from JS with the edited value
//! - JS handles all DOM/input/styling (js/vkb.js + css/vkb.css)

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
pub struct WebTextAgent {
    submitted_value: Rc<RefCell<Option<String>>>,
    is_open: bool,
}

#[cfg(target_arch = "wasm32")]
impl WebTextAgent {
    /// Install the submit listener. Call once during WASM init.
    pub fn install() -> Self {
        let submitted_value: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

        // Listen for vkb-submit from JS
        let submitted = submitted_value.clone();
        let handler = Closure::<dyn FnMut(web_sys::CustomEvent)>::new(
            move |event: web_sys::CustomEvent| {
                if let Some(detail) = event.detail().as_ref().dyn_ref::<js_sys::Object>() {
                    let value = js_sys::Reflect::get(detail, &"value".into())
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_default();
                    *submitted.borrow_mut() = Some(value);
                }
            },
        );
        let document = web_sys::window().unwrap().document().unwrap();
        document
            .add_event_listener_with_callback("vkb-submit", handler.as_ref().unchecked_ref())
            .unwrap();
        handler.forget();

        Self {
            submitted_value,
            is_open: false,
        }
    }

    /// Dispatch vkb-open to JS. Called from a touchend handler (user gesture context)
    /// or from the render loop when egui wants keyboard input.
    pub fn open(&mut self, field_type: &str, value: &str, min: Option<f64>, max: Option<f64>, required: bool) {
        if self.is_open {
            return;
        }
        let document = web_sys::window().unwrap().document().unwrap();
        let detail = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&detail, &"type".into(), &field_type.into());
        let _ = js_sys::Reflect::set(&detail, &"value".into(), &value.into());
        if let Some(min) = min {
            let _ = js_sys::Reflect::set(&detail, &"min".into(), &min.into());
        }
        if let Some(max) = max {
            let _ = js_sys::Reflect::set(&detail, &"max".into(), &max.into());
        }
        let _ = js_sys::Reflect::set(&detail, &"required".into(), &required.into());

        let mut init = web_sys::CustomEventInit::new();
        init.detail(&detail);
        let event = web_sys::CustomEvent::new_with_event_init_dict("vkb-open", &init).unwrap();
        let _ = document.dispatch_event(&event);
        self.is_open = true;
    }

    /// Take the submitted value, if any. Called before each egui frame.
    pub fn take_submitted(&mut self) -> Option<String> {
        let value = self.submitted_value.borrow_mut().take();
        if value.is_some() {
            self.is_open = false;
        }
        value
    }

    /// Whether the VKB overlay is currently shown.
    pub fn is_open(&self) -> bool {
        self.is_open
    }
}
