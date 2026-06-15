//! Minimal, XSS-safe Markdown for article bodies. Each stored body "block" (one
//! line) renders to one HTML element. The text is HTML-ESCAPED first and only our
//! own tags are added afterwards, so author input can never inject markup; link
//! URLs are validated to http(s) or site-relative. Supports: `#`/`##` headings,
//! `- ` bullets, `**bold**`, `*italic*`, `[text](url)`, a standalone image
//! block `![caption](url)`, and a leading `^ ` for an OPT-IN drop cap (the large
//! first letter) — only the paragraph(s) an author marks get it, never every one.
//! Used by every body renderer (public article, staff preview, editor live
//! preview) so what an editor types is what readers see.

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn valid_url(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://") || u.starts_with('/')
}

/// Render `[text](url)` links over already-escaped text. Invalid links are left
/// as literal text (the `[` is emitted and scanning continues).
fn render_links(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        if let Some(close) = after.find("](") {
            let text = &after[..close];
            let after2 = &after[close + 2..];
            if let Some(end) = after2.find(')') {
                let url = &after2[..end];
                if valid_url(url) && !text.contains('[') {
                    out.push_str(&format!(
                        "<a href=\"{url}\" rel=\"noopener\" target=\"_blank\">{text}</a>"
                    ));
                    rest = &after2[end + 1..];
                    continue;
                }
            }
        }
        out.push('[');
        rest = &rest[open + 1..];
    }
    out.push_str(rest);
    out
}

/// Toggle a paired delimiter (e.g. `**`) into open/close tags. Only applied when
/// the delimiters are balanced (even count), so a stray marker stays literal.
fn toggle(s: &str, delim: &str, open: &str, close: &str) -> String {
    let parts: Vec<&str> = s.split(delim).collect();
    if parts.len() < 3 || (parts.len() - 1) % 2 != 0 {
        return s.to_string();
    }
    let mut out = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            out.push_str(if i % 2 == 1 { open } else { close });
        }
        out.push_str(p);
    }
    out
}

/// Inline formatting on one block: escape, then links, then bold, then italic.
fn inline(s: &str) -> String {
    let out = esc(s);
    let out = render_links(&out);
    let out = toggle(&out, "**", "<strong>", "</strong>");
    toggle(&out, "*", "<em>", "</em>")
}

/// A block that is exactly an image `![alt](url)` → a figure. URL validated; alt
/// + url escaped. Returns None if the block isn't a standalone image.
fn image_html(b: &str) -> Option<String> {
    let rest = b.strip_prefix("![")?;
    let close = rest.find("](")?;
    let alt = &rest[..close];
    let url = rest[close + 2..].strip_suffix(')')?;
    if valid_url(url) && !alt.contains('[') {
        Some(format!(
            "<figure class=\"md-figure\"><img src=\"{}\" alt=\"{}\" loading=\"lazy\"/></figure>",
            esc(url),
            esc(alt)
        ))
    } else {
        None
    }
}

/// Render one body block to safe HTML.
pub fn block_html(block: &str) -> String {
    let b = block.trim();
    if let Some(img) = image_html(b) {
        return img;
    }
    if let Some(h) = b.strip_prefix("## ") {
        format!("<h2>{}</h2>", inline(h))
    } else if let Some(h) = b.strip_prefix("# ") {
        format!("<h2>{}</h2>", inline(h))
    } else if let Some(li) = b.strip_prefix("- ") {
        format!("<ul><li>{}</li></ul>", inline(li))
    } else if let Some(lead) = b.strip_prefix("^ ") {
        // Opt-in drop cap: a leading "^ " gives THIS paragraph the large red
        // initial (`.prose p.dropcap::first-letter`). Nothing is auto-capped, so
        // the style appears only where the author asks for it — never on every
        // paragraph. The marker itself is stripped before inline formatting.
        format!("<p class=\"dropcap\">{}</p>", inline(lead))
    } else {
        format!("<p>{}</p>", inline(b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_then_formats() {
        // raw HTML is neutralised
        assert_eq!(block_html("<script>x"), "<p>&lt;script&gt;x</p>");
        // bold + italic + heading
        assert_eq!(block_html("a **b** c"), "<p>a <strong>b</strong> c</p>");
        assert_eq!(block_html("## Heading"), "<h2>Heading</h2>");
        // a valid link renders; a javascript: url does not
        assert!(block_html("[ok](https://x.com)").contains("<a href=\"https://x.com\""));
        assert!(!block_html("[no](javascript:alert(1))").contains("<a "));
        // unbalanced marker stays literal
        assert_eq!(block_html("a * b"), "<p>a * b</p>");
        // opt-in drop cap: only a leading "^ " classes the paragraph; the marker
        // is stripped, and a plain paragraph is never auto-capped.
        assert_eq!(
            block_html("^ Once upon"),
            "<p class=\"dropcap\">Once upon</p>"
        );
        assert_eq!(block_html("Once upon"), "<p>Once upon</p>");
        // a standalone image renders; a javascript: image src does not
        assert!(
            block_html("![cat](https://x.com/c.jpg)").contains("<img src=\"https://x.com/c.jpg\"")
        );
        assert!(!block_html("![x](javascript:alert(1))").contains("<img"));
    }
}
