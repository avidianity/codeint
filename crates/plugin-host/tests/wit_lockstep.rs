use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use codeint_model::{
    CodeModel, FrameworkProfileSelection, LanguagePlugin, Manifest, Metric, MetricPlugin, RepoPath,
};
use codeint_plugin_host::{
    PLUGIN_INTERFACE_VERSION, TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME, built_in_language_plugin,
    built_in_language_plugins,
};

#[derive(Debug, PartialEq, Eq)]
struct WitContract {
    package_version: String,
    exported_interfaces: BTreeSet<String>,
    functions: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct LockstepDrift {
    contract: String,
    missing_in_rust: BTreeSet<String>,
    extra_in_rust: BTreeSet<String>,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/plugin-host lives two levels under workspace root")
        .to_path_buf()
}

fn read_workspace_file(relative_path: &str) -> String {
    let full_path = workspace_root().join(relative_path);
    fs::read_to_string(&full_path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", full_path.display()))
}

fn skip_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    index += 2;
    while index < bytes.len() && !bytes[index..].starts_with(b"*/") {
        index += 1;
    }
    (index + 2).min(bytes.len())
}

fn strip_wit_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"//") {
            index = skip_line_comment(bytes, index);
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            index = skip_block_comment(bytes, index);
            continue;
        }
        let ch = source[index..].chars().next().expect("valid UTF-8");
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

fn parse_wit_package_version(stripped: &str) -> Result<String, String> {
    let package_line = stripped
        .split(';')
        .map(str::trim)
        .find(|statement| statement.starts_with("package "))
        .ok_or_else(|| "WIT file is missing a package declaration".to_owned())?;
    let (_, version) = package_line
        .split_once('@')
        .ok_or_else(|| format!("WIT package declaration {package_line:?} has no @version"))?;
    Ok(version.trim().to_owned())
}

fn is_rust_char_literal(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes.get(index) != Some(&b'\'') {
        return None;
    }
    if bytes.get(index + 1) == Some(&b'\\') && bytes.get(index + 3) == Some(&b'\'') {
        return Some(index + 4);
    }
    if bytes.get(index + 2) == Some(&b'\'') && bytes.get(index + 1) != Some(&b'\'') {
        return Some(index + 3);
    }
    None
}

fn skip_quoted_string(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        if bytes[index] == b'"' {
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

fn skip_literal_end(bytes: &[u8], index: usize) -> Option<usize> {
    if bytes[index] == b'"' {
        return Some(skip_quoted_string(bytes, index));
    }
    is_rust_char_literal(bytes, index)
}

fn find_block_body<'a>(source: &'a str, header: &str) -> Result<&'a str, String> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("missing block header {header:?}"))?;
    let after_header = &source[start + header.len()..];
    let open_offset = after_header
        .find('{')
        .ok_or_else(|| format!("missing opening brace for {header:?}"))?;
    Ok(&after_header[open_offset + 1..])
}

fn record_depth_one_char(ch: char, depth: &mut usize, output: &mut String) -> bool {
    match ch {
        '{' => *depth += 1,
        '}' => {
            *depth -= 1;
            if *depth == 0 {
                return true;
            }
            if *depth == 1 {
                output.push(';');
            }
        }
        other if *depth == 1 => output.push(other),
        _ => {}
    }
    false
}

fn extract_depth_one_block(source: &str, header: &str) -> Result<String, String> {
    let body = find_block_body(source, header)?;
    let bytes = body.as_bytes();
    let mut depth = 1usize;
    let mut output = String::new();
    let mut index = 0;

    while index < bytes.len() {
        if let Some(next) = skip_literal_end(bytes, index) {
            if depth == 1 {
                output.push_str(&body[index..next]);
            }
            index = next;
            continue;
        }
        let ch = body[index..].chars().next().expect("valid UTF-8");
        index += ch.len_utf8();
        if record_depth_one_char(ch, &mut depth, &mut output) {
            return Ok(output);
        }
    }
    Err(format!("unclosed block for {header:?}"))
}

