use std::collections::BTreeMap;
use std::path::Path;

use codeint_eval::{SizeBucket, load_corpus};
use codeint_plugin_host::built_in_language_names;

#[test]
fn corpus_pins_five_stratified_public_repositories_per_built_in_language() {
    let corpus = load_corpus(&Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus.json"))
        .expect("valid corpus");
    assert_eq!(corpus.repositories.len(), 40);
    let mut buckets: BTreeMap<&str, Vec<SizeBucket>> = BTreeMap::new();
    for entry in &corpus.repositories {
        assert_eq!(
            entry.url,
            format!("https://github.com/{}", entry.name),
            "{entry:?}"
        );
        buckets
            .entry(entry.language.as_str())
            .or_default()
            .push(entry.bucket);
    }
    let stratified = vec![
        SizeBucket::Small,
        SizeBucket::Small,
        SizeBucket::Medium,
        SizeBucket::Medium,
        SizeBucket::Large,
    ];
    for language in built_in_language_names() {
        let mut found = buckets.remove(language).unwrap_or_default();
        found.sort();
        assert_eq!(found, stratified, "{language}");
    }
    assert!(buckets.is_empty(), "{buckets:?}");
}
