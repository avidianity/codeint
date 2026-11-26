use super::*;

fn sample(category: AuditCategory, items: &[&str]) -> AuditSample {
    AuditSample {
        category,
        seed: "00".to_owned(),
        population: items.len() as u64,
        items: items.iter().map(|item| (*item).to_owned()).collect(),
    }
}

fn verdict(category: AuditCategory, item: &str, judgement: Judgement) -> Verdict {
    Verdict {
        category,
        item: item.to_owned(),
        verdict: judgement,
        note: String::new(),
    }
}

#[test]
fn a_review_counts_false_positives_and_unreviewed_items_per_category() {
    let samples = [
        sample(AuditCategory::FlaggedLines, &["a", "b", "c", "d"]),
        sample(AuditCategory::OrphanFiles, &["x"]),
    ];
    let file = VerdictFile {
        reviewer: "agent".to_owned(),
        verdicts: vec![
            verdict(AuditCategory::FlaggedLines, "a", Judgement::Correct),
            verdict(AuditCategory::FlaggedLines, "b", Judgement::FalsePositive),
            verdict(AuditCategory::FlaggedLines, "c", Judgement::Correct),
            verdict(
                AuditCategory::FlaggedLines,
                "gone",
                Judgement::FalsePositive,
            ),
        ],
    };
    let review = review_samples(&samples, &file);
    assert_eq!(review.reviewer, "agent");
    let flagged = &review.categories[0];
    assert_eq!(
        (flagged.sampled, flagged.reviewed, flagged.false_positives),
        (4, 3, 1)
    );
    assert!((flagged.false_positive_percent - 100.0 / 3.0).abs() < 1e-9);
    let orphans = &review.categories[1];
    assert_eq!((orphans.sampled, orphans.reviewed), (1, 0));
    assert_eq!(orphans.false_positive_percent, 0.0);
}
