use super::*;

fn count(matched: u64, total: u64) -> RecallCount {
    RecallCount { matched, total }
}

#[test]
fn a_percentage_below_the_bar_never_displays_as_the_bar() {
    assert_eq!(percent(&count(48_999, 50_000)), "97.99% (48999/50000)");
    assert_eq!(
        percent(&count(979_999, 1_000_000)),
        "97.99% (979999/1000000)"
    );
}

#[test]
fn a_percentage_at_or_above_the_bar_displays_its_value() {
    assert_eq!(percent(&count(49_000, 50_000)), "98.00% (49000/50000)");
    assert_eq!(percent(&count(1, 1)), "100.00% (1/1)");
    assert_eq!(percent(&count(0, 0)), "100.00% (0/0)");
    assert_eq!(percent(&count(0, 7)), "0.00% (0/7)");
}
