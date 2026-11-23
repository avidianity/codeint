use codeint_model::language::{ImportRequest, Resolution};

use super::resolve_all;
use crate::stdlib::{is_stdlib_module, stdlib_table};

#[test]
fn stdlib_table_is_strictly_sorted_and_covers_core_modules() {
    let table = stdlib_table();
    for pair in table.windows(2) {
        assert!(pair[0] < pair[1], "unsorted stdlib entries: {pair:?}");
    }
    for name in ["os", "sys", "typing", "tomllib", "cProfile", "dataclasses"] {
        assert!(is_stdlib_module(name), "expected stdlib module {name}");
    }
    for third_party in [
        "requests",
        "pydantic_settings",
        "typing_extensions",
        "numpy",
    ] {
        assert!(
            !is_stdlib_module(third_party),
            "unexpected stdlib hit {third_party}"
        );
    }
}

#[test]
fn classifies_stdlib_and_normalizes_third_party_modules() {
    let files = vec!["src/app.py".to_owned()];
    let reqs = ["os.path", "urllib.parse", "typing_extensions", "My_Pkg.sub"]
        .into_iter()
        .map(|spec| ImportRequest {
            importer: "src/app.py".into(),
            specifier: spec.into(),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        resolve_all(&files, &[], &reqs),
        vec![
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::External("typing-extensions".into()),
            Resolution::External("my-pkg".into()),
        ]
    );
}
