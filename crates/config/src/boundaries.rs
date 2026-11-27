use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use codeint_model::{RepoPath, SourceSet};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment, glob_set};

#[cfg(test)]
mod tests;

const WILDCARD_ZONE: &str = "*";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryPreset {
    Layered,
    Hexagonal,
}

impl BoundaryPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            BoundaryPreset::Layered => "layered",
            BoundaryPreset::Hexagonal => "hexagonal",
        }
    }

    pub fn detect(paths: &[RepoPath]) -> Option<BoundaryPreset> {
        let layered = preset_match_score(BoundaryPreset::Layered, paths);
        let hexagonal = preset_match_score(BoundaryPreset::Hexagonal, paths);
        match (layered.0 >= 2, hexagonal.0 >= 2) {
            (false, false) => None,
            (true, false) => Some(BoundaryPreset::Layered),
            (false, true) => Some(BoundaryPreset::Hexagonal),
            (true, true) => {
                if hexagonal > layered {
                    Some(BoundaryPreset::Hexagonal)
                } else {
                    Some(BoundaryPreset::Layered)
                }
            }
        }
    }
}

fn preset_match_score(preset: BoundaryPreset, paths: &[RepoPath]) -> (usize, usize) {
    let boundaries = Boundaries::from_file(BoundariesFile {
        preset: Some(preset),
        ..Default::default()
    })
    .expect("built-in boundary presets compile without error");
    let mut matched_zones: BTreeSet<&str> = BTreeSet::new();
    let mut canonical_zones: BTreeSet<&str> = BTreeSet::new();
    for path in paths {
        if let Some(zone) = boundaries.zone_of(path) {
            matched_zones.insert(zone);
            if path_has_folder_segment(path, zone) {
                canonical_zones.insert(zone);
            }
        }
    }
    (matched_zones.len(), canonical_zones.len())
}

fn path_has_folder_segment(path: &RepoPath, folder: &str) -> bool {
    let mut segments = path.as_str().split('/');
    let _file = segments.next_back();
    segments.any(|segment| segment == folder)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryDirective {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryRuleDecision {
    Allowed,
    Denied {
        directive: BoundaryDirective,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryExternalPolicy {
    pub directive: BoundaryDirective,
    pub patterns: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoundarySourceSet {
    Production,
    Test,
    Generated,
    Vendored,
}

impl From<BoundarySourceSet> for SourceSet {
    fn from(value: BoundarySourceSet) -> SourceSet {
        match value {
            BoundarySourceSet::Production => SourceSet::Production,
            BoundarySourceSet::Test => SourceSet::Test,
            BoundarySourceSet::Generated => SourceSet::Generated,
            BoundarySourceSet::Vendored => SourceSet::Vendored,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BoundariesFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "BoundaryPreset",
        description = "Optional built-in architecture preset (\"layered\" or \"hexagonal\") providing default zones and rules that can be overridden or extended by zones and rules."
    )]
    preset: Option<BoundaryPreset>,
    #[serde(default)]
    #[schemars(
        description = "Named architectural zones, each defined by repository-relative path globs and optional public_files globs."
    )]
    zones: BTreeMap<String, BoundaryZoneFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Directional allow or deny import rules between architectural zones and for external packages."
    )]
    rules: Vec<BoundaryRuleFile>,
    #[serde(default, skip_serializing_if = "is_false")]
    #[schemars(
        description = "When true, report files in evaluated Source sets that do not match any configured zone."
    )]
    strict_zones: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Source sets evaluated by boundary rules; defaults to [\"production\"] when omitted or empty."
    )]
    source_sets: Vec<BoundarySourceSet>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BoundaryZoneFile {
    #[schemars(
        description = "Repository-relative path globs that assign files to this architectural zone."
    )]
    patterns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Optional repository-relative path globs naming the only files in this zone that other zones may import."
    )]
    public_files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BoundaryRuleFile {
    #[schemars(
        description = "Source zone name, or \"*\" to match any zone without a specific rule."
    )]
    from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "Vec<String>",
        description = "Target zone names (or \"*\") that the source zone is allowed to import. Mutually exclusive with deny."
    )]
    allow: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "Vec<String>",
        description = "Target zone names (or \"*\") that the source zone is forbidden to import. Mutually exclusive with allow."
    )]
    deny: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "Vec<String>",
        description = "External package name globs that the source zone is allowed to import (use [] to forbid all third-party packages while still permitting standard-library built-ins). Mutually exclusive with deny_external."
    )]
    allow_external: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "Vec<String>",
        description = "External package name globs that the source zone is forbidden to import. Mutually exclusive with allow_external."
    )]
    deny_external: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "String",
        description = "Optional human-readable explanation shown in boundary Findings."
    )]
    reason: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone)]
