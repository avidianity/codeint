use std::path::PathBuf;

use codeint_config::configuration_schema;

fn published_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/codeint.schema.json")
}

#[test]
fn the_published_schema_matches_the_types_that_the_configuration_crate_parses() {
    let path = published_schema_path();
    let generated = configuration_schema();
    let published = std::fs::read_to_string(&path).unwrap_or_default();

    if published != generated {
        std::fs::write(&path, &generated).expect("rewrite the published schema");
    }

    assert!(
        published == generated,
        "{} drifted from the configuration types; it is rewritten now, so review it, commit it, and run the tests again",
        path.display()
    );
}
