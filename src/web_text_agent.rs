//! WASM text input agent — bridges browser virtual keyboard to egui.
//!
//! Creates a hidden `<input>` element that captures text input, IME composition,
//! and key events from mobile virtual keyboards. egui's `PlatformOutput::ime`
//! signals when a TextEdit has focus; we focus/blur the hidden input accordingly
//! to show/dismiss the virtual keyboard.
//!
//! Adapted from eframe's `text_agent.rs` for use without eframe.

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Events captured by the text agent, to be drained into egui each frame.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub enum TextAgentEvent {
    /// Regular text input (non-composing)
    Text(String),
    /// IME composition started
    ImeEnabled,
    /// IME composition preview
    ImePreedit(String),
    /// IME composition committed
    ImeCommit(String),
}

#[cfg(target_arch = "wasm32")]
pub struct WebTextAgent {
    input: web_sys::HtmlInputElement,
    events: Rc<RefCell<Vec<TextAgentEvent>>>,
    is_focused: bool,
    /// Shared with the touchend handler so it only focuses during keyboard input
    wants_keyboard_flag: Rc<std::cell::Cell<bool>>,
}

#[cfg(target_arch = "wasm32")]
impl WebTextAgent {
    /// Create and install the hidden text input agent.
    /// Call once during WASM initialization.
    pub fn install() -> Self {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("Failed to get document");

        // Create hidden <input> element
        let input = document
            .create_element("input")
            .unwrap()
            .dyn_into::<web_sys::HtmlInputElement>()
            .unwrap();
        input.set_type("text");
        let _ = input.set_attribute("autocapitalize", "off");
        let _ = input.set_attribute("autocomplete", "off");
        let _ = input.set_attribute("autocorrect", "off");
        let _ = input.set_attribute("spellcheck", "false");

        // Style: invisible but focusable (display:none would prevent focus)
        let style = input.style();
        let _ = style.set_property("position", "absolute");
        let _ = style.set_property("top", "0");
        let _ = style.set_property("left", "0");
        let _ = style.set_property("width", "1px");
        let _ = style.set_property("height", "1px");
        let _ = style.set_property("opacity", "0");
        let _ = style.set_property("border", "none");
        let _ = style.set_property("outline", "none");
        let _ = style.set_property("background", "transparent");
        let _ = style.set_property("caret-color", "transparent");
        let _ = style.set_property("color", "transparent");
        // Prevent iOS from scrolling to the input
        let _ = style.set_property("font-size", "16px"); // prevents iOS zoom on focus

        // Append to body
        document.body().unwrap().append_child(&input).unwrap();

        let events: Rc<RefCell<Vec<TextAgentEvent>>> = Rc::new(RefCell::new(Vec::new()));

        // input event — regular text input
        {
            let events = events.clone();
            let input_clone = input.clone();
            let on_input = Closure::<dyn FnMut(web_sys::InputEvent)>::new(
                move |event: web_sys::InputEvent| {
                    let text = input_clone.value();
                    // Android Gboard fix: blur/focus cycle to reset suggestions
                    if !event.is_composing() {
                        let _ = input_clone.blur();
                        let _ = input_clone.focus();
                    }
                    if !text.is_empty() && !event.is_composing() {
                        input_clone.set_value("");
                        events.borrow_mut().push(TextAgentEvent::Text(text));
                    }
                },
            );
            input
                .add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref())
                .unwrap();
            on_input.forget();
        }

        // compositionstart — IME started
        {
            let events = events.clone();
            let input_clone = input.clone();
            let handler = Closure::<dyn FnMut(web_sys::CompositionEvent)>::new(
                move |_: web_sys::CompositionEvent| {
                    input_clone.set_value("");
                    events.borrow_mut().push(TextAgentEvent::ImeEnabled);
                },
            );
            input
                .add_event_listener_with_callback(
                    "compositionstart",
                    handler.as_ref().unchecked_ref(),
                )
                .unwrap();
            handler.forget();
        }

        // compositionupdate — IME preview
        {
            let events = events.clone();
            let handler = Closure::<dyn FnMut(web_sys::CompositionEvent)>::new(
                move |event: web_sys::CompositionEvent| {
                    if let Some(text) = event.data() {
                        events.borrow_mut().push(TextAgentEvent::ImePreedit(text));
                    }
                },
            );
            input
                .add_event_listener_with_callback(
                    "compositionupdate",
                    handler.as_ref().unchecked_ref(),
                )
                .unwrap();
            handler.forget();
        }