fn parse_wit_contract(
    source: &str,
    interface_name: &str,
    world_name: &str,
) -> Result<WitContract, String> {
    let stripped = strip_wit_comments(source);
    let package_version = parse_wit_package_version(&stripped)?;

    let world_body = extract_depth_one_block(&stripped, &format!("world {world_name}"))?;
    let exported_interfaces = world_body
        .split(';')
        .map(str::trim)
        .filter_map(|statement| statement.strip_prefix("export "))
        .map(|exported| exported.trim().to_owned())
        .collect();

    let interface_body =
        extract_depth_one_block(&stripped, &format!("interface {interface_name}"))?;
    let functions = interface_body
        .split(';')
        .filter_map(parse_wit_function_name)
        .collect();

    Ok(WitContract {
        package_version,
        exported_interfaces,
        functions,
    })
}

fn load_verified_wit_contract(path: &str, interface_name: &str, world_name: &str) -> WitContract {
    let wit_source = read_workspace_file(path);
    let contract = parse_wit_contract(&wit_source, interface_name, world_name)
        .unwrap_or_else(|error| panic!("cannot parse {path}: {error}"));
    assert_eq!(contract.package_version, PLUGIN_INTERFACE_VERSION);
    assert_eq!(
        contract.exported_interfaces,
        BTreeSet::from([interface_name.to_owned()])
    );
    assert!(!contract.functions.is_empty());
    contract
}

fn parse_wit_function_name(statement: &str) -> Option<String> {
    let (name, rhs) = statement.trim().split_once(':')?;
    let rhs = rhs.trim_start();
    if !rhs.starts_with("func(") && !rhs.starts_with("func ") {
        return None;
    }
    let name = name.trim().strip_prefix('%').unwrap_or(name.trim());
    (!name.is_empty()).then(|| name.to_owned())
}

fn wit_function_to_rust_method(wit_name: &str) -> String {
    wit_name.replace('-', "_")
}

fn parse_rust_block_methods(source: &str, header: &str) -> Result<BTreeSet<String>, String> {
    let body = extract_depth_one_block(source, header)?;
    Ok(body
        .split(';')
        .filter_map(extract_leading_fn_name)
        .collect())
}

fn extract_leading_fn_name(item: &str) -> Option<String> {
    let mut tokens = item.split_whitespace();
    while let Some(token) = tokens.next() {
        if matches!(token, "pub" | "async" | "unsafe" | "const") || token.starts_with("pub(") {
            continue;
        }
        if token != "fn" {
            return None;
        }
        let signature = tokens.next()?;
        let name: String = signature
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        return (!name.is_empty()).then_some(name);
    }
    None
}

fn verify_lockstep(
    contract: &str,
    wit_functions: &BTreeSet<String>,
    rust_methods: &BTreeSet<String>,
) -> Result<(), LockstepDrift> {
    let expected_rust_methods: BTreeSet<String> = wit_functions
        .iter()
        .map(String::as_str)
        .map(wit_function_to_rust_method)
        .collect();
    let missing_in_rust: BTreeSet<String> = expected_rust_methods
        .difference(rust_methods)
        .cloned()
        .collect();
    let extra_in_rust: BTreeSet<String> = rust_methods
        .difference(&expected_rust_methods)
        .cloned()
        .collect();
    if missing_in_rust.is_empty() && extra_in_rust.is_empty() {
        Ok(())
    } else {
        Err(LockstepDrift {
            contract: contract.to_owned(),
            missing_in_rust,
            extra_in_rust,
        })
    }
}

fn assert_lockstep(
    contract: &str,
    wit_functions: &BTreeSet<String>,
    rust_methods: &BTreeSet<String>,
) {
    if let Err(drift) = verify_lockstep(contract, wit_functions, rust_methods) {
        panic!(
            "{}: WIT and Rust methods drifted; missing_in_rust={:?}, extra_in_rust={:?}",
            drift.contract, drift.missing_in_rust, drift.extra_in_rust
        );
    }
}

fn assert_rust_block_lockstep(
    label: &str,
    wit_functions: &BTreeSet<String>,
    rust_source: &str,
    block_header: &str,
) {
    let rust_methods = parse_rust_block_methods(rust_source, block_header)
        .unwrap_or_else(|error| panic!("cannot parse {label}: {error}"));
    assert_lockstep(label, wit_functions, &rust_methods);
}

struct WitDriftCase<'a> {
    wit_path: &'a str,
    interface_name: &'a str,
    world_name: &'a str,
    target: &'a str,
    replacement: &'a str,
    rust_path: &'a str,
    trait_header: &'a str,
    expected_missing: &'a [&'a str],
    expected_extra: &'a [&'a str],
}

