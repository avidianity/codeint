use super::*;

fn population(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("finding {index:03}"))
        .collect()
}

#[test]
fn seeded_sample_is_deterministic_for_a_seed_and_ignores_input_order() {
    let forward = seeded_sample(population(500), 42, 100);
    let mut reversed_input = population(500);
    reversed_input.reverse();
    assert_eq!(forward, seeded_sample(reversed_input, 42, 100));
    assert_eq!(forward.len(), 100);
    assert_ne!(forward, seeded_sample(population(500), 43, 100));
    let unique: std::collections::BTreeSet<&String> = forward.iter().collect();
    assert_eq!(unique.len(), 100);
}

#[test]
fn seeded_sample_keeps_every_item_of_a_small_population() {
    let mut sample = seeded_sample(population(7), 9, 100);
    sample.sort();
    assert_eq!(sample, population(7));
}

#[test]
fn seed_depends_on_the_set_of_commits_not_their_order() {
    let first = vec!["b".repeat(40), "a".repeat(40)];
    let second = vec!["a".repeat(40), "b".repeat(40)];
    assert_eq!(seed_for(&first), seed_for(&second));
    assert_ne!(seed_for(&first), seed_for(&["c".repeat(40)]));
}