struct CompiledZonePattern {
    zone_name: String,
    literal_chars: usize,
    pattern_len: usize,
}

#[derive(Debug, Clone)]
struct CompiledZone {
    public_files: Option<GlobSet>,
    public_file_patterns: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    directive: BoundaryDirective,
    targets: HashSet<String>,
    has_wildcard_target: bool,
    reason: Option<String>,
}

impl CompiledRule {
    fn matches_target(&self, target_zone: &str) -> bool {
        self.has_wildcard_target || self.targets.contains(target_zone)
    }
}

#[derive(Debug, Clone)]
struct CompiledExternalRule {
    directive: BoundaryDirective,
    patterns: Vec<String>,
    matcher: GlobSet,
    reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Boundaries {
    preset: Option<BoundaryPreset>,
    resolved_zones: BTreeMap<String, BoundaryZoneFile>,
    resolved_rules: Vec<BoundaryRuleFile>,
    zones: BTreeMap<String, CompiledZone>,
    zone_matcher: GlobSet,
    zone_patterns: Vec<CompiledZonePattern>,
    rules_by_from: HashMap<String, Vec<CompiledRule>>,
    wildcard_rules: Vec<CompiledRule>,
    external_rules_by_from: HashMap<String, Vec<CompiledExternalRule>>,
    wildcard_external_rules: Vec<CompiledExternalRule>,
    strict_zones: bool,
    source_sets: Vec<SourceSet>,
}

impl Boundaries {
    pub(crate) fn from_file(file: BoundariesFile) -> Result<Boundaries, ConfigurationFileError> {
        let preset = file.preset;
        let (merged_zones, merged_rules) = merge_preset(preset, file.zones, file.rules);
        let resolved_zones = merged_zones.clone();
        let resolved_rules = merged_rules.clone();
        let boundaries_key = KeyPath::default().child(KeySegment::Key("boundaries".to_owned()));
        let compiled_zones = compile_zones(&boundaries_key, merged_zones)?;
        let (rules_by_from, wildcard_rules, external_rules_by_from, wildcard_external_rules) =
            compile_rules(&boundaries_key, &compiled_zones.zones, merged_rules)?;
        let source_sets = resolve_source_sets(file.source_sets);
        Ok(Boundaries {
            preset,
            resolved_zones,
            resolved_rules,
            zones: compiled_zones.zones,
            zone_matcher: compiled_zones.matcher,
            zone_patterns: compiled_zones.patterns,
            rules_by_from,
            wildcard_rules,
            external_rules_by_from,
            wildcard_external_rules,
            strict_zones: file.strict_zones,
            source_sets,
        })
    }

    pub(crate) fn preset(&self) -> Option<BoundaryPreset> {
        self.preset
    }

    pub(crate) fn source_set_keys(&self) -> Vec<&'static str> {
        self.source_sets.iter().map(|set| set.key()).collect()
    }