fn assert_mutated_wit_drift(case: WitDriftCase<'_>) {
    let mutated_wit = read_workspace_file(case.wit_path).replace(case.target, case.replacement);
    let contract = parse_wit_contract(&mutated_wit, case.interface_name, case.world_name)
        .expect("parse mutated WIT");
    let rust_source = read_workspace_file(case.rust_path);
    let trait_methods =
        parse_rust_block_methods(&rust_source, case.trait_header).expect("parse trait methods");
    let drift = verify_lockstep(case.interface_name, &contract.functions, &trait_methods)
        .expect_err("mutated WIT must trigger lockstep drift");
    let expected_missing_set: BTreeSet<String> = case
        .expected_missing
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    let expected_extra_set: BTreeSet<String> = case
        .expected_extra
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(drift.missing_in_rust, expected_missing_set);
    assert_eq!(drift.extra_in_rust, expected_extra_set);
}

fn exercise_language_plugin_contract(plugin: &dyn LanguagePlugin) {
    assert!(!plugin.extensions().is_empty());
    let _ = plugin.known_dialects();
    let sample_path = RepoPath::new("nonexistent.invalid_ext").expect("valid repo path");
    let _ = plugin.lower(&sample_path, "", None);
    let _ = plugin.flagged_line_rules();
    let _ = plugin.manifest_paths(&[]);
    let _ = plugin.analyze_manifests(Vec::new());
    let _ = plugin.resolve_imports(&[], Vec::new(), Vec::new(), &[]);
    let _ = plugin.entry_points(&[], Vec::new());
    assert!(plugin.reference_ranges().is_ok());
    let _ = plugin.known_framework_profiles();
    assert!(
        plugin
            .resolve_framework_profiles("", &[], Vec::new(), FrameworkProfileSelection::Auto)
            .is_ok()
    );
    let has_hazards = plugin.has_hazard_rules();
    let rules = plugin.hazard_rules();
    if !has_hazards {
        assert!(rules.is_empty());
    }
    let _ = plugin.has_imports();
    let _ = plugin.has_scope_analysis();
    let _ = plugin.analyze_scope("", None, Vec::new());
}

fn exercise_metric_plugin_contract(plugin: &dyn MetricPlugin) {
    assert!(!plugin.key().is_empty());
    assert!(!plugin.name().is_empty());
    assert!(!plugin.description().is_empty());
    assert!(plugin.version() >= 1);
    let _ = plugin.measure(&CodeModel::default());
}

fn built_in_language_crate_count() -> usize {
    let dir = workspace_root().join("plugins/languages");
    fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", dir.display()))
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .count()
}

#[test]
fn language_wit_matches_model_trait_and_host_implementations() {
    let contract = load_verified_wit_contract("wit/language.wit", "language", "language-plugin");
    let model_source = read_workspace_file("crates/model/src/lib.rs");
    let sdk_language_source = read_workspace_file("crates/plugin-sdk/src/language.rs");
    let host_source = read_workspace_file("crates/plugin-host/src/lib.rs");
    let sql_source = read_workspace_file("crates/plugin-host/src/sql.rs");

    assert_rust_block_lockstep(
        "LanguagePlugin trait",
        &contract.functions,
        &model_source,
        "pub trait LanguagePlugin: Sync",
    );
    assert_rust_block_lockstep(
        "SDK LanguagePlugin trait",
        &contract.functions,
        &sdk_language_source,
        "pub trait LanguagePlugin",
    );
    assert_rust_block_lockstep(
        "LanguagePlugin Guest blanket impl",
        &contract.functions,
        &model_source,
        "impl<Plugin: language::Guest + Sync> LanguagePlugin for Plugin",
    );
    assert_rust_block_lockstep(
        "WasmLanguagePlugin impl",
        &contract.functions,
        &host_source,
        "impl LanguagePlugin for WasmLanguagePlugin",
    );
    assert_rust_block_lockstep(
        "BuiltInSql impl",
        &contract.functions,
        &sql_source,
        "impl LanguagePlugin for BuiltInSql",
    );

    let registered = built_in_language_plugins();
    assert_eq!(registered.len(), built_in_language_crate_count());
    for named in registered {
        exercise_language_plugin_contract(named.plugin);
    }
}

