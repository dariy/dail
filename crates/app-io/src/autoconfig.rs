//! Mozilla's autoconfig format (`config-v1.1.xml`, ADR 0006): the IMAP and SMTP servers of a
//! domain. Only the parts dail uses are read, with a small scanner rather than an XML parser:
//! the first `incomingServer type="imap"` and `outgoingServer type="smtp"` whose `socketType`
//! is `SSL` or `STARTTLS` (plaintext servers are skipped), with `%EMAILDOMAIN%` and friends
//! filled in. The document comes from the network: anything unexpected yields `None`.

use app_core::{Endpoint, Security};

/// The servers an autoconfig document names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Servers {
    /// Incoming mail (IMAP).
    pub imap: Endpoint,
    /// Outgoing mail (SMTP submission).
    pub smtp: Endpoint,
}

/// The servers in `xml` for `email`, if it names an IMAP and an SMTP server dail can use.
pub fn parse(xml: &str, email: &str) -> Option<Servers> {
    let imap = server(xml, "incomingServer", "imap", email)?;
    let smtp = server(xml, "outgoingServer", "smtp", email)?;
    Some(Servers { imap, smtp })
}

/// The first usable `<tag type="kind">` block's endpoint.
fn server(xml: &str, tag: &str, kind: &str, email: &str) -> Option<Endpoint> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut rest = xml;
    while let Some(at) = rest.find(&open) {
        let after = &rest[at + open.len()..];
        let header_end = after.find('>')?;
        let header = &after[..header_end];
        let body_end = after.find(&close)?;
        let body = &after[header_end + 1..body_end];
        rest = &after[body_end + close.len()..];
        if attribute(header, "type").as_deref() != Some(kind) {
            continue;
        }
        let host = fill(element(body, "hostname")?.trim(), email);
        let port: u16 = element(body, "port")?.trim().parse().ok()?;
        let security = match element(body, "socketType")?.trim() {
            "SSL" => Security::Tls,
            "STARTTLS" => Security::StartTls,
            _ => continue,
        };
        let valid_host = !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'));
        if valid_host && port > 0 {
            return Some(Endpoint {
                host: host.to_ascii_lowercase(),
                port,
                security,
            });
        }
    }
    None
}

/// The text of the first `<name>…</name>` in `body`.
fn element<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let start = body.find(&format!("<{name}>"))? + name.len() + 2;
    let end = body[start..].find(&format!("</{name}>"))? + start;
    Some(&body[start..end])
}

/// The value of `name="…"` in a tag's attributes.
fn attribute(header: &str, name: &str) -> Option<String> {
    let start = header.find(&format!("{name}=\""))? + name.len() + 2;
    let end = header[start..].find('"')? + start;
    Some(header[start..end].to_owned())
}

/// Fills in `%EMAILADDRESS%`, `%EMAILLOCALPART%` and `%EMAILDOMAIN%`.
fn fill(text: &str, email: &str) -> String {
    let (local, domain) = email.split_once('@').unwrap_or((email, ""));
    text.replace("%EMAILADDRESS%", email)
        .replace("%EMAILLOCALPART%", local)
        .replace("%EMAILDOMAIN%", domain)
}

/// Where to look for `domain`'s document, in order: the provider's autoconfig host, its
/// well-known path, Mozilla's ISPDB. All HTTPS.
pub fn urls(domain: &str) -> Vec<String> {
    vec![
        format!("https://autoconfig.{domain}/mail/config-v1.1.xml"),
        format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"),
        format!("https://autoconfig.thunderbird.net/v1.1/{domain}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"<?xml version="1.0"?>
<clientConfig version="1.1">
  <emailProvider id="example.org">
    <incomingServer type="pop3">
      <hostname>pop.example.org</hostname><port>995</port><socketType>SSL</socketType>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>imap.example.org</hostname><port>143</port><socketType>plain</socketType>
    </incomingServer>
    <incomingServer type="imap">
      <hostname>mail.%EMAILDOMAIN%</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.Example.org</hostname><port>587</port><socketType>STARTTLS</socketType>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#;

    #[test]
    fn the_first_secure_imap_and_smtp_servers_are_used() {
        let servers = parse(DOC, "me@example.org").unwrap();
        assert_eq!(
            servers.imap,
            Endpoint {
                host: "mail.example.org".into(),
                port: 993,
                security: Security::Tls
            },
            "pop3 and the plaintext imap are skipped"
        );
        assert_eq!(servers.smtp.host, "smtp.example.org");
        assert_eq!(
            (servers.smtp.port, servers.smtp.security),
            (587, Security::StartTls)
        );
    }

    #[test]
    fn odd_documents_yield_nothing() {
        assert!(parse("", "a@b.c").is_none());
        assert!(parse("<html>Not found</html>", "a@b.c").is_none());
        let no_smtp = DOC.replace("outgoingServer", "x");
        assert!(parse(&no_smtp, "me@example.org").is_none());
        let bad_host = DOC.replace("mail.%EMAILDOMAIN%", "evil host;rm");
        assert!(parse(&bad_host, "me@example.org").is_none());
        assert_eq!(urls("example.org").len(), 3);
    }
}