    pub(crate) fn resolved_zones_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.resolved_zones).unwrap_or(serde_json::Value::Null)
    }

    pub(crate) fn resolved_rules_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.resolved_rules).unwrap_or(serde_json::Value::Null)
    }

    pub fn applies_to_source_set(&self, source_set: SourceSet) -> bool {
        self.source_sets.contains(&source_set)
    }

    pub fn strict_zones(&self) -> bool {
        self.strict_zones
    }

    pub fn zone_names(&self) -> Vec<&str> {
        self.zones.keys().map(String::as_str).collect()
    }

    pub fn public_file_patterns(&self, zone: &str) -> &[String] {
        self.zones
            .get(zone)
            .map_or(&[], |compiled| compiled.public_file_patterns.as_slice())
    }

    pub fn allow_reason(&self, source_zone: &str, target_zone: &str) -> Option<&str> {
        if source_zone == target_zone {
            return None;
        }
        let applicable = self.applicable_rules(source_zone);
        let first = applicable.first()?;
        if first.directive != BoundaryDirective::Allow {
            return None;
        }
        applicable
            .iter()
            .find(|rule| rule.matches_target(target_zone))
            .and_then(|rule| rule.reason.as_deref())
    }

    fn applicable_rules(&self, source_zone: &str) -> &[CompiledRule] {
        self.rules_by_from
            .get(source_zone)
            .map(Vec::as_slice)
            .unwrap_or(&self.wildcard_rules)
    }

    pub fn zone_of(&self, path: &RepoPath) -> Option<&str> {
        let mut best: Option<(&str, usize, usize)> = None;
        for index in self.zone_matcher.matches(path.as_str()) {
            let pattern = &self.zone_patterns[index];
            let candidate = (
                pattern.zone_name.as_str(),
                pattern.literal_chars,
                pattern.pattern_len,
            );
            let replaces = match best {
                None => true,
                Some((best_name, best_literals, best_len)) => {
                    (candidate.1, candidate.2) > (best_literals, best_len)
                        || ((candidate.1, candidate.2) == (best_literals, best_len)
                            && candidate.0 < best_name)
                }
            };
            if replaces {
                best = Some(candidate);
            }
        }
        best.map(|(name, _, _)| name)
    }

    pub fn has_public_files(&self, zone: &str) -> bool {
        self.zones
            .get(zone)
            .is_some_and(|compiled| compiled.public_files.is_some())
    }

    pub fn is_public_file(&self, zone: &str, path: &RepoPath) -> bool {
        match self.zones.get(zone).and_then(|z| z.public_files.as_ref()) {
            Some(globs) => globs.is_match(path.as_str()),
            None => true,
        }
    }

    pub fn has_external_rules(&self) -> bool {
        !self.external_rules_by_from.is_empty() || !self.wildcard_external_rules.is_empty()
    }

    pub fn external_policy_for(&self, source_zone: &str) -> Option<BoundaryExternalPolicy> {
        let applicable = self.applicable_external_rules(source_zone);
        let first = applicable.first()?;
        let mut patterns = Vec::new();
        let mut reasons = Vec::new();
        for rule in applicable {
            for pattern in &rule.patterns {
                if !patterns.contains(pattern) {
                    patterns.push(pattern.clone());
                }
            }
            if let Some(reason) = &rule.reason
                && !reasons.contains(reason)
            {
                reasons.push(reason.clone());
            }
        }
        let reason = (!reasons.is_empty()).then(|| reasons.join("; "));
        Some(BoundaryExternalPolicy {
            directive: first.directive,
            patterns,
            reason,
        })
    }

    fn applicable_external_rules(&self, source_zone: &str) -> &[CompiledExternalRule] {
        self.external_rules_by_from
            .get(source_zone)
            .map(Vec::as_slice)
            .unwrap_or(&self.wildcard_external_rules)
    }

    pub fn check_external_import(&self, source_zone: &str, package: &str) -> BoundaryRuleDecision {
        let applicable = self.applicable_external_rules(source_zone);
        evaluate_directive_rules(
            applicable,
            |rule| rule.directive,
            |rule| rule.reason.as_deref(),
            |rule| rule.matcher.is_match(package),
        )
    }

    pub fn check_import(&self, source_zone: &str, target_zone: &str) -> BoundaryRuleDecision {
        if source_zone == target_zone {
            return BoundaryRuleDecision::Allowed;
        }
        let applicable = self.applicable_rules(source_zone);
        evaluate_directive_rules(
            applicable,
            |rule| rule.directive,
            |rule| rule.reason.as_deref(),
            |rule| rule.matches_target(target_zone),
        )
    }
}

