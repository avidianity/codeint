use super::{format_similarity_percent, reference_range};
use crate::{Decimal, ReferenceRangeEntry};

#[test]
fn a_published_reference_range_shows_its_bounds_and_its_source_with_markdown_escaped() {
    let range = ReferenceRangeEntry::Published {
        low: Decimal::rounded(0.05),
        high: Decimal::rounded(0.2),
        source: "Survey <2024> of *open* code".to_string(),
    };

    assert_eq!(
        reference_range(&range),
        "0.05 to 0.2, source: Survey \\<2024\\> of \\*open\\* code"
    );
}

#[test]
fn tiny_positive_similarity_formats_as_less_than_one_percent() {
    assert_eq!(format_similarity_percent(0.00001), "<1%");
    assert_eq!(format_similarity_percent(0.0049), "<1%");
    assert_eq!(format_similarity_percent(0.005), "1%");
    assert_eq!(format_similarity_percent(0.85), "85%");
    assert_eq!(format_similarity_percent(1.0), "100%");
}
