//! wasm32 implementation of [`TainoEditor`]. Ported from upstream
//! `taino-edit-dioxus` 0.5.3 (MIT/Apache), trimmed to the text-editing surface
//! the newsroom needs (no table/view-plugin or pointer wiring) and updated for
//! Dioxus 0.8's `MountedData` API.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use taino_edit_core::{EditorState, KeyPress, Keymap, Transaction, Transform};
use taino_edit_dom::EditorView;

use crate::KeymapProp;

/// A Dioxus component rendering an editor backed by a [`Signal<EditorState>`].
/// On every signal change the mounted DOM is reconciled; browser edits feed back
/// into the signal.
#[component]
pub fn TainoEditor(state: Signal<EditorState>, #[props(default)] keymap: KeymapProp) -> Element {
    // The mounted view + its event closures live here across renders.
    let mut runtime: Signal<Option<EditorRuntime>> = use_signal(|| None);

    // On every state change, patch the DOM and re-sync the selection.
    use_effect(move || {
        let snapshot = state.read().clone();
        if let Some(rt) = runtime.write().as_mut() {
            rt.view.update(snapshot.doc().clone());
            let mirrored_from_dom = rt.selection_from_dom.replace(false);
            if !mirrored_from_dom
                && rt.view.has_focus()
                && rt.view.read_selection() != Some(snapshot.selection())
            {
                rt.applying_selection.set(true);
                let _ = rt.view.set_selection(snapshot.selection());
                rt.applying_selection.set(false);
            }
        }
    });

    let on_mounted = move |evt: Event<MountedData>| {
        // Obtain the backing web_sys::Element from the mounted node.
        let Some(element) = evt.data().downcast::<web_sys::Element>().cloned() else {
            return;
        };
        let snapshot = state.read().clone();
        let view = EditorView::mount(
            snapshot.doc().clone(),
            snapshot.schema().clone(),
            element.clone(),
        );
        let applying = Rc::new(Cell::new(false));
        let from_dom = Rc::new(Cell::new(false));
        let keymap_cell: Rc<RefCell<Option<Keymap>>> = Rc::new(RefCell::new(keymap.take()));
        let closures = wire_events(
            &element,
            runtime,
            state,
            applying.clone(),
            from_dom.clone(),
            keymap_cell,
        );
        runtime.set(Some(EditorRuntime {
            view,
            closures,
            applying_selection: applying,
            selection_from_dom: from_dom,
        }));
    };

    rsx! {
        div {
            class: "taino-editor",
            onmounted: on_mounted,
        }
    }
}

/// What a mounted `TainoEditor` owns; dropping it frees the view + detaches every
/// listener.
struct EditorRuntime {
    view: EditorView,
    #[allow(dead_code)] // kept alive so the listeners they back stay attached.
    closures: Vec<EventCloser>,
    applying_selection: Rc<Cell<bool>>,
    selection_from_dom: Rc<Cell<bool>>,
}

/// A `Closure` registered on a DOM target; removed on drop.
struct EventCloser {
    event: &'static str,
    target: web_sys::EventTarget,
    closure: Closure<dyn FnMut(web_sys::Event)>,
}

impl Drop for EventCloser {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.event, self.closure.as_ref().unchecked_ref());
    }
}

fn push_listener(
    closers: &mut Vec<EventCloser>,
    target: web_sys::EventTarget,
    event: &'static str,
    closure: Closure<dyn FnMut(web_sys::Event)>,
) {
    if target
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .is_ok()
    {
        closers.push(EventCloser {
            event,
            target,
            closure,
        });
    }
}