#[test]
fn metric_wit_matches_model_trait_and_host_implementation() {
    let contract = load_verified_wit_contract("wit/metric.wit", "metric", "metric-plugin");
    let model_metric_source = read_workspace_file("crates/model/src/metric.rs");
    let sdk_metric_source = read_workspace_file("crates/plugin-sdk/src/metric.rs");
    let host_source = read_workspace_file("crates/plugin-host/src/lib.rs");

    assert_rust_block_lockstep(
        "MetricPlugin trait",
        &contract.functions,
        &model_metric_source,
        "pub trait MetricPlugin",
    );
    assert_rust_block_lockstep(
        "SDK MetricPlugin trait",
        &contract.functions,
        &sdk_metric_source,
        "pub trait MetricPlugin",
    );
    assert_rust_block_lockstep(
        "WasmMetricPlugin impl",
        &contract.functions,
        &host_source,
        "impl MetricPlugin for WasmMetricPlugin",
    );

    let _witness: fn(&dyn MetricPlugin) = exercise_metric_plugin_contract;
}

#[test]
fn lockstep_verifier_fails_when_wit_adds_unimplemented_export() {
    assert_mutated_wit_drift(WitDriftCase {
        wit_path: "wit/language.wit",
        interface_name: "language",
        world_name: "language-plugin",
        target: "has-imports: func() -> bool;",
        replacement: "has-imports: func() -> bool;\n    extra-capability: func() -> bool;",
        rust_path: "crates/model/src/lib.rs",
        trait_header: "pub trait LanguagePlugin: Sync",
        expected_missing: &["extra_capability"],
        expected_extra: &[],
    });
    assert_mutated_wit_drift(WitDriftCase {
        wit_path: "wit/metric.wit",
        interface_name: "metric",
        world_name: "metric-plugin",
        target: "description: func() -> string;",
        replacement: "description: func() -> string;\n    category: func() -> string;",
        rust_path: "crates/model/src/metric.rs",
        trait_header: "pub trait MetricPlugin",
        expected_missing: &["category"],
        expected_extra: &[],
    });
}

#[test]
fn lockstep_verifier_fails_when_wit_drops_export_or_trait_adds_extra_method() {
    assert_mutated_wit_drift(WitDriftCase {
        wit_path: "wit/language.wit",
        interface_name: "language",
        world_name: "language-plugin",
        target: "has-imports: func() -> bool;",
        replacement: "",
        rust_path: "crates/model/src/lib.rs",
        trait_header: "pub trait LanguagePlugin: Sync",
        expected_missing: &[],
        expected_extra: &["has_imports"],
    });

    let metric_contract = load_verified_wit_contract("wit/metric.wit", "metric", "metric-plugin");
    let mutated_metric_trait = read_workspace_file("crates/model/src/metric.rs").replace(
        "fn description(&self) -> &str;",
        "fn description(&self) -> &str;\n    fn stale_host_only_method(&self) -> &str;",
    );
    let mutated_methods = parse_rust_block_methods(&mutated_metric_trait, "pub trait MetricPlugin")
        .expect("parse mutated metric trait");
    let extra_drift = verify_lockstep("metric", &metric_contract.functions, &mutated_methods)
        .expect_err("extra Rust trait method must fail lockstep");
    assert!(extra_drift.missing_in_rust.is_empty());
    assert_eq!(
        extra_drift.extra_in_rust,
        BTreeSet::from(["stale_host_only_method".to_owned()])
    );
}

#[test]
fn wit_parser_ignores_comments_and_nested_records_and_detects_version_drift() {
    let synthetic = r#"
        package codeint:plugin@9.9.9;
        /// fake-doc: func() -> bool;
        /* block-commented: func() -> bool; */
        interface sample {
            record nested {
                field-name: string,
            }
            real-fn: func(arg: nested) -> bool;
        }
        world sample-plugin {
            export sample;
        }
    "#;
    let parsed = parse_wit_contract(synthetic, "sample", "sample-plugin").expect("parse synthetic");
    assert_ne!(parsed.package_version, PLUGIN_INTERFACE_VERSION);
    assert_eq!(
        parsed.exported_interfaces,
        BTreeSet::from(["sample".to_owned()])
    );
    assert_eq!(parsed.functions, BTreeSet::from(["real-fn".to_owned()]));
}

