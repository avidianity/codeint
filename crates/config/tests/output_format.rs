use std::fs;
use std::path::PathBuf;

use codeint_config::{
    ConfigOverrides, ConfigProvenance, ConfigurationFileError, OutputFormat, configuration_schema,
    read_configuration, read_resolved_configuration, write_starter_configuration,
};
use serde_json::{Value, json};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn create(name: &str) -> Sandbox {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("output-format-sandbox")
            .join(name);
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("sandbox dir");
        Sandbox { root }
    }

    fn write(&self, rel: &str, body: &str) {
        let dst = self.root.join(rel);
        if let Some(dir) = dst.parent() {
            fs::create_dir_all(dir).expect("parent dir");
        }
        fs::write(dst, body).expect("write file");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn parses_valid_output_formats_across_all_config_paths_and_defaults_to_toon() {
    let empty = Sandbox::create("default");
    let default_cfg = read_configuration(&empty.root).expect("default config");
    assert_eq!(default_cfg.format, OutputFormat::Toon);

    let cases = [
        (
            "toon-json",
            "codeint.json",
            r#"{ "format": "toon" }"#,
            OutputFormat::Toon,
        ),
        (
            "text-jsonc",
            "codeint.jsonc",
            "{\n  // default format\n  \"format\": \"text\",\n}\n",
            OutputFormat::Text,
        ),
        (
            "json-dot",
            ".codeint/config.json",
            r#"{ "format": "json" }"#,
            OutputFormat::Json,
        ),
        (
            "sarif-dot-jsonc",
            ".codeint/config.jsonc",
            "{ \"format\": \"sarif\", }\n",
            OutputFormat::Sarif,
        ),
        (
            "markdown-json",
            "codeint.json",
            r#"{ "format": "markdown" }"#,
            OutputFormat::Markdown,
        ),
    ];

    for (tag, file_name, body, expected) in cases {
        let box_dir = Sandbox::create(tag);
        box_dir.write(file_name, body);
        let cfg = read_configuration(&box_dir.root).expect("parse format");
        assert_eq!(cfg.format, expected, "case {tag}");
    }

    let toon_box = Sandbox::create("explicit-toon");
    toon_box.write("codeint.json", r#"{ "format": "toon" }"#);
    let explicit_toon = read_configuration(&toon_box.root).expect("explicit toon");

    let text_box = Sandbox::create("explicit-text");
    text_box.write("codeint.json", r#"{ "format": "text" }"#);
    let explicit_text = read_configuration(&text_box.root).expect("explicit text");

    assert_eq!(
        serde_json::to_string(&default_cfg).expect("serialize default"),
        serde_json::to_string(&explicit_toon).expect("serialize explicit toon")
    );
    assert_ne!(
        serde_json::to_string(&default_cfg).expect("serialize default"),
        serde_json::to_string(&explicit_text).expect("serialize explicit text")
    );
}

#[test]
fn rejects_unknown_and_non_string_format_values() {
    let enum_box = Sandbox::create("bad-enum");
    enum_box.write("codeint.json", r#"{ "format": "yaml" }"#);
    assert!(matches!(
        read_configuration(&enum_box.root),
        Err(ConfigurationFileError::NotOneOf { .. })
    ));

    for (tag, body) in [
        ("number", r#"{ "format": 42 }"#),
        ("bool", r#"{ "format": true }"#),
        ("array", r#"{ "format": ["text"] }"#),
    ] {
        let bad = Sandbox::create(tag);
        bad.write("codeint.json", body);
        assert!(
            matches!(
                read_configuration(&bad.root),
                Err(ConfigurationFileError::WrongType { .. })
            ),
            "expected WrongType for {tag}"
        );
    }
}

#[test]
fn schema_resolved_provenance_and_starter_config_handle_format() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("schema json");
    assert!(schema["properties"]["format"].is_object());
    assert_eq!(
        schema["$defs"]["OutputFormat"]["enum"],
        json!(["toon", "text", "json", "sarif", "markdown"])
    );

    let cases = [
        ("res-default", None, "toon", ConfigProvenance::Default),
        (
            "res-file-toon",
            Some(r#"{ "format": "toon" }"#),
            "toon",
            ConfigProvenance::File,
        ),
        (
            "res-file-text",
            Some(r#"{ "format": "text" }"#),
            "text",
            ConfigProvenance::File,
        ),
    ];
    for (tag, body, expected_val, expected_src) in cases {
        let box_dir = Sandbox::create(tag);
        if let Some(text) = body {
            box_dir.write("codeint.json", text);
        }
        let resolved = read_resolved_configuration(&box_dir.root, &ConfigOverrides::default())
            .expect("resolve");
        let entry = resolved
            .settings
            .iter()
            .find(|setting| setting.key == "format")
            .expect("format setting");
        assert_eq!(entry.value, json!(expected_val), "case {tag}");
        assert_eq!(entry.source, expected_src, "case {tag}");
    }

    let starter_box = Sandbox::create("starter");
    let written = write_starter_configuration(&starter_box.root, &[], None).expect("write starter");
    let content = fs::read_to_string(written).expect("read starter");
    assert!(
        !content.contains("\"format\""),
        "unexpected format:\n{content}"
    );
}
