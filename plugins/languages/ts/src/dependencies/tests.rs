use codeint_model::language::{DeclaredDep, DepKind, Manifest};

use super::extract_declared;

fn assert_root_declared(manifest_json: &str, expected: &[(&str, DepKind)]) {
    let manifests = [Manifest {
        path: "package.json".to_owned(),
        text: manifest_json.to_owned(),
    }];
    let expected_deps: Vec<DeclaredDep> = expected
        .iter()
        .map(|&(name, kind)| DeclaredDep {
            package_root: String::new(),
            name: name.to_owned(),
            kind,
        })
        .collect();
    assert_eq!(extract_declared(&manifests), expected_deps);
}

#[test]
fn ambient_types_packages_classify_as_optional_and_script_dev_deps_stay_dev() {
    assert_root_declared(
        r#"{
          "name": "typed-app",
          "scripts": {
            "test": "vitest run",
            "typecheck": "tsc --noEmit"
          },
          "dependencies": {
            "zod": "^3.0.0",
            "@types/lodash": "^4.17.0"
          },
          "devDependencies": {
            "@types/node": "^22.0.0",
            "@types/react": "^18.3.0",
            "typescript": "^5.7.0",
            "vitest": "^2.1.0",
            "faker": "^8.0.0"
          }
        }"#,
        &[
            ("zod", DepKind::Runtime),
            ("faker", DepKind::Dev),
            ("typescript", DepKind::Dev),
            ("vitest", DepKind::Dev),
            ("@types/lodash", DepKind::Optional),
            ("@types/node", DepKind::Optional),
            ("@types/react", DepKind::Optional),
        ],
    );
}
