//! Mobile browsers do not show a soft keyboard for text typed into a WebGL canvas. On native
//! Android/iOS, `Window::request_ime_update` wires egui's IME output into the OS; on wasm,
//! `winit-web` returns [`winit::window::ImeRequestError::NotSupported`] for every IME request.
//!
//! We keep a hidden `<input>` in the DOM, move it under egui's IME rectangle, focus it while a
//! [`egui::TextEdit`] is active, and forward `beforeinput` / `compositionend` into
//! [`egui::RawInput::events`] (same path as native `WindowEvent::Ime`).

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Event, ImeEvent, Key, Modifiers};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::HtmlInputElement;

const PROXY_ID: &str = "sow-wasm-ime-proxy";

pub(crate) fn should_use_dom_soft_keyboard() -> bool {
    web_sys::window()
        .map(|w| w.navigator().max_touch_points() > 0)
        .unwrap_or(false)
}

fn backspace_event() -> Event {
    Event::Key {
        key: Key::Backspace,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::default(),
    }
}

fn delete_event() -> Event {
    Event::Key {
        key: Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::default(),
    }
}

/// Sync DOM proxy with egui IME output (logical points, same space as [`egui::RawInput::screen_rect`]).
pub(crate) struct WasmImeBridge {
    pending: Rc<RefCell<Vec<Event>>>,
    input: HtmlInputElement,
    dom_active: bool,
}

