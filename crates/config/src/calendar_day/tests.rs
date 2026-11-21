use super::CalendarDay;

fn day(text: &str) -> CalendarDay {
    text.parse().expect("a valid day")
}

#[test]
fn the_unix_epoch_starts_on_1970_01_01() {
    assert_eq!(day("1970-01-01").start_unix_seconds(), 0);
    assert_eq!(CalendarDay::containing(0).to_string(), "1970-01-01");
    assert_eq!(CalendarDay::containing(-1).to_string(), "1969-12-31");
}

#[test]
fn known_days_start_at_their_published_unix_seconds() {
    for (text, seconds) in [
        ("2000-03-01", 951_868_800),
        ("2024-02-29", 1_709_164_800),
        ("2026-11-21", 1_795_219_200),
        ("1900-03-01", -2_203_891_200),
    ] {
        assert_eq!(day(text).start_unix_seconds(), seconds, "{text}");
        assert_eq!(CalendarDay::containing(seconds).to_string(), text);
        assert_eq!(CalendarDay::containing(seconds + 86_399).to_string(), text);
    }
}

#[test]
fn every_day_from_1600_to_2400_round_trips_through_its_unix_seconds() {
    let first = day("1600-01-01").start_unix_seconds();
    let last = day("2400-12-31").start_unix_seconds();
    let mut previous: Option<CalendarDay> = None;
    for start in (first..=last).step_by(86_400) {
        let current = CalendarDay::containing(start);
        assert_eq!(current.start_unix_seconds(), start);
        assert_eq!(day(&current.to_string()), current);
        if let Some(previous) = previous {
            assert!(previous < current);
        }
        previous = Some(current);
    }
}

#[test]
fn only_real_days_in_the_form_yyyy_mm_dd_parse() {
    for text in [
        "2023-02-29",
        "1900-02-29",
        "2026-13-01",
        "2026-00-10",
        "2026-04-31",
        "2026-01-00",
        "2026-1-01",
        "2026-01-1",
        "+026-01-01",
        "2026-01-01T00:00:00Z",
        "2026/01/01",
        "",
    ] {
        assert!(text.parse::<CalendarDay>().is_err(), "{text:?} parsed");
    }
    assert_eq!(day("2000-02-29").to_string(), "2000-02-29");
}
