use codeint_model::language::SourceSet;
use codeint_model::{LanguagePlugin, RepoPath};

use super::classify;
use crate::Dart;

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

#[test]
fn dartdoc_mentioning_generated_code_remains_production() {
    let doc_only = "/// Helper library used by generated code at runtime.\nlibrary helper;\nint id(int x) => x;\n";
    let model = Dart
        .lower(
            &RepoPath::new("lib/internal/helper.dart").unwrap(),
            doc_only,
            None,
        )
        .unwrap();
    assert_eq!(model.source_set, SourceSet::Production);

    let banner = "// GENERATED CODE - DO NOT MODIFY BY HAND\nint id(int x) => x;\n";
    let gen_model = Dart
        .lower(
            &RepoPath::new("lib/internal/schema.dart").unwrap(),
            banner,
            None,
        )
        .unwrap();
    assert_eq!(gen_model.source_set, SourceSet::Generated);
}
