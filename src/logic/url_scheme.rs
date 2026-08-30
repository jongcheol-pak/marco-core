//! URL scheme classification, shared by the HTML renderer and the diagnostics
//! layer.
//!
//! # Normalise first, then validate
//!
//! Every published bypass of a Markdown URL filter has been a normalisation
//! failure rather than a policy failure: the filter asked the right question
//! about the wrong string. goldmark fell to `javascript&colon;alert()` because
//! it read the scheme before HTML entities were resolved; MathJax 3.x fell to
//! `javasc\nript:` because its `[a-z]+` protocol pattern stopped at the control
//! character and the truncated scheme matched nothing on its denylist.
//!
//! marco-core's own parser preserves link destinations verbatim — it neither
//! lowercases nor decodes — so all of those encodings reach the AST intact and
//! are decoded later by the browser. [`scheme_of`] therefore decodes before it
//! classifies, and both policies read its output rather than the raw URL.
//!
//! Decoding here is **for analysis only**. The URL emitted into the document is
//! always the author's original text; this module decides whether to emit it,
//! never what to emit.

/// Where a URL appears in the document.
///
/// The two contexts are treated differently on purpose: an inert base64 raster
/// image is safe as an `<img src>` but a `data:` URL a reader can *click* is a
/// different question, so the exemption in [`is_dangerous`] applies to
/// [`UrlContext::Image`] only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UrlContext {
    /// A link destination — something the reader can activate.
    Link,
    /// An image source — something the browser fetches and renders inline.
    Image,
}

/// Schemes that carry active content in the browsers and WebViews this crate's
/// HTML is rendered in.
///
/// Matches the set used by cmark, comrak, goldmark and markdown-it, which
/// arrived at it independently.
pub const DANGEROUS_SCHEMES: &[&str] = &["javascript", "vbscript", "file", "data"];

/// Image mediatypes whose `;base64,` data URIs cannot execute.
///
/// `image/svg+xml` is deliberately absent: it is an image mediatype but a
/// script-capable one in WebKit6 and WebView2.
pub const INERT_IMAGE_DATA_MEDIATYPES: &[&str] =
    &["image/png", "image/jpeg", "image/gif", "image/webp"];

/// Schemes emitted under the strictest policy.
///
/// This list is a public contract — removing an entry breaks documents that
/// render today — so it starts short and grows on request. Relative URLs of
/// every form are permitted separately and are not listed here.
pub const SAFE_SCHEMES: &[&str] = &[
    "http", "https", "mailto", "ftp", "ftps", "irc", "ircs", "news", "nntp", "tel", "sms",
];

/// The scheme of `url`, lowercased and decoded, or `None` when the URL is
/// relative.
///
/// Returns `None` for every relative form — `./x`, `/x`, `x/y`, `#frag`,
/// `?q=1`, `//host/path` — because a `:` appearing after `/`, `?` or `#` is
/// path or query content, not a scheme separator. `./a:b` is a relative path.
///
/// Before the scheme is read, the leading run of the URL is decoded the way a
/// browser would see it: ASCII whitespace and control characters are dropped,
/// `%XX` escapes are resolved, and HTML character references are resolved. A
/// scheme longer than [`MAX_SCHEME_LEN`] is treated as absent — no real scheme
/// approaches that length, and the cap bounds the work done on hostile input.
pub fn scheme_of(url: &str) -> Option<String> {
    let mut scheme = String::new();
    let mut chars = url.chars().peekable();

    while let Some(ch) = chars.next() {
        // Browsers drop ASCII whitespace and control characters from URLs
        // before parsing them, so `java\tscript:` is `javascript:`.
        if is_ignorable(ch) {
            continue;
        }

        let decoded = match ch {
            '%' => decode_percent(&mut chars).unwrap_or('%'),
            '&' => decode_entity(&mut chars).unwrap_or('&'),
            other => other,
        };

        match decoded {
            ':' => return (!scheme.is_empty()).then_some(scheme),
            // Past any of these the URL is relative; there is no scheme.
            '/' | '?' | '#' => return None,
            other => {
                if scheme.len() >= MAX_SCHEME_LEN {
                    return None;
                }
                scheme.extend(other.to_lowercase());
            }
        }
    }

    None
}

/// Longest scheme this module will consider. The longest IANA-registered
/// scheme is well under this; anything longer is hostile or malformed.
pub const MAX_SCHEME_LEN: usize = 32;

