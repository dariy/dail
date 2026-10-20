//! Display formatting of dates, addresses and sizes. Dates are shown in UTC: the core has no
//! time zone data and the front-ends add no dependency for it yet.

use app_core::Addr;
use app_core::date::civil_from_days;

const DAY_MS: i64 = 86_400_000;
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

fn hm(ms: i64) -> (i64, i64) {
    let minutes = ms.rem_euclid(DAY_MS) / 60_000;
    (minutes / 60, minutes % 60)
}

fn month_name(month: u32) -> &'static str {
    MONTHS
        .get(month.saturating_sub(1) as usize)
        .copied()
        .unwrap_or("?")
}

/// Short list date: `09:41` today, `Nov 14` this year, `2023-11-14` otherwise.
pub fn short_date(ms: i64, now_ms: i64) -> String {
    let (day, today) = (ms.div_euclid(DAY_MS), now_ms.div_euclid(DAY_MS));
    if day == today {
        let (h, m) = hm(ms);
        return format!("{h:02}:{m:02}");
    }
    let (year, month, d) = civil_from_days(day);
    let (this_year, _, _) = civil_from_days(today);
    if year == this_year {
        format!("{} {d}", month_name(month))
    } else {
        format!("{year}-{month:02}-{d:02}")
    }
}

/// Full date: `2023-11-14 09:41 UTC`.
pub fn long_date(ms: i64) -> String {
    let (year, month, d) = civil_from_days(ms.div_euclid(DAY_MS));
    let (h, m) = hm(ms);
    format!("{year}-{month:02}-{d:02} {h:02}:{m:02} UTC")
}

/// Relative age: `just now`, `5m ago`, `3h ago`, `2d ago`.
pub fn age(ms: i64, now_ms: i64) -> String {
    let secs = (now_ms - ms).max(0) / 1000;
    match secs {
        0..60 => "just now".to_owned(),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86_400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

/// A sender's display name: the name, else the address.
pub fn name(addr: &Addr) -> &str {
    addr.name
        .as_deref()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(&addr.email)
}

/// `Name <email>` or `email`.
pub fn full(addr: &Addr) -> String {
    match addr.name.as_deref().filter(|n| !n.trim().is_empty()) {
        Some(name) => format!("{name} <{}>", addr.email),
        None => addr.email.clone(),
    }
}

/// A comma-separated list of names.
pub fn names(addrs: &[&Addr]) -> String {
    addrs.iter().map(|a| name(a)).collect::<Vec<_>>().join(", ")
}

/// `1.2 MB`, `340 KB`, `12 B`.
pub fn size(bytes: u64) -> String {
    let b = bytes as f64;
    if bytes >= 1_000_000 {
        format!("{:.1} MB", b / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.0} KB", b / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        // 2023-11-14 09:41 UTC
        let t = 1_699_954_860_000;
        assert_eq!(long_date(t), "2023-11-14 09:41 UTC");
        assert_eq!(short_date(t, t + 3_600_000), "09:41");
        assert_eq!(short_date(t, t + 3 * DAY_MS), "Nov 14");
        assert_eq!(short_date(t, t + 400 * DAY_MS), "2023-11-14");
        assert_eq!(age(t, t + 90_000), "1m ago");
    }

    #[test]
    fn sizes_and_names() {
        assert_eq!(size(12), "12 B");
        assert_eq!(size(340_000), "340 KB");
        assert_eq!(size(1_234_567), "1.2 MB");
        let a = Addr {
            name: Some(" ".into()),
            email: "a@x".into(),
        };
        assert_eq!(name(&a), "a@x");
        assert_eq!(full(&a), "a@x");
    }
}
