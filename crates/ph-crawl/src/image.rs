//! Image handling for self-hosting source photos.
//!
//! Two concerns, kept apart:
//! * [`og_image`] — pull the article's primary image URL from page HTML at CRAWL
//!   time (pure; tested without network).
//! * [`fetch_image_local`] — at PROMOTE time, download that image and store it
//!   locally so a published report self-hosts it instead of hot-linking the
//!   source. This URL is attacker-influenceable (a source page chooses its own
//!   `og:image`), and we run on EC2 with an instance role, so the download is
//!   **SSRF-guarded**: the host must resolve to public IPs only, redirects are
//!   refused, the size is capped, and the format is taken from the magic bytes
//!   (jpg/png/webp only — never SVG, which can carry script).

use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

/// Largest image we will download (bytes). Real article photos are well under this.
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// Extract the article's primary image URL from page HTML: `og:image` (or
/// `og:image:url`), then `twitter:image`. Relative URLs are resolved against
/// `base`; only `http`/`https` results are returned. `None` when absent.
pub fn og_image(html: &str, base: &url::Url) -> Option<String> {
    use scraper::{Html, Selector};
    let doc = Html::parse_document(html);
    const CANDIDATES: &[&str] = &[
        r#"meta[property="og:image"]"#,
        r#"meta[property="og:image:url"]"#,
        r#"meta[name="twitter:image"]"#,
        r#"meta[name="twitter:image:src"]"#,
    ];
    for sel in CANDIDATES {
        let Ok(selector) = Selector::parse(sel) else {
            continue;
        };
        for el in doc.select(&selector) {
            let Some(raw) = el.value().attr("content") else {
                continue;
            };
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            if let Ok(abs) = base.join(raw) {
                if matches!(abs.scheme(), "http" | "https") {
                    return Some(abs.to_string());
                }
            }
        }
    }
    None
}

/// Is this a public, fetchable IP? Rejects the addresses an SSRF would target:
/// loopback, private (RFC1918), CGNAT (100.64/10), link-local (incl. 169.254 —
/// the cloud metadata endpoint), unspecified, broadcast/documentation, multicast,
/// IPv6 loopback/unspecified/multicast, unique-local (fc00::/7), IPv6 link-local
/// (fe80::/10), and any IPv4-mapped IPv6 (which could smuggle a private v4).
pub fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            let cgnat = o[0] == 100 && (o[1] & 0xc0) == 64; // 100.64.0.0/10
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || cgnat)
        }
        IpAddr::V6(v6) => {
            let seg0 = v6.segments()[0];
            let ula = (seg0 & 0xfe00) == 0xfc00; // fc00::/7
            let link_local = (seg0 & 0xffc0) == 0xfe80; // fe80::/10
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || ula
                || link_local
                || v6.to_ipv4_mapped().is_some())
        }
    }
}