/// Whether `url` uses a scheme known to carry active content — the denylist
/// policy.
///
/// Unknown schemes **pass**. That is the defining property of a denylist and
/// the reason [`is_safe`] exists alongside it.
///
/// In [`UrlContext::Image`], a `data:` URI carrying an inert base64 raster
/// image is exempt: it cannot execute, so reporting it would be a false
/// positive. The exemption does not apply to links.
pub fn is_dangerous(url: &str, context: UrlContext) -> bool {
    let Some(scheme) = scheme_of(url) else {
        return false; // relative URLs carry no scheme and cannot be dangerous
    };

    if !DANGEROUS_SCHEMES.contains(&scheme.as_str()) {
        return false;
    }

    if context == UrlContext::Image && is_inert_base64_image(url) {
        return false;
    }

    true
}

/// Whether `url` is safe to emit under the strictest policy — the allowlist.
///
/// Relative URLs of every form are safe. Absolute URLs are safe only when their
/// scheme is on [`SAFE_SCHEMES`]; unknown schemes are **rejected**, which is
/// what makes this resistant to encodings nobody has thought of yet.
///
/// The inert-image exemption of [`is_dangerous`] deliberately does **not**
/// apply here. A consumer choosing this policy has asked for the strict answer,
/// and `data:` is not on the safe list.
pub fn is_safe(url: &str, _context: UrlContext) -> bool {
    match scheme_of(url) {
        None => true, // relative
        Some(scheme) => SAFE_SCHEMES.contains(&scheme.as_str()),
    }
}

/// The mediatype of a `data:<mediatype>;base64,…` URL, lowercased.
///
/// Returns `None` for anything else — a bare `data:image/png,` payload, an
/// extra parameter before the `;base64`, a URI with no payload separator at
/// all, or a non-`data:` URL. Callers apply their own list of acceptable
/// mediatypes to the result: the renderer uses
/// [`INERT_IMAGE_DATA_MEDIATYPES`], while the diagnostics layer takes its list
/// from the diagnostics catalog, so the two can be configured independently
/// while sharing this parsing.
pub fn base64_data_mediatype(url: &str) -> Option<String> {
    let cleaned: String = url.chars().filter(|c| !is_ignorable(*c)).collect();
    let lowered = cleaned.to_lowercase();

    let rest = lowered.strip_prefix("data:")?;
    let (metadata, _payload) = rest.split_once(',')?;
    let mediatype = metadata.strip_suffix(";base64")?;

    Some(mediatype.to_string())
}

/// Whether an image source is a base64 data URI carrying an inert image.
///
/// Only the mediatypes in [`INERT_IMAGE_DATA_MEDIATYPES`] qualify, and only in
/// their `;base64,` form.
fn is_inert_base64_image(url: &str) -> bool {
    base64_data_mediatype(url)
        .is_some_and(|mediatype| INERT_IMAGE_DATA_MEDIATYPES.contains(&mediatype.as_str()))
}

/// ASCII whitespace and control characters, which browsers strip from URLs.
fn is_ignorable(ch: char) -> bool {
    ch.is_ascii_control() || ch == ' '
}