impl WasmImeBridge {
    pub fn new() -> Self {
        let window = web_sys::window().expect("wasm window");
        let document = window.document().expect("wasm document");

        let input = document
            .create_element("input")
            .expect("create input")
            .dyn_into::<HtmlInputElement>()
            .expect("input element");

        input.set_id(PROXY_ID);
        input.set_type("text");
        input.set_autocomplete("off");
        let _ = input.set_attribute("autocapitalize", "off");
        let _ = input.set_attribute("autocorrect", "off");
        let _ = input.set_attribute("spellcheck", "false");
        let _ = input.set_attribute("enterkeyhint", "done");
        let _ = input.set_attribute("inputmode", "text");

        let style = input.style();
        let _ = style.set_property("position", "fixed");
        let _ = style.set_property("z-index", "2147483647");
        let _ = style.set_property("opacity", "0.04");
        let _ = style.set_property("font-size", "16px");
        let _ = style.set_property("color", "transparent");
        let _ = style.set_property("caret-color", "transparent");
        let _ = style.set_property("pointer-events", "none");
        let _ = style.set_property("margin", "0");
        let _ = style.set_property("padding", "0");
        let _ = style.set_property("border", "none");
        let _ = style.set_property("outline", "none");
        let _ = style.set_property("background", "transparent");
        let _ = style.set_property("display", "none");

        let pending: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));

        {
            let pending = pending.clone();
            let beforeinput = Closure::wrap(Box::new(move |e: web_sys::Event| {
                let input_type =
                    js_sys::Reflect::get(&e, &wasm_bindgen::JsValue::from_str("inputType"))
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_default();

                match input_type.as_str() {
                    "deleteContentBackward" | "deleteWordBackward" | "deleteSoftLineBackward" => {
                        pending.borrow_mut().push(backspace_event());
                        let _ = e.prevent_default();
                    }
                    "deleteContentForward" | "deleteWordForward" => {
                        pending.borrow_mut().push(delete_event());
                        let _ = e.prevent_default();
                    }
                    "insertText" | "insertLineBreak" | "insertReplacementText" => {
                        if let Some(data) =
                            js_sys::Reflect::get(&e, &wasm_bindgen::JsValue::from_str("data"))
                                .ok()
                                .and_then(|v| v.as_string())
                        {
                            if !data.is_empty() {
                                pending.borrow_mut().push(Event::Text(data));
                                let _ = e.prevent_default();
                            }
                        }
                    }
                    "insertCompositionText" | "deleteCompositionText" => {}
                    _ => {}
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback(
                    "beforeinput",
                    beforeinput.as_ref().unchecked_ref(),
                )
                .expect("beforeinput listener");
            beforeinput.forget();
        }

        {
            let pending = pending.clone();
            let compositionupdate = Closure::wrap(Box::new(move |e: web_sys::Event| {
                if let Some(data) =
                    js_sys::Reflect::get(&e, &wasm_bindgen::JsValue::from_str("data"))
                        .ok()
                        .and_then(|v| v.as_string())
                {
                    pending
                        .borrow_mut()
                        .push(Event::Ime(ImeEvent::Preedit(data)));
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback(
                    "compositionupdate",
                    compositionupdate.as_ref().unchecked_ref(),
                )
                .expect("compositionupdate listener");
            compositionupdate.forget();
        }

        {
            let pending = pending.clone();
            let compositionend = Closure::wrap(Box::new(move |e: web_sys::Event| {
                if let Some(data) =
                    js_sys::Reflect::get(&e, &wasm_bindgen::JsValue::from_str("data"))
                        .ok()
                        .and_then(|v| v.as_string())
                {
                    if !data.is_empty() {
                        pending
                            .borrow_mut()
                            .push(Event::Ime(ImeEvent::Commit(data)));
                    }
                }
            }) as Box<dyn FnMut(_)>);
            input
                .add_event_listener_with_callback(
                    "compositionend",
                    compositionend.as_ref().unchecked_ref(),
                )
                .expect("compositionend listener");
            compositionend.forget();
        }

        if let Some(body) = document.body() {
            body.append_child(&input).expect("append IME proxy");
        } else if let Some(de) = document.document_element() {
            de.append_child(&input).expect("append IME proxy");
        } else {
            panic!("document needs body or documentElement for IME proxy");
        }

        Self {
            pending,
            input,
            dom_active: false,
        }
    }

    pub fn drain_pending_into(&self, events: &mut Vec<Event>) {
        let mut buf = self.pending.borrow_mut();
        if buf.is_empty() {
            return;
        }
        events.extend(buf.drain(..));
    }

    pub fn sync_from_egui_ime(&mut self, ime: Option<egui::output::IMEOutput>) {
        if !should_use_dom_soft_keyboard() {
            self.hide();
            return;
        }

        let Some(ime_out) = ime else {
            self.hide();
            return;
        };

        let r = ime_out.rect;
        let w = r.width().max(48.0);
        let h = r.height().max(ime_out.cursor_rect.height()).max(24.0);

        let style = self.input.style();
        let _ = style.set_property("left", &format!("{}px", r.min.x));
        let _ = style.set_property("top", &format!("{}px", r.min.y));
        let _ = style.set_property("width", &format!("{}px", w));
        let _ = style.set_property("height", &format!("{}px", h));
        let _ = style.set_property("display", "block");

        let was_inactive = !self.dom_active;
        self.dom_active = true;

        if was_inactive {
            let _ = self.input.set_value("");
            schedule_double_raf_focus(&self.input);
        }
    }

    fn hide(&mut self) {
        if !self.dom_active {
            return;
        }
        self.dom_active = false;
        let _ = self.input.blur();
        let _ = self.input.set_value("");
        let _ = self.input.style().set_property("display", "none");
    }
}

fn schedule_double_raf_focus(input: &HtmlInputElement) {
    let win = web_sys::window().expect("window");
    let input1 = input.clone();
    let outer = Closure::wrap(Box::new(move || {
        let win = web_sys::window().expect("window");
        let input2 = input1.clone();
        let inner = Closure::wrap(Box::new(move || {
            let _ = input2.focus();
        }) as Box<dyn FnMut()>);
        let _ = win.request_animation_frame(inner.as_ref().unchecked_ref());
        inner.forget();
    }) as Box<dyn FnMut()>);
    let _ = win.request_animation_frame(outer.as_ref().unchecked_ref());
    outer.forget();
}

/// Best-effort: canvas must be focusable for some mobile browsers when tabbing
/// / accessibility tools run.
pub(crate) fn ensure_canvas_tabindex() {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let document = match window.document() {
        Some(d) => d,
        None => return,
    };
    if let Some(canvas) = document.get_element_by_id("blade") {
        let _ = canvas.set_attribute("tabindex", "0");
    }
}
