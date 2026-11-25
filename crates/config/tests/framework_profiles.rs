use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use codeint_config::{
    ConfigFileKind, ConfigOverrides, ConfigurationFileError, configuration_schema,
    read_configuration, read_resolved_configuration,
};
use codeint_model::FrameworkProfileSelection;
use serde_json::{Value, json};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp_config(body: &str) -> TempDir {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "codeint-config-framework-profiles-{}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create temp dir");
    fs::write(path.join("codeint.json"), body).expect("write codeint.json");
    TempDir(path)
}

#[test]
fn parses_global_and_per_package_framework_profile_forms_and_normalizes_package_keys() {
    let dir = temp_config(
        r#"{
          "framework_profiles": { "exclude": ["svelte"] },
          "packages": {
            ".": { "framework_profiles": false },
            "./packages/web/": { "framework_profiles": ["react"] },
            "packages/admin": { "framework_profiles": { "only": ["vue3"] } },
            "packages/docs": { "framework_profiles": true }
          }
        }"#,
    );
    let resolved = read_resolved_configuration(dir.path(), &ConfigOverrides::default())
        .expect("read resolved config");
    let fp = &resolved.configuration.framework_profiles;
    assert_eq!(
        fp.global(),
        &FrameworkProfileSelection::Exclude(vec!["svelte".to_owned()])
    );
    assert_eq!(
        fp.selection_for_package(""),
        FrameworkProfileSelection::Disabled
    );
    assert_eq!(
        fp.selection_for_package("packages/web"),
        FrameworkProfileSelection::Only(vec!["react".to_owned()])
    );
    assert_eq!(
        fp.selection_for_package("packages/admin"),
        FrameworkProfileSelection::Only(vec!["vue3".to_owned()])
    );
    assert_eq!(
        fp.selection_for_package("packages/docs"),
        FrameworkProfileSelection::Auto
    );
    assert_eq!(
        fp.selection_for_package("packages/other"),
        FrameworkProfileSelection::Exclude(vec!["svelte".to_owned()])
    );

    let known = ["react", "vue2", "vue3", "svelte"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    fp.validate_known(ConfigFileKind::Json, &known)
        .expect("all configured profiles are known");
}

#[test]
fn rejects_invalid_framework_profile_shapes_conflicts_and_unknown_profile_names() {
    let conflict =
        temp_config(r#"{ "framework_profiles": { "exclude": ["react"], "only": ["vue3"] } }"#);
    assert!(matches!(
        read_configuration(conflict.path()),
        Err(ConfigurationFileError::ConflictingFrameworkProfileRule { .. })
    ));

    let empty_obj = temp_config(r#"{ "framework_profiles": {} }"#);
    assert!(matches!(
        read_configuration(empty_obj.path()),
        Err(ConfigurationFileError::EmptyFrameworkProfileObject { .. })
    ));

    let empty_name = temp_config(r#"{ "framework_profiles": [""] }"#);
    assert!(matches!(
        read_configuration(empty_name.path()),
        Err(ConfigurationFileError::EmptyFrameworkProfileName { .. })
    ));

    let wrong_type = temp_config(r#"{ "framework_profiles": 42 }"#);
    assert!(matches!(
        read_configuration(wrong_type.path()),
        Err(ConfigurationFileError::WrongType { .. })
    ));

    let bad_key = temp_config(r#"{ "framework_profiles": { "unknown": ["react"] } }"#);
    assert!(matches!(
        read_configuration(bad_key.path()),
        Err(ConfigurationFileError::UnknownKey { .. })
    ));

    let bad_pkg =
        temp_config(r#"{ "packages": { "../escape": { "framework_profiles": false } } }"#);
    assert!(matches!(
        read_configuration(bad_pkg.path()),
        Err(ConfigurationFileError::InvalidPackageConfigKey { .. })
    ));

    let dup_pkg = temp_config(
        r#"{
          "packages": {
            "packages/web": { "framework_profiles": false },
            "packages/web/": { "framework_profiles": ["react"] }
          }
        }"#,
    );
    assert!(matches!(
        read_configuration(dup_pkg.path()),
        Err(ConfigurationFileError::DuplicatePackageConfigKey { .. })
    ));

    let typo = temp_config(r#"{ "framework_profiles": ["reacct"] }"#);
    let config = read_configuration(typo.path()).expect("shape parses");
    let known = vec!["react".to_owned(), "vue3".to_owned()];
    let err = config
        .framework_profiles
        .validate_known(ConfigFileKind::Json, &known)
        .expect_err("unknown profile must fail");
    assert!(matches!(
        err,
        ConfigurationFileError::UnknownFrameworkProfile { ref name, .. } if name == "reacct"
    ));
}

#[test]
fn configuration_schema_exposes_framework_profiles_and_packages() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("valid JSON schema");
    let properties = schema["properties"]
        .as_object()
        .expect("root properties object");
    assert!(properties.contains_key("framework_profiles"));
    assert!(properties.contains_key("packages"));

    let default_dir = temp_config("{}");
    let resolved = read_resolved_configuration(default_dir.path(), &ConfigOverrides::default())
        .expect("default config");
    assert!(
        resolved
            .settings
            .iter()
            .any(|setting| setting.key == "framework_profiles" && setting.value == json!("auto"))
    );
}

#[test]
fn nested_package_framework_profiles_reports_inner_unknown_key_and_all_allowed_types() {
    let unknown_key_dir = temp_config(
        r#"{ "packages": { "packages/web": { "framework_profiles": { "unknown": ["react"] } } } }"#,
    );
    let err = read_configuration(unknown_key_dir.path()).expect_err("unknown nested key must fail");
    match &err {
        ConfigurationFileError::UnknownKey { key, allowed, .. } => {
            assert_eq!(
                key.to_string(),
                "packages.packages/web.framework_profiles.unknown"
            );
            assert_eq!(allowed, &vec!["exclude".to_owned(), "only".to_owned()]);
        }
        other => panic!("expected UnknownKey, got {other:?}"),
    }

    let wrong_type_dir =
        temp_config(r#"{ "packages": { "packages/web": { "framework_profiles": 42 } } }"#);
    let err = read_configuration(wrong_type_dir.path()).expect_err("wrong nested type must fail");
    let rendered = err.to_string();
    assert!(
        rendered.contains("boolean")
            && rendered.contains("array")
            && rendered.contains("object")
            && rendered.contains("null"),
        "expected all allowed types in error, got: {rendered}"
    );
}