#[test]
fn every_built_in_language_reference_range_matches_current_metric_version() {
    for named in built_in_language_plugins() {
        let ranges = named
            .plugin
            .reference_ranges()
            .unwrap_or_else(|error| panic!("{}: invalid reference_ranges: {error}", named.name));
        let mut count = 0usize;
        for range in ranges.iter() {
            let metric = Metric::from_key(&range.metric).unwrap_or_else(|| {
                panic!(
                    "{}: Reference range names unknown built-in Metric {:?}",
                    named.name, range.metric
                )
            });
            assert_eq!(
                range.metric_version,
                metric.version(),
                "{}: stale Reference range metric_version for {}",
                named.name,
                range.metric
            );
            count += 1;
        }
        assert!(
            count > 0,
            "{}: expected built-in Reference ranges to be checked",
            named.name
        );
    }

    let ts = built_in_language_plugin(TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME)
        .expect("built-in typescript-javascript plugin");
    let tsx_path = RepoPath::new("src/App.tsx").expect("valid repo path");
    let vue_path = RepoPath::new("src/App.vue").expect("valid repo path");
    for (label, path, dep_json) in [
        (
            "react",
            &tsx_path,
            r#"{"dependencies":{"react":"^18.3.1"}}"#,
        ),
        ("vue2", &vue_path, r#"{"dependencies":{"vue":"^2.7.16"}}"#),
        ("vue3", &vue_path, r#"{"dependencies":{"vue":"^3.4.0"}}"#),
    ] {
        let manifests = vec![Manifest {
            path: "package.json".to_owned(),
            text: dep_json.to_owned(),
        }];
        let resolved = ts
            .plugin
            .resolve_framework_profiles(
                "",
                std::slice::from_ref(path),
                manifests,
                FrameworkProfileSelection::Auto,
            )
            .unwrap_or_else(|error| panic!("{label}: resolve_framework_profiles failed: {error}"));
        assert_eq!(resolved.active_profiles, vec![label]);
        assert!(
            !resolved.reference_ranges.is_empty(),
            "{label}: expected calibrated framework Reference ranges"
        );
        for range in &resolved.reference_ranges {
            let metric = Metric::from_key(&range.metric).expect("known built-in metric");
            assert_eq!(
                range.metric_version,
                metric.version(),
                "{label}: stale framework Reference range metric_version for {}",
                range.metric
            );
        }
    }
}

fn rust_files_under(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read Rust source directory") {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            rust_files_under(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn workspace_crate_roots(root: &Path) -> Vec<PathBuf> {
    let mut crate_roots = Vec::new();
    for parent in ["crates", "plugins/languages"] {
        for entry in fs::read_dir(root.join(parent)).expect("read workspace member directory") {
            let crate_dir = entry.expect("read directory entry").path();
            for root_file in ["src/lib.rs", "src/main.rs"] {
                let candidate = crate_dir.join(root_file);
                if candidate.is_file() {
                    crate_roots.push(candidate);
                }
            }
        }
    }
    crate_roots.push(root.join("xtask/src/main.rs"));
    crate_roots.sort();
    crate_roots
}

#[test]
fn every_workspace_crate_except_plugin_sdk_forbids_unsafe_code_at_its_root() {
    let root = workspace_root();
    let sdk_root = root.join("crates/plugin-sdk/src/lib.rs");
    let crate_roots = workspace_crate_roots(&root);
    assert!(crate_roots.contains(&sdk_root));
    assert!(crate_roots.len() > built_in_language_crate_count());
    for crate_root in crate_roots.iter().filter(|path| **path != sdk_root) {
        let source = fs::read_to_string(crate_root).expect("read crate root");
        assert!(
            source.starts_with("#![forbid(unsafe_code)]"),
            "{} must start with #![forbid(unsafe_code)] per ADR 0004",
            crate_root.display()
        );
    }
}

#[test]
fn plugin_sdk_isolates_unsafe_code_strictly_to_guest_runtime_module() {
    let root = workspace_root();
    let sdk_src = root.join("crates/plugin-sdk/src");
    let runtime_module = sdk_src.join("runtime.rs");
    let crate_root = sdk_src.join("lib.rs");
    let mut sdk_files = Vec::new();
    rust_files_under(&sdk_src, &mut sdk_files);
    assert!(sdk_files.contains(&runtime_module));

    for file in &sdk_files {
        let source = fs::read_to_string(file).expect("read SDK source");
        if *file == runtime_module {
            assert!(source.contains("unsafe"));
        } else if *file == crate_root {
            assert!(
                !source.contains("unsafe"),
                "{} must contain no unsafe code per ADR 0027",
                file.display()
            );
        } else {
            assert!(
                source.starts_with("#![forbid(unsafe_code)]"),
                "{} must start with #![forbid(unsafe_code)] per ADR 0027",
                file.display()
            );
        }
    }
}
