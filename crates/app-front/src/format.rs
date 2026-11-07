//! Display formatting of dates, addresses and sizes. Dates and day groups are shown in local
//! time (MISC-46): the front end passes a [`Zone`], the local UTC offset at each instant, since
//! this crate reads no clock and no time zone data.

use crate::i18n::Key;
use app_core::Addr;
use app_core::date::civil_from_days;

const DAY_MS: i64 = 86_400_000;
const MONTHS: [Key; 12] = [
    Key::Jan,
    Key::Feb,
    Key::Mar,
    Key::Apr,
    Key::May,
    Key::Jun,
    Key::Jul,
    Key::Aug,
    Key::Sep,
    Key::Oct,
    Key::Nov,
    Key::Dec,
];

/// A time zone: the offset from UTC, in ms, of local time at the UTC instant `ms` (Unix ms).
/// Asked per instant, so a summer date read in winter keeps its summer offset. The front ends
/// read it from the system; tests and fixed clocks use [`utc`].
pub type Zone = fn(i64) -> i64;

/// The UTC zone: offset 0 at every instant.
pub const fn utc(_ms: i64) -> i64 {
    0
}

/// The local wall-clock time of the UTC instant `ms`, as Unix ms.
fn local(ms: i64, zone: Zone) -> i64 {
    ms.saturating_add(zone(ms))
}

fn hm(ms: i64) -> (i64, i64) {
    let minutes = ms.rem_euclid(DAY_MS) / 60_000;
    (minutes / 60, minutes % 60)
}

fn month_name(month: u32) -> &'static str {
    MONTHS
        .get(month.saturating_sub(1) as usize)
        .map_or("?", |&key| crate::i18n::s(key))
}

/// Short list date in the local time of `zone`: `09:41` today, `Nov 14` this year,
/// `2023-11-14` otherwise.
pub fn short_date(ms: i64, now_ms: i64, zone: Zone) -> String {
    let (ms, now_ms) = (local(ms, zone), local(now_ms, zone));
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

const FULL_MONTHS: [Key; 12] = [
    Key::January,
    Key::February,
    Key::March,
    Key::April,
    Key::May,
    Key::June,
    Key::July,
    Key::August,
    Key::September,
    Key::October,
    Key::November,
    Key::December,
];

/// The list's date group of a thread dated `ms` (UX-13): `Today`, `Yesterday`, `This week`
/// (since Monday), `Earlier this month`, else `November 2023`; `Later` for the future. Local days of
/// `zone`, like the dates.
pub fn date_group(ms: i64, now_ms: i64, zone: Zone) -> String {
    let (ms, now_ms) = (local(ms, zone), local(now_ms, zone));
    let (day, today) = (ms.div_euclid(DAY_MS), now_ms.div_euclid(DAY_MS));
    // 1970-01-01 was a Thursday: days since the last Monday.
    let since_monday = (today + 3).rem_euclid(7);
    let (year, month, _) = civil_from_days(day);
    let (this_year, this_month, _) = civil_from_days(today);
    if day > today {
        crate::i18n::s(Key::Later).to_owned()
    } else if day == today {
        crate::i18n::s(Key::Today).to_owned()
    } else if day == today - 1 {
        crate::i18n::s(Key::Yesterday).to_owned()
    } else if day >= today - since_monday {
        crate::i18n::s(Key::ThisWeek).to_owned()
    } else if (year, month) == (this_year, this_month) {
        crate::i18n::s(Key::EarlierThisMonth).to_owned()
    } else {
        let name = FULL_MONTHS
            .get(month.saturating_sub(1) as usize)
            .map_or("?", |&key| crate::i18n::s(key));
        format!("{name} {year}")
    }
}

/// Full date in the local time of `zone`, with its offset: `2023-11-14 10:41 +01:00`, or
/// `2023-11-14 09:41 UTC` at offset 0.
pub fn long_date(ms: i64, zone: Zone) -> String {
    let offset = zone(ms);
    let ms = ms.saturating_add(offset);
    let (year, month, d) = civil_from_days(ms.div_euclid(DAY_MS));
    let (h, m) = hm(ms);
    let minutes = offset / 60_000;
    let suffix = if minutes == 0 {
        "UTC".to_owned()
    } else {
        let sign = if minutes < 0 { '-' } else { '+' };
        let minutes = minutes.abs();
        format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60)
    };
    format!("{year}-{month:02}-{d:02} {h:02}:{m:02} {suffix}")
}

/// How long until `at_ms`: `in 45s`, `in 2m`, `in 3h`; `now` when it is due.
pub fn until(at_ms: i64, now_ms: i64) -> String {
    let secs = (at_ms - now_ms).max(0) / 1000;
    match secs {
        0 => crate::i18n::s(Key::Now).to_owned(),
        1..60 => crate::i18n::f(Key::InS, &[("n", &secs.to_string())]),
        60..3600 => crate::i18n::f(Key::InM, &[("n", &(secs / 60).to_string())]),
        _ => crate::i18n::f(Key::InH, &[("n", &(secs / 3600).to_string())]),
    }
}

/// Relative age: `just now`, `5m ago`, `3h ago`, `2d ago`.
pub fn age(ms: i64, now_ms: i64) -> String {
    let secs = (now_ms - ms).max(0) / 1000;
    match secs {
        0..60 => crate::i18n::s(Key::JustNow).to_owned(),
        60..3600 => crate::i18n::f(Key::MAgo, &[("n", &(secs / 60).to_string())]),
        3600..86_400 => crate::i18n::f(Key::HAgo, &[("n", &(secs / 3600).to_string())]),
        _ => crate::i18n::f(Key::DAgo, &[("n", &(secs / 86_400).to_string())]),
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

/// `1,842`: thousands separated by commas.
pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
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

/// `n` in superscript digits (`¹²³`), for link markers in text-only places such as the *Links* list (MISC-28).
pub fn superscript(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| match c {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            _ => '⁹',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_separated() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_842), "1,842");
        assert_eq!(thousands(100_000), "100,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn dates() {
        // 2023-11-14 09:41 UTC
        let t = 1_699_954_860_000;
        assert_eq!(long_date(t, utc), "2023-11-14 09:41 UTC");
        assert_eq!(short_date(t, t + 3_600_000, utc), "09:41");
        assert_eq!(short_date(t, t + 3 * DAY_MS, utc), "Nov 14");
        assert_eq!(short_date(t, t + 400 * DAY_MS, utc), "2023-11-14");
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

    /// Wednesday 2023-11-15 08:00 UTC.
    const WED_NOW: i64 = 1_700_035_200_000;
    const DAY: i64 = 86_400_000;

    #[test]
    fn date_groups_from_today_back() {
        let at = |days: i64| date_group(WED_NOW - days * DAY, WED_NOW, utc);
        assert_eq!(at(0), "Today");
        assert_eq!(at(1), "Yesterday");
        assert_eq!(at(2), "This week", "Monday the 13th");
        assert_eq!(at(3), "Earlier this month", "Sunday the 12th");
        assert_eq!(at(14), "Earlier this month", "the 1st");
        assert_eq!(at(15), "October 2023");
        assert_eq!(at(365), "November 2022");
        assert_eq!(date_group(WED_NOW + DAY, WED_NOW, utc), "Later");
    }
}
