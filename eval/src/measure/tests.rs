use super::*;

#[test]
fn peak_rss_parses_bsd_bytes_and_gnu_kilobytes() {
    let bsd = "        0.10 real         0.05 user         0.01 sys\n            15204352  maximum resident set size\n";
    assert_eq!(parse_peak_rss(bsd), Some(15_204_352));
    let gnu =
        "\tCommand being timed: \"codeint check\"\n\tMaximum resident set size (kbytes): 2048\n";
    assert_eq!(parse_peak_rss(gnu), Some(2_097_152));
    assert_eq!(parse_peak_rss(""), None);
}

#[test]
fn median_takes_the_middle_value_or_the_mean_of_the_two_middle_values() {
    assert_eq!(median(&[]), None);
    assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
    assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), Some(2.5));
}
