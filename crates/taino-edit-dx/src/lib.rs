//! `taino-edit-dx` — a Dioxus **0.8** adapter for taino-edit, the native-Rust
//! WYSIWYG editor. Ported from upstream `taino-edit-dioxus` 0.5.3 (Dioxus 0.6)
//! and target-gated for a fullstack/SSG app: the real editor (web-sys + the
//! `taino-edit-dom` bridge) compiles only for `wasm32`; the server/SSG build
//! renders an empty `div.taino-editor` host, which the wasm client hydrates and
//! mounts the editor into.
//!
//! The host app keeps markdown as its canonical store and bridges it to the
//! editor with `taino_edit_core` (`to_markdown` / parse), so the editor never
//! changes how articles are stored or rendered publicly.

#![deny(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

// Only the native stub below builds a component (needs Signal/Element/rsx!); the
// wasm editor lives in `web`. Gate the prelude so wasm doesn't see it unused.
#[cfg(not(target_arch = "wasm32"))]
use dioxus::prelude::*;

// Re-export the engine surface the host reaches for, so consumers depend only on
// this adapter. These are framework-agnostic (compile on every target).
#[doc(no_inline)]
pub use taino_edit_core::{EditorState, KeyPress, Keymap, Node, Schema, Transaction, Transform};

/// A take-once container for the optional `keymap` prop. `Keymap` is not
/// `Clone + PartialEq` (Dioxus props must be), so it lives behind a shared cell
/// and is moved into the view once, at mount. The prop always compares equal so
/// it never forces a re-render.
#[derive(Clone, Default)]
pub struct KeymapProp(Rc<RefCell<Option<Keymap>>>);

impl KeymapProp {
    /// Wrap a keymap for the `keymap` prop.
    pub fn new(keymap: Keymap) -> Self {
        Self(Rc::new(RefCell::new(Some(keymap))))
    }

    /// Take the keymap out (once, at mount).
    #[allow(dead_code)] // only read on wasm32 (the native stub ignores it)
    fn take(&self) -> Option<Keymap> {
        self.0.borrow_mut().take()
    }
}

impl PartialEq for KeymapProp {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for KeymapProp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeymapProp").finish_non_exhaustive()
    }
}

// ---- server / SSG (native) build: empty host, hydrated on the client ----
#[cfg(not(target_arch = "wasm32"))]
#[component]
pub fn TainoEditor(state: Signal<EditorState>, #[props(default)] keymap: KeymapProp) -> Element {
    // web-sys is unavailable off-wasm; render the host the client mounts into.
    let _ = (state, keymap);
    rsx! {
        div { class: "taino-editor" }
    }
}

// ---- browser (wasm32) build: the real editor ----
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::TainoEditor;