fn evaluate_directive_rules<T>(
    applicable: &[T],
    directive_of: impl Fn(&T) -> BoundaryDirective,
    reason_of: impl Fn(&T) -> Option<&str>,
    matches_rule: impl Fn(&T) -> bool,
) -> BoundaryRuleDecision {
    let Some(first) = applicable.first() else {
        return BoundaryRuleDecision::Allowed;
    };
    match directive_of(first) {
        BoundaryDirective::Allow => {
            if applicable.iter().any(matches_rule) {
                BoundaryRuleDecision::Allowed
            } else {
                let reason = applicable
                    .iter()
                    .find_map(|rule| reason_of(rule).map(str::to_owned));
                BoundaryRuleDecision::Denied {
                    directive: BoundaryDirective::Allow,
                    reason,
                }
            }
        }
        BoundaryDirective::Deny => {
            for rule in applicable {
                if matches_rule(rule) {
                    return BoundaryRuleDecision::Denied {
                        directive: BoundaryDirective::Deny,
                        reason: reason_of(rule).map(str::to_owned),
                    };
                }
            }
            BoundaryRuleDecision::Allowed
        }
    }
}

struct CompiledZones {
    zones: BTreeMap<String, CompiledZone>,
    matcher: GlobSet,
    patterns: Vec<CompiledZonePattern>,
}

fn compile_zones(
    boundaries_key: &KeyPath,
    merged_zones: BTreeMap<String, BoundaryZoneFile>,
) -> Result<CompiledZones, ConfigurationFileError> {
    let zones_key = boundaries_key.child(KeySegment::Key("zones".to_owned()));
    let mut zones = BTreeMap::new();
    let mut matcher_builder = GlobSetBuilder::new();
    let mut patterns = Vec::new();
    for (name, zone_file) in merged_zones {
        let (compiled_name, compiled_zone, compiled_globs) =
            compile_zone(&zones_key, name, zone_file)?;
        for (glob, literal_chars, pattern_len) in compiled_globs {
            matcher_builder.add(glob);
            patterns.push(CompiledZonePattern {
                zone_name: compiled_name.clone(),
                literal_chars,
                pattern_len,
            });
        }
        zones.insert(compiled_name, compiled_zone);
    }
    let matcher =
        matcher_builder
            .build()
            .map_err(|source| ConfigurationFileError::InvalidGlob {
                file: ConfigFileKind::Json,
                key: zones_key,
                source,
            })?;
    Ok(CompiledZones {
        zones,
        matcher,
        patterns,
    })
}

type CompiledZoneGlobs = Vec<(Glob, usize, usize)>;

fn compile_zone(
    zones_key: &KeyPath,
    name: String,
    zone_file: BoundaryZoneFile,
) -> Result<(String, CompiledZone, CompiledZoneGlobs), ConfigurationFileError> {
    let zone_key = zones_key.child(KeySegment::Key(name.clone()));
    if name.trim().is_empty() || name == WILDCARD_ZONE {
        return Err(ConfigurationFileError::InvalidBoundaryZoneName {
            file: ConfigFileKind::Json,
            key: zone_key,
            name,
        });
    }
    let patterns_key = zone_key.child(KeySegment::Key("patterns".to_owned()));
    if zone_file.patterns.is_empty() {
        return Err(ConfigurationFileError::EmptyBoundaryZonePatterns {
            file: ConfigFileKind::Json,
            key: patterns_key,
            zone: name,
        });
    }
    let patterns = compile_patterns(&patterns_key, &zone_file.patterns)?;
    let public_file_patterns = zone_file.public_files;
    let public_files = if public_file_patterns.is_empty() {
        None
    } else {
        let public_key = zone_key.child(KeySegment::Key("public_files".to_owned()));
        Some(glob_set(&public_key, &public_file_patterns)?)
    };
    Ok((
        name,
        CompiledZone {
            public_files,
            public_file_patterns,
        },
        patterns,
    ))
}

type CompiledRuleIndex = (
    HashMap<String, Vec<CompiledRule>>,
    Vec<CompiledRule>,
    HashMap<String, Vec<CompiledExternalRule>>,
    Vec<CompiledExternalRule>,
);

