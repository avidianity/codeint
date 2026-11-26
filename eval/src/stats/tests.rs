use super::*;

#[test]
fn percentiles_interpolate_linearly_between_sorted_values() {
    let values = [0.0, 0.1, 0.2, 0.3, 0.4];
    assert!((percentile(&values, 0.10) - 0.04).abs() < 1e-12);
    assert!((percentile(&values, 0.90) - 0.36).abs() < 1e-12);
    assert!((percentile(&values, 0.5) - 0.2).abs() < 1e-12);
    assert_eq!(percentile(&[0.5], 0.9), 0.5);
}

#[test]
fn distribution_reports_the_published_statistics_of_unsorted_values() {
    let distribution = Distribution::of(&[0.4, 0.0, 0.2, 0.1, 0.3]).expect("values");
    assert_eq!(distribution.count, 5);
    assert_eq!(distribution.lowest, 0.0);
    assert_eq!(distribution.highest, 0.4);
    assert!((distribution.median - 0.2).abs() < 1e-12);
    assert!((distribution.mean - 0.2).abs() < 1e-12);
    assert!((distribution.standard_deviation - 0.141_421_356).abs() < 1e-6);
    assert!((distribution.p10 - 0.04).abs() < 1e-12);
    assert!((distribution.p90 - 0.36).abs() < 1e-12);
}

#[test]
fn distribution_of_nothing_is_absent() {
    assert_eq!(Distribution::of(&[]), None);
}