        // compositionend — IME committed
        {
            let events = events.clone();
            let input_clone = input.clone();
            let handler = Closure::<dyn FnMut(web_sys::CompositionEvent)>::new(
                move |event: web_sys::CompositionEvent| {
                    if let Some(text) = event.data() {
                        input_clone.set_value("");
                        events.borrow_mut().push(TextAgentEvent::ImeCommit(text));
                    }
                },
            );
            input
                .add_event_listener_with_callback(
                    "compositionend",
                    handler.as_ref().unchecked_ref(),
                )
                .unwrap();
            handler.forget();
        }

        // Forward keydown events from hidden input to the canvas so winit sees them
        // (when hidden input has focus, canvas doesn't receive key events)
        {
            let canvas_id = "canvas".to_string();
            let handler = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
                move |event: web_sys::KeyboardEvent| {
                    let document = web_sys::window().unwrap().document().unwrap();
                    if let Some(canvas) = document.get_element_by_id(&canvas_id) {
                        // Re-dispatch key event to canvas
                        let new_event = web_sys::KeyboardEvent::new_with_keyboard_event_init_dict(
                            event.type_().as_str(),
                            web_sys::KeyboardEventInit::new()
                                .key(&event.key())
                                .code(&event.code())
                                .ctrl_key(event.ctrl_key())
                                .shift_key(event.shift_key())
                                .alt_key(event.alt_key())
                                .meta_key(event.meta_key()),
                        )
                        .unwrap();
                        let _ = canvas.dispatch_event(&new_event);
                    }
                },
            );
            input
                .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref())
                .unwrap();
            handler.forget();
        }

        // iOS Safari requires .focus() inside a user gesture handler for the
        // virtual keyboard to appear. We share a flag that update_focus() sets
        // when egui wants keyboard input; the touchend handler checks it.
        let wants_keyboard_flag = Rc::new(std::cell::Cell::new(false));
        {
            let input_clone = input.clone();
            let flag = wants_keyboard_flag.clone();
            let handler = Closure::<dyn FnMut(web_sys::Event)>::new(
                move |_: web_sys::Event| {
                    if flag.get() {
                        let _ = input_clone.focus();
                    }
                },
            );
            if let Some(canvas) = document.get_element_by_id("canvas") {
                canvas
                    .add_event_listener_with_callback("touchend", handler.as_ref().unchecked_ref())
                    .unwrap();
                handler.forget();
            }
        }

        Self {
            input,
            events,
            is_focused: false,
            wants_keyboard_flag,
        }
    }

    /// Update focus state based on whether egui wants keyboard input.
    /// Call after each egui frame.
    pub fn update_focus(&mut self, wants_keyboard: bool) {
        // Update the shared flag so the touchend handler knows whether to focus.
        // The touchend handler runs in a user gesture context (required by iOS
        // for the virtual keyboard to appear), so the actual .focus() happens there.
        self.wants_keyboard_flag.set(wants_keyboard);

        if wants_keyboard && !self.is_focused {
            log::info!("WebTextAgent: wants keyboard (will focus on next touch)");
            // Don't call .focus() here — it's inside rAF, not a user gesture,
            // so iOS won't show the keyboard. The touchend handler will do it.
            self.is_focused = true;
        } else if !wants_keyboard && self.is_focused {
            log::info!("WebTextAgent: blurring (keyboard should dismiss)");
            let _ = self.input.blur();
            self.input.set_value("");
            self.is_focused = false;
        }
    }

    /// Drain pending events into egui's raw input.
    /// Call before each egui frame.
    pub fn drain_events(&self, raw_input: &mut egui::RawInput) {
        let events: Vec<TextAgentEvent> = self.events.borrow_mut().drain(..).collect();
        for event in events {
            match event {
                TextAgentEvent::Text(text) => {
                    raw_input.events.push(egui::Event::Text(text));
                }
                TextAgentEvent::ImeEnabled => {
                    raw_input
                        .events
                        .push(egui::Event::Ime(egui::ImeEvent::Enabled));
                }
                TextAgentEvent::ImePreedit(text) => {
                    raw_input
                        .events
                        .push(egui::Event::Ime(egui::ImeEvent::Preedit(text)));
                }
                TextAgentEvent::ImeCommit(text) => {
                    raw_input
                        .events
                        .push(egui::Event::Ime(egui::ImeEvent::Commit(text)));
                }
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for WebTextAgent {
    fn drop(&mut self) {
        self.input.remove();
    }
}
