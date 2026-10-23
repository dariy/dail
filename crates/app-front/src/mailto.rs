//! `mailto:` links (RFC 6068) as the fields of a new draft: `mailto:a@b,c@d?cc=…&subject=…&body=…`.
//! A link only ever fills a draft; it never sends.

use app_core::Addr;

use crate::compose::parse_addr;

/// The fields a `mailto:` link fills.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mailto {
    /// `To` addresses (the path and `to=`).
    pub to: Vec<Addr>,
    /// `Cc` addresses.
    pub cc: Vec<Addr>,
    /// `Bcc` addresses.
    pub bcc: Vec<Addr>,
    /// The subject.
    pub subject: String,
    /// The body.
    pub body: String,
}

/// Parses a `mailto:` URI; `None` if it is not one. Unknown header fields are ignored, and so
/// are the parts of an address list that are not addresses.
pub fn parse(uri: &str) -> Option<Mailto> {
    let rest = uri
        .get(..7)
        .filter(|s| s.eq_ignore_ascii_case("mailto:"))
        .and_then(|_| uri.get(7..))?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let mut out = Mailto::default();
    add(&mut out.to, &decode(path));
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = decode(value);
        match key.to_ascii_lowercase().as_str() {
            "to" => add(&mut out.to, &value),
            "cc" => add(&mut out.cc, &value),
            "bcc" => add(&mut out.bcc, &value),
            "subject" => out.subject = value,
            "body" => out.body = value.replace("\r\n", "\n"),
            _ => {}
        }
    }
    Some(out)
}

fn add(list: &mut Vec<Addr>, text: &str) {
    for addr in text.split([',', ';']).filter_map(parse_addr) {
        if !list
            .iter()
            .any(|a| a.email.eq_ignore_ascii_case(&addr.email))
        {
            list.push(addr);
        }
    }
}

/// Percent-decoding as UTF-8 (invalid sequences become U+FFFD); `+` stays a plus (RFC 6068).
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (b, hex) {
            (b'%', Some(value)) => {
                out.push(value);
                i += 3;
            }
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emails(list: &[Addr]) -> Vec<&str> {
        list.iter().map(|a| a.email.as_str()).collect()
    }

    #[test]
    fn fields_come_from_the_path_and_the_query() {
        let m = parse(
            "MAILTO:bob@example.org,%20Carol%20%3Ccarol@example.org%3E?cc=dan@example.org\
             &subject=Lunch%3F%20%E2%9C%93&body=Hi%0D%0Athere+you&x-junk=1&to=bob@example.org",
        )
        .unwrap();
        assert_eq!(emails(&m.to), ["bob@example.org", "carol@example.org"]);
        assert_eq!(m.to[1].name.as_deref(), Some("Carol"));
        assert_eq!(emails(&m.cc), ["dan@example.org"]);
        assert_eq!(m.subject, "Lunch? ✓");
        assert_eq!(m.body, "Hi\nthere+you");
    }

    #[test]
    fn other_schemes_and_bad_parts_are_refused() {
        assert_eq!(parse("https://example.org"), None);
        assert_eq!(parse("mail"), None);
        let m = parse("mailto:not-an-address?bcc=%ZZ,eve@example.org").unwrap();
        assert!(m.to.is_empty());
        assert_eq!(emails(&m.bcc), ["eve@example.org"]);
        assert_eq!(parse("mailto:").unwrap(), Mailto::default());
    }
}
