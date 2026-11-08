//! The system zone the front ends show dates in (MISC-46).

use app_sync::clock::local_offset_ms;

#[test]
fn the_local_offset_is_a_whole_minute_within_a_day() {
    for at in [0, 1_700_035_200_000, 1_689_000_000_000, -86_400_000] {
        let offset = local_offset_ms(at);
        assert!(offset.abs() < 86_400_000, "{offset}");
        assert_eq!(offset % 60_000, 0, "{offset}");
    }
    assert_eq!(local_offset_ms(i64::MAX), 0, "out of range");
}
