use codeint_config::RegressionLimit;

use super::{MetricChanges, RegressionRule};
use crate::{Decimal, Findings};

fn ratio_changes(base_ratio: f64, head_ratio: f64) -> MetricChanges<()> {
    MetricChanges {
        base_ratio: Decimal::rounded(base_ratio),
        head_ratio: Decimal::rounded(head_ratio),
        base_numerator: Decimal(1.0),
        head_numerator: Decimal(1.0),
        new: Findings {
            listed: Vec::new(),
            left_out: 0,
        },
        resolved: Findings {
            listed: Vec::new(),
            left_out: 0,
        },
        unchanged: 0,
    }
}

#[test]
fn a_displayed_rise_of_0_01_passes_even_when_the_float_difference_is_above_0_01() {
    let changes = ratio_changes(0.0042, 0.0142);

    assert!(!changes.breaks(RegressionRule::RatioRise, RegressionLimit::DEFAULT));
}

#[test]
fn a_displayed_rise_of_0_0101_is_a_regression() {
    let changes = ratio_changes(0.0042, 0.0143);

    assert!(changes.breaks(RegressionRule::RatioRise, RegressionLimit::DEFAULT));
}