/// Match the image format by magic bytes (never by URL/extension). Returns the
/// canonical file extension for the supported raster formats, else `None`.
fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        Some("jpg")
    } else if bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'] {
        Some("png")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Download `src_url` and store it under `dir` as `<sha256>.<ext>` (content
/// addressed — re-downloads dedupe to the same file). Returns the stored file
/// name (e.g. `"ab12….jpg"`) on success, `None` on ANY failure — this is
/// best-effort and never fatal to a promote.
///
/// Hardened against SSRF: `http`/`https` only; the host must resolve to public
/// IPs and the connection is pinned to a validated address (no DNS rebinding);
/// redirects are refused; the body is size-capped; and the stored format is
/// decided by magic bytes (jpg/png/webp), not the URL.
pub async fn fetch_image_local(src_url: &str, dir: &Path, user_agent: &str) -> Option<String> {
    let url = url::Url::parse(src_url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.to_string();
    let port = url.port_or_known_default()?;

    // Resolve + SSRF guard: every resolved address must be public, and we pin the
    // request to a validated address so a rebind can't swap in an internal one.
    let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host.as_str(), port))
        .await
        .ok()?
        .collect();
    if addrs.is_empty() || !addrs.iter().all(|a| is_public_ip(&a.ip())) {
        return None;
    }
    let pinned = *addrs.first()?;

    let client = reqwest::Client::builder()
        .user_agent(user_agent)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(8))
        .resolve(&host, pinned)
        .build()
        .ok()?;
    let mut resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    // Must declare an image content-type.
    let ct_ok = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.trim_start().starts_with("image/"))
        .unwrap_or(false);
    if !ct_ok {
        return None;
    }
    // Reject early if the server declares an oversize body, then STREAM with a hard
    // cap: a missing/lying Content-Length (e.g. chunked-transfer CDNs) can't make us
    // read an unbounded body, and chunked images still self-host instead of being
    // silently skipped.
    if resp.content_length().is_some_and(|n| n > MAX_IMAGE_BYTES) {
        return None;
    }
    let mut bytes: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await.ok()? {
        if bytes.len() + chunk.len() > MAX_IMAGE_BYTES as usize {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return None;
    }
    let ext = image_ext(&bytes)?; // jpg/png/webp only — by magic bytes

    use sha2::{Digest, Sha256};
    let name = format!("{}.{ext}", hex(&Sha256::digest(&bytes)));
    tokio::fs::create_dir_all(dir).await.ok()?;
    let path = dir.join(&name);
    if tokio::fs::metadata(&path).await.is_err() {
        tokio::fs::write(&path, &bytes).await.ok()?;
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn og_image_absolute_relative_and_fallback() {
        let base = url::Url::parse("https://news.example/article/1").unwrap();
        // og:image absolute
        let h = r#"<html><head><meta property="og:image" content="https://cdn.example/p.jpg"></head></html>"#;
        assert_eq!(og_image(h, &base).as_deref(), Some("https://cdn.example/p.jpg"));
        // relative og:image resolved against the page
        let h = r#"<meta property="og:image" content="/img/p.png">"#;
        assert_eq!(og_image(h, &base).as_deref(), Some("https://news.example/img/p.png"));
        // twitter:image fallback when no og:image
        let h = r#"<meta name="twitter:image" content="https://cdn.example/t.webp">"#;
        assert_eq!(og_image(h, &base).as_deref(), Some("https://cdn.example/t.webp"));
        // none present
        assert_eq!(og_image("<html><head></head></html>", &base), None);
        // javascript: / data: schemes are rejected
        let h = r#"<meta property="og:image" content="javascript:alert(1)">"#;
        assert_eq!(og_image(h, &base), None);
    }

    #[test]
    fn ssrf_guard_rejects_internal_ips() {
        let public = ["8.8.8.8", "1.1.1.1", "93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946"];
        for s in public {
            assert!(is_public_ip(&s.parse().unwrap()), "{s} should be public");
        }
        let internal = [
            "127.0.0.1",       // loopback
            "10.0.0.5",        // private
            "172.16.0.1",      // private
            "192.168.1.1",     // private
            "169.254.169.254", // link-local / cloud metadata
            "100.64.0.1",      // CGNAT
            "0.0.0.0",         // unspecified
            "::1",             // v6 loopback
            "fc00::1",         // v6 unique-local
            "fe80::1",         // v6 link-local
            "::ffff:10.0.0.1", // v4-mapped private
        ];
        for s in internal {
            assert!(!is_public_ip(&s.parse().unwrap()), "{s} should be rejected");
        }
    }

    #[test]
    fn image_ext_by_magic_only() {
        assert_eq!(image_ext(&[0xFF, 0xD8, 0xFF, 0x00]), Some("jpg"));
        assert_eq!(image_ext(b"\x89PNG\r\n\x1a\n....."), Some("png"));
        assert_eq!(image_ext(b"RIFF\0\0\0\0WEBPxx"), Some("webp"));
        // SVG / gif / html are not accepted
        assert_eq!(image_ext(b"<svg xmlns=..."), None);
        assert_eq!(image_ext(b"GIF89a"), None);
        assert_eq!(image_ext(b"<!doctype html>"), None);
    }
}