/// Resolve a `%XX` escape, having already consumed the `%`.
///
/// Only ASCII is decoded. A percent escape for a non-ASCII byte cannot be part
/// of a scheme, so leaving it undecoded is safe and avoids partial-UTF-8
/// reassembly.
fn decode_percent(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<char> {
    let mut value: u32 = 0;
    let mut digits = 0;

    while digits < 2 {
        let digit = chars.peek().and_then(|c| c.to_digit(16))?;
        value = value * 16 + digit;
        chars.next();
        digits += 1;
    }

    (value < 0x80).then(|| char::from(value as u8))
}

/// Resolve an HTML character reference, having already consumed the `&`.
///
/// Handles the numeric forms `&#58;` and `&#x3a;`, plus the named references
/// that can appear inside a scheme. `&colon;` is the one that broke goldmark.
fn decode_entity(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<char> {
    let mut body = String::new();

    // A character reference is short; anything longer is not one.
    for _ in 0..12 {
        match chars.peek() {
            Some(';') => {
                chars.next();
                return resolve_reference(&body);
            }
            Some(c) => {
                body.push(*c);
                chars.next();
            }
            None => return None,
        }
    }

    None
}

fn resolve_reference(body: &str) -> Option<char> {
    let lowered = body.to_ascii_lowercase();

    if let Some(hex) = lowered.strip_prefix("#x") {
        let value = u32::from_str_radix(hex, 16).ok()?;
        return char::from_u32(value);
    }

    if let Some(decimal) = lowered.strip_prefix('#') {
        let value: u32 = decimal.parse().ok()?;
        return char::from_u32(value);
    }

    // Named references that matter inside a scheme or its delimiters.
    match lowered.as_str() {
        "colon" => Some(':'),
        "tab" | "newline" => Some('\n'),
        "sol" => Some('/'),
        "quest" => Some('?'),
        "num" => Some('#'),
        "amp" => Some('&'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_plain_schemes() {
        assert_eq!(scheme_of("https://example.com"), Some("https".to_string()));
        assert_eq!(scheme_of("mailto:a@b.c"), Some("mailto".to_string()));
        assert_eq!(
            scheme_of("javascript:alert(1)"),
            Some("javascript".to_string())
        );
    }

    #[test]
    fn smoke_test_relative_urls_have_no_scheme() {
        for url in [
            "./a:b",
            "/absolute/path",
            "relative/path",
            "#fragment",
            "?query=1",
            "//host/path",
            "a/b:c",
        ] {
            assert_eq!(scheme_of(url), None, "expected no scheme for {url}");
        }
    }

    #[test]
    fn case_is_normalised() {
        assert_eq!(
            scheme_of("JaVaScRiPt:alert(1)"),
            Some("javascript".to_string())
        );
        assert!(is_dangerous("JAVASCRIPT:alert(1)", UrlContext::Link));
    }

    /// The three bypasses found in the shipped diagnostic (TODO T1.2). Each of
    /// these is decoded by the browser inside an `href`.
    #[test]
    fn known_bypasses_are_closed() {
        for url in [
            "javascript&colon;alert(1)",
            "javascript&#58;alert(1)",
            "javascript&#x3a;alert(1)",
            "%6aavascript:alert(1)",
            "java\tscript:alert(1)",
            "java\nscript:alert(1)",
            "java\0script:alert(1)",
            " javascript:alert(1)",
            "\njavascript:alert(1)",
            "JaVaScRiPt&colon;alert(1)",
        ] {
            assert_eq!(
                scheme_of(url),
                Some("javascript".to_string()),
                "failed to normalise {url:?}"
            );
            assert!(is_dangerous(url, UrlContext::Link), "not caught: {url:?}");
            assert!(!is_safe(url, UrlContext::Link), "wrongly allowed: {url:?}");
        }
    }

    #[test]
    fn safe_schemes_and_relative_urls_pass_the_allowlist() {
        for url in [
            "https://example.com",
            "http://example.com",
            "mailto:a@b.c",
            "tel:+123",
            "./relative",
            "/absolute",
            "#frag",
            "//host/path",
        ] {
            assert!(is_safe(url, UrlContext::Link), "wrongly blocked: {url}");
        }
    }

    #[test]
    fn unknown_schemes_split_the_two_policies() {
        // The defining difference: a denylist passes what it does not know.
        assert!(!is_dangerous("vscode://file/x", UrlContext::Link));
        assert!(!is_safe("vscode://file/x", UrlContext::Link));
    }

    #[test]
    fn inert_base64_images_are_exempt_for_images_only() {
        let png = "data:image/png;base64,iVBORw0KGgo=";
        assert!(!is_dangerous(png, UrlContext::Image));
        assert!(is_dangerous(png, UrlContext::Link));
        // Strict policy does not exempt data: at all.
        assert!(!is_safe(png, UrlContext::Image));
    }

    #[test]
    fn svg_and_non_base64_data_uris_are_not_exempt() {
        for url in [
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
            "data:image/svg+xml,<svg></svg>",
            "data:image/png,rawbytes",
            "data:text/html;base64,PGgxPmhpPC9oMT4=",
            "data:image/png;charset=utf-8;base64,x",
            "data:image/png",
        ] {
            assert!(is_dangerous(url, UrlContext::Image), "exempted: {url}");
        }
    }

    #[test]
    fn all_dangerous_schemes_are_caught() {
        for scheme in DANGEROUS_SCHEMES {
            let url = format!("{scheme}:payload");
            assert!(is_dangerous(&url, UrlContext::Link), "missed {scheme}");
        }
    }

    #[test]
    fn overlong_schemes_are_rejected_rather_than_scanned() {
        let url = format!("{}:x", "a".repeat(MAX_SCHEME_LEN + 1));
        assert_eq!(scheme_of(&url), None);
    }

    #[test]
    fn malformed_encodings_do_not_panic_and_do_not_falsely_match() {
        for url in [
            "%",
            "%zz",
            "%6",
            "&",
            "&;",
            "&nosuchentity;",
            "&#;",
            "&#xzz;",
            "&unterminated",
            ":",
            "",
        ] {
            let _ = scheme_of(url);
            let _ = is_dangerous(url, UrlContext::Link);
            let _ = is_safe(url, UrlContext::Link);
        }
    }

    #[test]
    fn a_bare_colon_is_not_a_scheme() {
        assert_eq!(scheme_of(":alert(1)"), None);
    }
}
