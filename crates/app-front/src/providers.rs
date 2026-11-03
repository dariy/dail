//! Provider facts the setup needs before it asks for a password, as data (AGENTS.md §4: provider quirks are data).
//!
//! Some providers allow no password login at all for IMAP: they need OAuth2, which dail does not support yet (it
//! follows the launch, ROADMAP Wave 11). Their addresses stop at the address step with that said, instead of failing
//! after the password.

/// Address domains whose mail needs OAuth2, with the provider's name: Microsoft's consumer domains. A company domain
/// on Microsoft 365 cannot be told from its address; it fails at the test with the server's answer.
pub const OAUTH_ONLY: &[(&str, &str)] = &[
    ("outlook.com", "Microsoft"),
    ("hotmail.com", "Microsoft"),
    ("live.com", "Microsoft"),
    ("msn.com", "Microsoft"),
    ("passport.com", "Microsoft"),
    ("outlook.fr", "Microsoft"),
    ("hotmail.fr", "Microsoft"),
    ("live.fr", "Microsoft"),
    ("outlook.de", "Microsoft"),
    ("hotmail.de", "Microsoft"),
    ("live.de", "Microsoft"),
    ("outlook.es", "Microsoft"),
    ("hotmail.es", "Microsoft"),
    ("outlook.it", "Microsoft"),
    ("hotmail.it", "Microsoft"),
    ("live.it", "Microsoft"),
    ("hotmail.co.uk", "Microsoft"),
    ("live.co.uk", "Microsoft"),
    ("outlook.co.uk", "Microsoft"),
    ("hotmail.be", "Microsoft"),
    ("live.be", "Microsoft"),
    ("hotmail.ca", "Microsoft"),
    ("live.ca", "Microsoft"),
    ("outlook.com.au", "Microsoft"),
    ("hotmail.com.au", "Microsoft"),
    ("live.com.au", "Microsoft"),
    ("live.nl", "Microsoft"),
    ("hotmail.nl", "Microsoft"),
    ("outlook.jp", "Microsoft"),
    ("hotmail.co.jp", "Microsoft"),
    ("live.jp", "Microsoft"),
];

/// Where Google lets a user create an App Password (2-Step Verification must be on).
pub const GMAIL_APP_PASSWORDS: &str = "https://myaccount.google.com/apppasswords";

/// The provider whose mail at `email` needs OAuth2, if any. The domain is compared without case.
pub fn oauth_only(email: &str) -> Option<&'static str> {
    let domain = email.trim().rsplit_once('@')?.1.trim_end_matches('.');
    OAUTH_ONLY
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(domain))
        .map(|(_, provider)| *provider)
}