fn compile_rules(
    boundaries_key: &KeyPath,
    zones: &BTreeMap<String, CompiledZone>,
    merged_rules: Vec<BoundaryRuleFile>,
) -> Result<CompiledRuleIndex, ConfigurationFileError> {
    let known_zones: Vec<String> = zones.keys().cloned().collect();
    let rules_key = boundaries_key.child(KeySegment::Key("rules".to_owned()));
    let mut rules_by_from: HashMap<String, Vec<CompiledRule>> = HashMap::new();
    let mut wildcard_rules = Vec::new();
    let mut external_rules_by_from: HashMap<String, Vec<CompiledExternalRule>> = HashMap::new();
    let mut wildcard_external_rules = Vec::new();
    let mut internal_by_from: BTreeMap<String, BoundaryDirective> = BTreeMap::new();
    let mut external_by_from: BTreeMap<String, BoundaryDirective> = BTreeMap::new();
    for (index, rule) in merged_rules.into_iter().enumerate() {
        let rule_key = rules_key.child(KeySegment::Index(index));
        let from = rule.from.clone();
        let (internal, external) = compile_rule(
            &rule_key,
            rule,
            zones,
            &known_zones,
            &mut internal_by_from,
            &mut external_by_from,
        )?;
        if let Some(compiled) = internal {
            if from == WILDCARD_ZONE {
                wildcard_rules.push(compiled);
            } else {
                rules_by_from
                    .entry(from.clone())
                    .or_default()
                    .push(compiled);
            }
        }
        if let Some(compiled) = external {
            if from == WILDCARD_ZONE {
                wildcard_external_rules.push(compiled);
            } else {
                external_rules_by_from
                    .entry(from)
                    .or_default()
                    .push(compiled);
            }
        }
    }
    Ok((
        rules_by_from,
        wildcard_rules,
        external_rules_by_from,
        wildcard_external_rules,
    ))
}

fn compile_rule(
    rule_key: &KeyPath,
    rule: BoundaryRuleFile,
    zones: &BTreeMap<String, CompiledZone>,
    known_zones: &[String],
    internal_by_from: &mut BTreeMap<String, BoundaryDirective>,
    external_by_from: &mut BTreeMap<String, BoundaryDirective>,
) -> Result<(Option<CompiledRule>, Option<CompiledExternalRule>), ConfigurationFileError> {
    validate_source_zone(rule_key, &rule.from, zones, known_zones)?;
    let internal = extract_directive_pair(
        rule_key,
        &rule.from,
        rule.allow,
        rule.deny,
        ("allow", "deny"),
        |key, from| ConfigurationFileError::ConflictingBoundaryRule {
            file: ConfigFileKind::Json,
            key,
            from,
        },
    )?;
    let external = extract_directive_pair(
        rule_key,
        &rule.from,
        rule.allow_external,
        rule.deny_external,
        ("allow_external", "deny_external"),
        |key, from| ConfigurationFileError::ConflictingExternalBoundaryRule {
            file: ConfigFileKind::Json,
            key,
            from,
        },
    )?;
    if internal.is_none() && external.is_none() {
        return Err(ConfigurationFileError::MissingBoundaryRuleDirective {
            file: ConfigFileKind::Json,
            key: rule_key.clone(),
            from: rule.from,
        });
    }
    let reason = rule.reason.filter(|text| !text.trim().is_empty());
    let compiled_internal = internal
        .map(|entry| {
            compile_internal_rule(
                rule_key,
                &rule.from,
                entry,
                reason.clone(),
                zones,
                known_zones,
                internal_by_from,
            )
        })
        .transpose()?;
    let compiled_external = external
        .map(|entry| compile_external_rule(rule_key, &rule.from, entry, reason, external_by_from))
        .transpose()?;
    Ok((compiled_internal, compiled_external))
}

