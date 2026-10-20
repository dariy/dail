//! "Open original": the sanitized HTML page of a message.

use app_io::original::{escape, original_html, sanitize};

const HTML_MAIL: &str = "From: \"Eve <evil>\" <eve@example.org>\r\nTo: me@example.com\r\n\
Subject: Win <b>big</b>\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
<html><head><style>body{background:url(https://track.example/s.png)}</style>\
<script>alert(1)</script></head><body onload=\"steal()\">\
<p style=\"color:red\">Hello <a href=\"https://example.org/offer\" onclick=\"x()\">offer</a> \
<a href=\"javascript:alert(2)\">js</a> <a href=\"file:///etc/passwd\">file</a></p>\
<img src=\"https://track.example/pixel.gif\"><iframe src=\"https://example.org\"></iframe>\
<form action=\"https://phish.example\"><input name=p></form></body></html>\r\n";

#[test]
fn html_is_sanitized_and_locked_down() {
    let page = original_html(HTML_MAIL.as_bytes());
    assert!(page.contains("default-src 'none'"), "CSP present");
    assert!(page.contains("no-referrer"));
    for banned in [
        "<script",
        "alert(1)",
        "onload",
        "onclick",
        "track.example",
        "<img",
        "<iframe",
        "<form",
        "<input",
        "javascript:",
        "file:///",
        "steal()",
    ] {
        assert!(!page.contains(banned), "{banned} survived:\n{page}");
    }
    assert!(page.contains("href=\"https://example.org/offer\""));
    assert!(page.contains("rel=\"noopener noreferrer nofollow\""));
    assert!(page.contains("Hello"));
    // Headers are escaped, never interpreted.
    assert!(page.contains("Win &lt;b&gt;big&lt;/b&gt;"));
    assert!(page.contains("Eve &lt;evil&gt; &lt;eve@example.org&gt;"));
}

#[test]
fn plain_text_is_escaped_into_pre() {
    let raw = "From: bob@example.org\r\nSubject: Hi\r\n\r\n<script>alert(1)</script> & co\r\n";
    let page = original_html(raw.as_bytes());
    assert!(page.contains("<pre>&lt;script&gt;alert(1)&lt;/script&gt; &amp; co</pre>"));
    assert!(!page.contains("<script>"));
}

#[test]
fn garbage_gives_an_empty_page() {
    let page = original_html(b"\xff\xfe\x00garbage");
    assert!(page.starts_with("<!doctype html>"));
    assert_eq!(escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&#39;");
    assert_eq!(sanitize("<b onclick=x>ok</b>"), "<b>ok</b>");
}
