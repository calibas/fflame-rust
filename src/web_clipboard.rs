//! WASM clipboard bridge — connects browser clipboard to egui's event system.
//!
//! egui-winit's `clipboard` feature uses `arboard` (native-only, won't compile for WASM).
//! This module bridges browser clipboard events into egui manually:
//!
//! - **Paste**: Listens for browser `paste` events, captures text, injects `egui::Event::Paste`
//! - **Copy**: After each egui frame, writes `PlatformOutput::copied_text` to browser clipboard
//! - **Ctrl+V interception**: Stops propagation in capture phase so winit doesn't consume
//!   the keydown, allowing the browser to fire the paste event normally

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
pub struct WebClipboard {
    paste_buffer: Rc<RefCell<Option<String>>>,
}

#[cfg(target_arch = "wasm32")]
impl WebClipboard {
    /// Install clipboard event listeners on the document.
    /// Call once during app initialization.
    pub fn install() -> Self {
        let paste_buffer = Rc::new(RefCell::new(None));

        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("Failed to get document");

        // Listen for paste events — captures clipboard text from the browser
        {
            let buffer = paste_buffer.clone();
            let on_paste = Closure::wrap(Box::new(move |event: web_sys::ClipboardEvent| {
                if let Some(data) = event.clipboard_data() {
                    if let Ok(text) = data.get_data("text") {
                        let text = text.replace("\r\n", "\n");
                        if !text.is_empty() {
                            *buffer.borrow_mut() = Some(text);
                            event.prevent_default();
                        }
                    }
                }
            }) as Box<dyn FnMut(web_sys::ClipboardEvent)>);

            let _ = document.add_event_listener_with_callback(
                "paste",
                on_paste.as_ref().unchecked_ref(),
            );
            on_paste.forget();
        }

        // Intercept Ctrl+V / Cmd+V in the capture phase.
        // This prevents winit from seeing the keydown (and calling preventDefault),
        // which allows the browser to fire the paste event normally.
        {
            let on_keydown = Closure::wrap(Box::new(move |event: web_sys::KeyboardEvent| {
                if (event.ctrl_key() || event.meta_key()) && event.key() == "v" {
                    event.stop_propagation();
                }
            }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);

            let _ = document.add_event_listener_with_callback_and_bool(
                "keydown",
                on_keydown.as_ref().unchecked_ref(),
                true, // capture phase — fires before winit's bubble-phase handler
            );
            on_keydown.forget();
        }

        Self { paste_buffer }
    }

    /// Take any pending paste text from the browser clipboard.
    /// Returns `Some(text)` if a paste event occurred since the last call.
    pub fn take_paste(&self) -> Option<String> {
        self.paste_buffer.borrow_mut().take()
    }

    /// Write text to the browser clipboard via `navigator.clipboard.writeText()`.
    /// Requires a secure context (HTTPS or localhost).
    pub fn copy_text(text: &str) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let navigator = window.navigator();
        // Safely check if clipboard API is available (undefined in non-secure contexts)
        let Ok(clipboard_val) = js_sys::Reflect::get(&navigator, &"clipboard".into()) else {
            return;
        };
        if clipboard_val.is_undefined() || clipboard_val.is_null() {
            return;
        }
        let clipboard: web_sys::Clipboard = clipboard_val.unchecked_into();
        let _ = clipboard.write_text(text);
    }
}