fn validate_source_zone(
    rule_key: &KeyPath,
    from: &str,
    zones: &BTreeMap<String, CompiledZone>,
    known_zones: &[String],
) -> Result<(), ConfigurationFileError> {
    if from != WILDCARD_ZONE && !zones.contains_key(from) {
        return Err(ConfigurationFileError::UnknownBoundaryZone {
            file: ConfigFileKind::Json,
            key: rule_key.child(KeySegment::Key("from".to_owned())),
            zone: from.to_owned(),
            known: known_zones.to_vec(),
        });
    }
    Ok(())
}

fn prepare_rule_key(
    rule_key: &KeyPath,
    from: &str,
    entry: &RuleDirectiveValues,
    by_from: &mut BTreeMap<String, BoundaryDirective>,
    external: bool,
) -> Result<KeyPath, ConfigurationFileError> {
    record_rule_directive(rule_key, from, entry.directive, by_from, |key, zone| {
        if external {
            ConfigurationFileError::ConflictingExternalBoundaryDirectives {
                file: ConfigFileKind::Json,
                key,
                from: zone,
            }
        } else {
            ConfigurationFileError::ConflictingBoundaryDirectives {
                file: ConfigFileKind::Json,
                key,
                from: zone,
            }
        }
    })?;
    Ok(rule_key.child(KeySegment::Key(entry.key_name.to_owned())))
}

fn compile_internal_rule(
    rule_key: &KeyPath,
    from: &str,
    entry: RuleDirectiveValues,
    reason: Option<String>,
    zones: &BTreeMap<String, CompiledZone>,
    known_zones: &[String],
    by_from: &mut BTreeMap<String, BoundaryDirective>,
) -> Result<CompiledRule, ConfigurationFileError> {
    let targets_key = prepare_rule_key(rule_key, from, &entry, by_from, false)?;
    validate_rule_targets(&targets_key, &entry.values, zones, known_zones)?;
    let has_wildcard_target = entry.values.iter().any(|target| target == WILDCARD_ZONE);
    let targets = entry
        .values
        .into_iter()
        .filter(|target| target != WILDCARD_ZONE)
        .collect();
    Ok(CompiledRule {
        directive: entry.directive,
        targets,
        has_wildcard_target,
        reason,
    })
}

fn compile_external_rule(
    rule_key: &KeyPath,
    from: &str,
    entry: RuleDirectiveValues,
    reason: Option<String>,
    by_from: &mut BTreeMap<String, BoundaryDirective>,
) -> Result<CompiledExternalRule, ConfigurationFileError> {
    let patterns_key = prepare_rule_key(rule_key, from, &entry, by_from, true)?;
    let matcher = compile_external_globs(&patterns_key, &entry.values)?;
    Ok(CompiledExternalRule {
        directive: entry.directive,
        patterns: entry.values,
        matcher,
        reason,
    })
}

struct RuleDirectiveValues {
    directive: BoundaryDirective,
    values: Vec<String>,
    key_name: &'static str,
}

fn record_rule_directive(
    rule_key: &KeyPath,
    from: &str,
    directive: BoundaryDirective,
    by_from: &mut BTreeMap<String, BoundaryDirective>,
    conflict_error: impl FnOnce(KeyPath, String) -> ConfigurationFileError,
) -> Result<(), ConfigurationFileError> {
    if let Some(&previous) = by_from.get(from)
        && previous != directive
    {
        return Err(conflict_error(rule_key.clone(), from.to_owned()));
    }
    by_from.insert(from.to_owned(), directive);
    Ok(())
}

fn extract_directive_pair(
    rule_key: &KeyPath,
    from: &str,
    allow: Option<Vec<String>>,
    deny: Option<Vec<String>>,
    key_names: (&'static str, &'static str),
    conflict_error: impl FnOnce(KeyPath, String) -> ConfigurationFileError,
) -> Result<Option<RuleDirectiveValues>, ConfigurationFileError> {
    match (allow, deny) {
        (Some(values), None) => Ok(Some(RuleDirectiveValues {
            directive: BoundaryDirective::Allow,
            values,
            key_name: key_names.0,
        })),
        (None, Some(values)) => Ok(Some(RuleDirectiveValues {
            directive: BoundaryDirective::Deny,
            values,
            key_name: key_names.1,
        })),
        (Some(_), Some(_)) => Err(conflict_error(rule_key.clone(), from.to_owned())),
        (None, None) => Ok(None),
    }
}

