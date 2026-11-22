use codeint_model::language::SourceSet;

use super::classify;

#[test]
fn classifies_dart_generated_test_and_production_files() {
    assert_eq!(classify("lib/user.g.dart", ""), SourceSet::Generated);
    assert_eq!(classify("lib/user.freezed.dart", ""), SourceSet::Generated);
    assert_eq!(
        classify(
            "lib/schema.dart",
            "// GENERATED CODE - DO NOT MODIFY BY HAND\n"
        ),
        SourceSet::Generated
    );
    assert_eq!(classify("test/widget_test.dart", ""), SourceSet::Test);
    assert_eq!(classify("integration_test/app.dart", ""), SourceSet::Test);
    assert_eq!(
        classify("packages/app/lib/calc_test.dart", ""),
        SourceSet::Test
    );
    assert_eq!(classify("lib/src/router.dart", ""), SourceSet::Production);
}
