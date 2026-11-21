use super::Decimal;

#[test]
fn a_negative_zero_from_an_empty_sum_is_reported_as_zero() {
    let empty_sum: f64 = std::iter::empty::<f64>().sum();

    let rendered = serde_json::to_string(&Decimal::rounded(empty_sum)).expect("serialize");

    assert_eq!(rendered, "0.0");
}

#[test]
fn a_tiny_negative_value_that_rounds_to_zero_is_reported_as_zero() {
    let rendered = serde_json::to_string(&Decimal::rounded(-0.00001)).expect("serialize");

    assert_eq!(rendered, "0.0");
}
