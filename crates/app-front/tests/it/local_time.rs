//! Dates and day groups in local time (MISC-46): the front end's zone moves the day boundaries.

use app_front::format::{Zone, date_group, long_date, short_date, utc};

/// Wednesday 2023-11-15 08:00 UTC.
const WED_NOW: i64 = 1_700_035_200_000;
const HOUR: i64 = 3_600_000;
const MINUTE: i64 = 60_000;

const fn east3(_: i64) -> i64 {
    3 * HOUR
}

const fn west5(_: i64) -> i64 {
    -5 * HOUR
}

const fn newfoundland(_: i64) -> i64 {
    -(3 * HOUR + 30 * MINUTE)
}

/// A zone with daylight saving time: +02:00 before WED_NOW - 30 days, +01:00 after.
const fn dst(at_ms: i64) -> i64 {
    if at_ms < WED_NOW - 30 * 24 * HOUR {
        2 * HOUR
    } else {
        HOUR
    }
}

#[test]
fn long_date_shows_local_time_and_offset() {
    // 2023-11-14 09:41 UTC.
    let t = 1_699_954_860_000;
    assert_eq!(long_date(t, utc), "2023-11-14 09:41 UTC");
    assert_eq!(long_date(t, east3), "2023-11-14 12:41 +03:00");
    assert_eq!(long_date(t, west5), "2023-11-14 04:41 -05:00");
    assert_eq!(long_date(t, newfoundland), "2023-11-14 06:11 -03:30");
    assert_eq!(long_date(t - 10 * HOUR, west5), "2023-11-13 18:41 -05:00");
}

#[test]
fn the_offset_is_asked_per_instant() {
    // A summer date read in winter keeps its summer offset.
    let summer = WED_NOW - 60 * 24 * HOUR;
    assert_eq!(long_date(summer, dst), "2023-09-16 10:00 +02:00");
    assert_eq!(long_date(WED_NOW, dst), "2023-11-15 09:00 +01:00");
}

#[test]
fn short_date_is_local() {
    let ms = WED_NOW - 9 * HOUR - 30 * MINUTE; // Tue 22:30 UTC, Wed 01:30 at +03:00.
    assert_eq!(short_date(ms, WED_NOW, utc), "Nov 14");
    assert_eq!(short_date(ms, WED_NOW, east3), "01:30");
    let ms = WED_NOW - 3 * HOUR - 30 * MINUTE; // Wed 04:30 UTC, Tue 23:30 at -05:00.
    assert_eq!(short_date(ms, WED_NOW, utc), "04:30");
    assert_eq!(short_date(ms, WED_NOW, west5), "Nov 14");
}

#[test]
fn short_date_changes_year_at_local_midnight() {
    // 2023-12-31 23:00 UTC is 2024-01-01 00:00 at +01:00.
    let ms = 1_704_063_600_000;
    let one = |_: i64| HOUR;
    assert_eq!(short_date(ms, ms + 30 * MINUTE, one), "00:00");
    assert_eq!(short_date(ms - MINUTE, ms + 30 * MINUTE, one), "2023-12-31");
    assert_eq!(short_date(ms - MINUTE, ms + 30 * MINUTE, utc), "22:59");
}

#[test]
fn today_locally_is_yesterday_in_utc() {
    let ms = WED_NOW - 9 * HOUR - 30 * MINUTE; // Tue 22:30 UTC, Wed 01:30 at +03:00.
    assert_eq!(date_group(ms, WED_NOW, utc), "Yesterday");
    assert_eq!(date_group(ms, WED_NOW, east3), "Today");
}

#[test]
fn yesterday_locally_is_today_in_utc() {
    let ms = WED_NOW - 3 * HOUR - 30 * MINUTE; // Wed 04:30 UTC, Tue 23:30 at -05:00.
    assert_eq!(date_group(ms, WED_NOW, utc), "Today");
    assert_eq!(date_group(ms, WED_NOW, west5), "Yesterday");
}

#[test]
fn groups_turn_at_local_midnight() {
    // At +03:00, Wednesday starts at Tue 21:00 UTC.
    let midnight = WED_NOW - 11 * HOUR;
    let zone: Zone = east3;
    assert_eq!(date_group(midnight, WED_NOW, zone), "Today");
    assert_eq!(date_group(midnight - 1, WED_NOW, zone), "Yesterday");
    // Thursday starts at Wed 21:00 UTC: later than now.
    assert_eq!(date_group(midnight + 24 * HOUR - 1, WED_NOW, zone), "Today");
    assert_eq!(date_group(midnight + 24 * HOUR, WED_NOW, zone), "Later");
    // The week turns at local midnight too: Monday 00:00 at +03:00 is Sun 21:00 UTC.
    let monday = midnight - 2 * 24 * HOUR;
    assert_eq!(date_group(monday, WED_NOW, zone), "This week");
    assert_eq!(date_group(monday - 1, WED_NOW, zone), "Earlier this month");
}

#[test]
fn now_itself_moves_with_the_zone() {
    // Wed 08:00 UTC is Wed 03:00 at -05:00: a message at Wed 02:00 local (07:00 UTC) is today
    // either way, and one at 23:59 local Tuesday is not.
    assert_eq!(date_group(WED_NOW - HOUR, WED_NOW, west5), "Today");
    assert_eq!(
        date_group(WED_NOW - 3 * HOUR - MINUTE, WED_NOW, west5),
        "Yesterday"
    );
}