fn compile_external_globs(
    key: &KeyPath,
    patterns: &[String],
) -> Result<GlobSet, ConfigurationFileError> {
    let mut builder = GlobSetBuilder::new();
    for (index, pattern) in patterns.iter().enumerate() {
        let pattern_key = key.child(KeySegment::Index(index));
        if pattern.trim().is_empty() {
            return Err(ConfigurationFileError::EmptyBoundaryExternalPattern {
                file: ConfigFileKind::Json,
                key: pattern_key,
            });
        }
        let compiled = GlobBuilder::new(pattern)
            .literal_separator(false)
            .backslash_escape(true)
            .build()
            .map_err(|source| ConfigurationFileError::InvalidGlob {
                file: ConfigFileKind::Json,
                key: pattern_key,
                source,
            })?;
        builder.add(compiled);
    }
    builder
        .build()
        .map_err(|source| ConfigurationFileError::InvalidGlob {
            file: ConfigFileKind::Json,
            key: key.clone(),
            source,
        })
}

fn validate_rule_targets(
    targets_key: &KeyPath,
    targets: &[String],
    zones: &BTreeMap<String, CompiledZone>,
    known_zones: &[String],
) -> Result<(), ConfigurationFileError> {
    for (target_index, target) in targets.iter().enumerate() {
        if target != WILDCARD_ZONE && !zones.contains_key(target) {
            return Err(ConfigurationFileError::UnknownBoundaryZone {
                file: ConfigFileKind::Json,
                key: targets_key.child(KeySegment::Index(target_index)),
                zone: target.clone(),
                known: known_zones.to_vec(),
            });
        }
    }
    Ok(())
}

fn resolve_source_sets(source_sets: Vec<BoundarySourceSet>) -> Vec<SourceSet> {
    if source_sets.is_empty() {
        vec![SourceSet::Production]
    } else {
        source_sets.into_iter().map(SourceSet::from).collect()
    }
}

fn compile_patterns(
    key: &KeyPath,
    globs: &[String],
) -> Result<CompiledZoneGlobs, ConfigurationFileError> {
    globs
        .iter()
        .enumerate()
        .map(|(index, glob)| {
            let compiled = GlobBuilder::new(glob)
                .literal_separator(true)
                .backslash_escape(true)
                .build()
                .map_err(|source| ConfigurationFileError::InvalidGlob {
                    file: ConfigFileKind::Json,
                    key: key.child(KeySegment::Index(index)),
                    source,
                })?;
            Ok((compiled, literal_char_count(glob), glob.len()))
        })
        .collect()
}

fn literal_char_count(pattern: &str) -> usize {
    pattern
        .chars()
        .filter(|character| !matches!(character, '*' | '?' | '[' | ']' | '{' | '}'))
        .count()
}

fn merge_preset(
    preset: Option<BoundaryPreset>,
    user_zones: BTreeMap<String, BoundaryZoneFile>,
    user_rules: Vec<BoundaryRuleFile>,
) -> (BTreeMap<String, BoundaryZoneFile>, Vec<BoundaryRuleFile>) {
    let Some(preset) = preset else {
        return (user_zones, user_rules);
    };
    let (mut zones, preset_rules) = preset_defaults(preset);
    for (name, zone) in user_zones {
        zones.insert(name, zone);
    }
    let overridden_from: BTreeSet<String> = user_rules
        .iter()
        .filter(|rule| rule.allow.is_some() || rule.deny.is_some())
        .map(|rule| rule.from.clone())
        .collect();
    let mut rules: Vec<BoundaryRuleFile> = preset_rules
        .into_iter()
        .filter(|rule| !overridden_from.contains(&rule.from))
        .collect();
    rules.extend(user_rules);
    (zones, rules)
}

