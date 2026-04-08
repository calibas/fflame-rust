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
    cancelled: Rc<std::cell::Cell<bool>>,
}

#[cfg(target_arch = "wasm32")]
impl WebTextAgent {
    /// Install the submit/cancel listeners. Call once during WASM init.
    pub fn install() -> Self {
        let submitted_value: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let cancelled: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(false));

        let document = web_sys::window().unwrap().document().unwrap();

        // Listen for vkb-submit from JS
        {
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
            document
                .add_event_listener_with_callback("vkb-submit", handler.as_ref().unchecked_ref())
                .unwrap();
            handler.forget();
        }

        // Listen for vkb-cancel from JS — just defocus, no value change
        {
            let cancelled = cancelled.clone();
            let handler = Closure::<dyn FnMut(web_sys::Event)>::new(
                move |_: web_sys::Event| {
                    cancelled.set(true);
                },
            );
            document
                .add_event_listener_with_callback("vkb-cancel", handler.as_ref().unchecked_ref())
                .unwrap();
            handler.forget();
        }

        Self {
            submitted_value,
            cancelled,
        }
    }

    /// Dispatch vkb-open to JS.
    pub fn open(&self, field_type: &str, value: &str, min: Option<f64>, max: Option<f64>, required: bool) {
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
    }

    /// Take the submitted value, if any. Called before each egui frame.
    pub fn take_submitted(&self) -> Option<String> {
        self.submitted_value.borrow_mut().take()
    }

    /// Take the cancelled flag. Returns true if the user cancelled the input.
    pub fn take_cancelled(&self) -> bool {
        let was_cancelled = self.cancelled.get();
        self.cancelled.set(false);
        was_cancelled
    }
}
