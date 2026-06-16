//! Markdown ↔ editor bridge. The newsroom keeps **markdown** as its canonical
//! store (so `md.rs`, the public render, XSS-safety and the `^ ` drop-cap are
//! unchanged); the WYSIWYG editor loads from markdown and saves back to it.
//!
//! All of this is `taino-edit-core` + `taino-edit-extensions` only (no web-sys),
//! so it compiles on the server/SSG build as well as the browser.

use taino_edit_core::markdown::{parse_markdown, to_markdown};
use taino_edit_core::{EditorState, Keymap, Node, NodeSpec, Schema, SchemaBuilder};
use taino_edit_extensions::{
    build_keymap_with, build_schema_with, Blockquote, Bold, Code, CodeBlock, Extension, Heading,
    History, Image, Italic, Link, Lists, Paragraph,
};

/// The editing feature set for newsroom articles: text formatting, headings,
/// links/images, quotes, code, lists and undo/redo. (No tables/alignment — not
/// part of the article style, and they would pull the table view plugin.)
fn extensions() -> [&'static dyn Extension; 11] {
    [
        &Paragraph,
        &Heading,
        &Bold,
        &Italic,
        &Code,
        &Link,
        &Image,
        &Blockquote,
        &CodeBlock,
        &Lists,
        &History,
    ]
}

/// Build the article schema — shared by the editor, the keymap, and the markdown
/// parser so they all agree on the node/mark set.
pub fn newsroom_schema() -> Schema {
    let base = SchemaBuilder::new()
        .node(
            "doc",
            NodeSpec {
                content: Some("block+".into()),
                ..Default::default()
            },
        )
        .node(
            "text",
            NodeSpec {
                group: Some("inline".into()),
                ..Default::default()
            },
        );
    build_schema_with(base, &extensions(), "doc").expect("newsroom schema builds")
}

/// Parse markdown into a document for `schema`. Falls back to an empty paragraph
/// if the text doesn't parse, so the editor always mounts with a valid doc.
pub fn markdown_to_doc(schema: &Schema, md: &str) -> Node {
    parse_markdown(schema, md).unwrap_or_else(|_| empty_doc(schema))
}

fn empty_doc(schema: &Schema) -> Node {
    let p = schema
        .node("paragraph", Default::default(), vec![], vec![])
        .expect("paragraph node");
    schema
        .node("doc", Default::default(), vec![p], vec![])
        .expect("doc node")
}

/// Serialise the editor's current document back to markdown — the canonical
/// store the article is saved and publicly rendered from.
pub fn state_to_markdown(state: &EditorState) -> String {
    to_markdown(state.doc())
}

/// The keyboard map (Mod-b bold, Mod-i italic, Mod-z undo, …) for `schema`.
pub fn newsroom_keymap(schema: &Schema) -> Keymap {
    build_keymap_with(&extensions(), schema, false)
}