fn preset_defaults(
    preset: BoundaryPreset,
) -> (BTreeMap<String, BoundaryZoneFile>, Vec<BoundaryRuleFile>) {
    match preset {
        BoundaryPreset::Layered => layered_preset(),
        BoundaryPreset::Hexagonal => hexagonal_preset(),
    }
}

fn zone_patterns(patterns: &[&str]) -> BoundaryZoneFile {
    BoundaryZoneFile {
        patterns: patterns
            .iter()
            .map(|pattern| (*pattern).to_owned())
            .collect(),
        public_files: Vec::new(),
    }
}

fn allow_rule(from: &str, allow: &[&str], reason: &str) -> BoundaryRuleFile {
    BoundaryRuleFile {
        from: from.to_owned(),
        allow: Some(allow.iter().map(|target| (*target).to_owned()).collect()),
        deny: None,
        allow_external: None,
        deny_external: None,
        reason: Some(reason.to_owned()),
    }
}

fn layered_preset() -> (BTreeMap<String, BoundaryZoneFile>, Vec<BoundaryRuleFile>) {
    let mut zones = BTreeMap::new();
    zones.insert(
        "presentation".to_owned(),
        zone_patterns(&[
            "**/presentation/**",
            "**/ui/**",
            "**/web/**",
            "**/api/**",
            "**/controllers/**",
            "**/routes/**",
        ]),
    );
    zones.insert(
        "application".to_owned(),
        zone_patterns(&[
            "**/application/**",
            "**/app/**",
            "**/services/**",
            "**/usecases/**",
            "**/use-cases/**",
            "**/use_cases/**",
        ]),
    );
    zones.insert(
        "domain".to_owned(),
        zone_patterns(&[
            "**/domain/**",
            "**/core/**",
            "**/entities/**",
            "**/models/**",
        ]),
    );
    zones.insert(
        "infrastructure".to_owned(),
        zone_patterns(&[
            "**/infrastructure/**",
            "**/infra/**",
            "**/adapters/**",
            "**/repositories/**",
            "**/db/**",
            "**/persistence/**",
        ]),
    );
    let rules = vec![
        allow_rule(
            "domain",
            &["domain"],
            "Layered preset: domain layer must not depend on outer layers",
        ),
        allow_rule(
            "application",
            &["domain"],
            "Layered preset: application layer may only depend on domain",
        ),
        allow_rule(
            "infrastructure",
            &["domain", "application"],
            "Layered preset: infrastructure layer may only depend on domain and application",
        ),
        allow_rule(
            "presentation",
            &["application", "domain"],
            "Layered preset: presentation layer must not depend directly on infrastructure",
        ),
    ];
    (zones, rules)
}

fn hexagonal_preset() -> (BTreeMap<String, BoundaryZoneFile>, Vec<BoundaryRuleFile>) {
    let mut zones = BTreeMap::new();
    zones.insert(
        "domain".to_owned(),
        zone_patterns(&["**/domain/**", "**/core/**", "**/entities/**"]),
    );
    zones.insert("ports".to_owned(), zone_patterns(&["**/ports/**"]));
    zones.insert(
        "application".to_owned(),
        zone_patterns(&[
            "**/application/**",
            "**/app/**",
            "**/usecases/**",
            "**/use-cases/**",
            "**/use_cases/**",
        ]),
    );
    zones.insert(
        "adapters".to_owned(),
        zone_patterns(&[
            "**/adapters/**",
            "**/infrastructure/**",
            "**/infra/**",
            "**/driving/**",
            "**/driven/**",
            "**/inbound/**",
            "**/outbound/**",
        ]),
    );
    let rules = vec![
        allow_rule(
            "domain",
            &["domain"],
            "Hexagonal preset: domain must stay independent of ports, application, and adapters",
        ),
        allow_rule(
            "ports",
            &["domain"],
            "Hexagonal preset: ports may only depend on domain",
        ),
        allow_rule(
            "application",
            &["domain", "ports"],
            "Hexagonal preset: application may only depend on domain and ports, not adapters",
        ),
        allow_rule(
            "adapters",
            &["domain", "ports", "application"],
            "Hexagonal preset: adapters may depend on domain, ports, and application",
        ),
    ];
    (zones, rules)
}