fn wire_events(
    el: &web_sys::Element,
    runtime: Signal<Option<EditorRuntime>>,
    state: Signal<EditorState>,
    applying_selection: Rc<Cell<bool>>,
    selection_from_dom: Rc<Cell<bool>>,
    keymap_cell: Rc<RefCell<Option<Keymap>>>,
) -> Vec<EventCloser> {
    let target: web_sys::EventTarget = el.clone().into();
    let mut closers: Vec<EventCloser> = Vec::new();

    // `input`: text typed or deleted in a text node.
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        if let Some(Some(t)) = with_view(runtime, |v| v.read_dom_changes()) {
            apply_transform(state, &t);
        }
    });
    push_listener(&mut closers, target.clone(), "input", cb);

    // `keydown`: with a keymap installed, the editor owns keyboard editing.
    let km_for_keydown = keymap_cell;
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Ok(kev) = ev.dyn_into::<web_sys::KeyboardEvent>() else {
            return;
        };
        let key = KeyPress {
            key: kev.key(),
            ctrl: kev.ctrl_key(),
            alt: kev.alt_key(),
            shift: kev.shift_key(),
            meta: kev.meta_key(),
        };
        let mut cur = state.peek().clone();
        if let Some(Some(live)) = with_view(runtime, |v| v.read_selection()) {
            if live != cur.selection() {
                let mut tx = cur.tr();
                tx.set_selection(live);
                tx.no_history();
                cur = cur.apply(tx);
            }
        }
        let mut next = None;
        let handled = match km_for_keydown.borrow().as_ref() {
            Some(km) => {
                let mut d = |t: Transaction| next = Some(cur.apply(t));
                km.handle(&cur, &key, Some(&mut d))
            }
            None => false,
        };
        if let Some(n) = next {
            let mut rt_w = runtime;
            if let Some(rt) = rt_w.write().as_mut() {
                rt.view.update(n.doc().clone());
                rt.applying_selection.set(true);
                let _ = rt.view.set_selection(n.selection());
                rt.applying_selection.set(false);
            }
            let mut s = state;
            s.set(n);
        }
        let structural = matches!(key.key.as_str(), "Enter" | "Backspace" | "Delete");
        if handled || structural {
            kev.prevent_default();
        }
    });
    push_listener(&mut closers, target.clone(), "keydown", cb);

    // IME composition: suspend reads while composing, commit on end.
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        with_view(runtime, |v| v.composition_start());
    });
    push_listener(&mut closers, target.clone(), "compositionstart", cb);

    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        let t = with_view(runtime, |v| {
            v.composition_end();
            v.read_dom_changes()
        })
        .flatten();
        if let Some(t) = t {
            apply_transform(state, &t);
        }
    });
    push_listener(&mut closers, target.clone(), "compositionend", cb);

    // Paste: prefer Markdown, then HTML, then plain text — all sanitised through
    // the schema-aware paths in core.
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Ok(clip) = ev.dyn_into::<web_sys::ClipboardEvent>() else {
            return;
        };
        clip.prevent_default();
        let Some(data) = clip.clipboard_data() else {
            return;
        };
        let md = data.get_data("text/markdown").unwrap_or_default();
        let html = data.get_data("text/html").unwrap_or_default();
        let text = data.get_data("text/plain").unwrap_or_default();
        let t = with_view(runtime, |v| {
            if !md.is_empty() {
                v.paste_markdown(&md)
            } else if !html.is_empty() {
                v.paste_html(&html)
            } else if !text.is_empty() {
                v.paste_text(&text)
            } else {
                None
            }
        })
        .flatten();
        if let Some(t) = t {
            apply_transform(state, &t);
        }
    });
    push_listener(&mut closers, target.clone(), "paste", cb);

    // `selectionchange` only fires on `document`; mirror the browser selection
    // into state so keymap commands see the right anchor/head. Drop the echo from
    // our own effect-driven set_selection.
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        let doc_target: web_sys::EventTarget = doc.into();
        let applying = applying_selection;
        let from_dom = selection_from_dom;
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
            if applying.get() {
                return;
            }
            let Some(Some(sel)) = with_view(runtime, |v| v.read_selection()) else {
                return;
            };
            let cur = state.peek().selection();
            if sel == cur {
                return;
            }
            from_dom.set(true);
            let mut s = state;
            let next = {
                let snap = s.peek();
                let mut tx = snap.tr();
                tx.set_selection(sel);
                tx.no_history();
                snap.apply(tx)
            };
            s.set(next);
        });
        push_listener(&mut closers, doc_target, "selectionchange", cb);
    }

    closers
}

/// Run `f` against the mounted `EditorView`, if any.
fn with_view<R>(
    runtime: Signal<Option<EditorRuntime>>,
    f: impl FnOnce(&EditorView) -> R,
) -> Option<R> {
    runtime.peek().as_ref().map(|rt| f(&rt.view))
}

/// Fold a DOM-bridge transform into the state signal.
fn apply_transform(mut state: Signal<EditorState>, tr: &Transform) {
    let next = {
        let snap = state.peek();
        let mut tx = snap.tr();
        let mut ok = true;
        for step in tr.steps() {
            if tx.transform().step(step.clone(), snap.schema()).is_err() {
                ok = false;
                break;
            }
        }
        if !ok {
            return;
        }
        snap.apply(tx)
    };
    state.set(next);
}
