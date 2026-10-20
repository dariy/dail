//! "Open original": a message as a self-contained, sanitized HTML page for the system browser.
//!
//! The HTML part is cleaned with `ammonia` (no scripts, styles, event handlers, forms, frames or
//! media; links only to `http`, `https` and `mailto`, with `rel="noopener noreferrer nofollow"`),
//! images are removed so nothing remote is ever loaded, and a Content-Security-Policy of
//! `default-src 'none'` blocks whatever might slip through. A message without HTML shows its text
//! in a `<pre>`. Header values are escaped. Message content is untrusted throughout.

use std::collections::HashSet;

use mail_parser::MessageParser;

use crate::mime::{TextSource, parse_message};

/// Tags removed with their content (media would load remote resources; the rest is chrome).
const DROPPED: &[&str] = &[
    "img", "picture", "source", "video", "audio", "track", "iframe", "object", "embed", "svg",
    "math", "form", "input", "button", "select", "textarea",
];

/// Escapes text for HTML element content and attribute values.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Cleans an HTML fragment: see the module docs.
pub fn sanitize(html: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder
        .rm_tags(DROPPED.iter().copied())
        .clean_content_tags(HashSet::from(["script", "style", "svg", "math"]))
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .link_rel(Some("noopener noreferrer nofollow"));
    builder.clean(html).to_string()
}

/// The page for a raw RFC 822 message. Never fails: garbage yields an empty page.
pub fn original_html(raw: &[u8]) -> String {
    let parsed = parse_message(raw);
    let html: Vec<String> = MessageParser::default()
        .parse(raw)
        .map(|message| {
            message
                .html_body
                .iter()
                .filter_map(|id| message.parts.get(usize::try_from(*id).ok()?))
                .filter(|part| part.is_text_html())
                .filter_map(|part| part.text_contents().map(sanitize))
                .collect()
        })
        .unwrap_or_default();
    let body = if parsed.text_source == TextSource::Html && !html.is_empty() {
        html.join("\n<hr>\n")
    } else {
        format!("<pre>{}</pre>", escape(&parsed.text))
    };
    let h = &parsed.headers;
    let addrs = |list: &[app_core::Addr]| {
        list.iter()
            .map(|a| match &a.name {
                Some(name) => format!("{name} <{}>", a.email),
                None => a.email.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut header = String::new();
    for (label, value) in [
        ("From", addrs(&h.from)),
        ("To", addrs(&h.to)),
        ("Cc", addrs(&h.cc)),
        ("Subject", h.subject.clone()),
    ] {
        if !value.is_empty() {
            header.push_str(&format!("<div><b>{label}:</b> {}</div>\n", escape(&value)));
        }
    }
    format!(
        "<!doctype html>\n<html><head><meta charset=\"utf-8\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">\n\
         <meta name=\"referrer\" content=\"no-referrer\">\n\
         <title>{title}</title>\n\
         <style>body{{font-family:sans-serif;max-width:760px;margin:2em auto;padding:0 1em;line-height:1.5}}\
         header{{border-bottom:1px solid #ccc;padding-bottom:.8em;margin-bottom:1.2em;color:#444}}\
         pre{{white-space:pre-wrap}}</style>\n\
         </head><body>\n<header>\n{header}<div><small>Opened by dail: scripts, images and remote content \
         are removed.</small></div>\n</header>\n<main>\n{body}\n</main>\n</body></html>\n",
        title = escape(&h.subject),
    )
}
