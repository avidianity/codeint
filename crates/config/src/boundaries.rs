use std::collections::{BTreeMap, BTreeSet};

use codeint_model::{RepoPath, SourceSet};
use globset::{GlobBuilder, GlobMatcher, GlobSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment, glob_set};

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
        description = "Directional allow or deny import rules between architectural zones."
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
        with = "String",
        description = "Optional human-readable explanation shown in boundary Findings."
    )]
    reason: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone)]
struct CompiledPattern {
    matcher: GlobMatcher,
    literal_chars: usize,
    pattern_len: usize,
}

#[derive(Debug, Clone)]
struct CompiledZone {
    patterns: Vec<CompiledPattern>,
    public_files: Option<GlobSet>,
    public_file_patterns: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    from: String,
    directive: BoundaryDirective,
    targets: Vec<String>,
    reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Boundaries {
    preset: Option<BoundaryPreset>,
    resolved_zones: BTreeMap<String, BoundaryZoneFile>,
    resolved_rules: Vec<BoundaryRuleFile>,
    zones: BTreeMap<String, CompiledZone>,
    rules: Vec<CompiledRule>,
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
        let zones = compile_zones(&boundaries_key, merged_zones)?;
        let rules = compile_rules(&boundaries_key, &zones, merged_rules)?;
        let source_sets = resolve_source_sets(file.source_sets);
        Ok(Boundaries {
            preset,
            resolved_zones,
            resolved_rules,
            zones,
            rules,
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
            .find(|rule| {
                rule.targets
                    .iter()
                    .any(|target| target == target_zone || target == WILDCARD_ZONE)
            })
            .and_then(|rule| rule.reason.as_deref())
    }

    fn applicable_rules(&self, source_zone: &str) -> Vec<&CompiledRule> {
        let specific: Vec<&CompiledRule> = self
            .rules
            .iter()
            .filter(|rule| rule.from == source_zone)
            .collect();
        if specific.is_empty() {
            self.rules
                .iter()
                .filter(|rule| rule.from == WILDCARD_ZONE)
                .collect()
        } else {
            specific
        }
    }

    pub fn zone_of(&self, path: &RepoPath) -> Option<&str> {
        let mut best: Option<(&str, usize, usize)> = None;
        for (zone_name, zone) in &self.zones {
            for pattern in &zone.patterns {
                if pattern.matcher.is_match(path.as_str()) {
                    let candidate = (
                        zone_name.as_str(),
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

    pub fn check_import(&self, source_zone: &str, target_zone: &str) -> BoundaryRuleDecision {
        if source_zone == target_zone {
            return BoundaryRuleDecision::Allowed;
        }
        let applicable = self.applicable_rules(source_zone);
        let Some(first) = applicable.first() else {
            return BoundaryRuleDecision::Allowed;
        };
        match first.directive {
            BoundaryDirective::Allow => {
                let permitted = applicable.iter().any(|rule| {
                    rule.targets
                        .iter()
                        .any(|target| target == target_zone || target == WILDCARD_ZONE)
                });
                if permitted {
                    BoundaryRuleDecision::Allowed
                } else {
                    let reason = applicable.iter().find_map(|rule| rule.reason.clone());
                    BoundaryRuleDecision::Denied {
                        directive: BoundaryDirective::Allow,
                        reason,
                    }
                }
            }
            BoundaryDirective::Deny => {
                let denied_by = applicable.iter().find(|rule| {
                    rule.targets
                        .iter()
                        .any(|target| target == target_zone || target == WILDCARD_ZONE)
                });
                match denied_by {
                    Some(rule) => BoundaryRuleDecision::Denied {
                        directive: BoundaryDirective::Deny,
                        reason: rule.reason.clone(),
                    },
                    None => BoundaryRuleDecision::Allowed,
                }
            }
        }
    }
}

fn compile_zones(
    boundaries_key: &KeyPath,
    merged_zones: BTreeMap<String, BoundaryZoneFile>,
) -> Result<BTreeMap<String, CompiledZone>, ConfigurationFileError> {
    let zones_key = boundaries_key.child(KeySegment::Key("zones".to_owned()));
    let mut zones = BTreeMap::new();
    for (name, zone_file) in merged_zones {
        let (compiled_name, compiled_zone) = compile_zone(&zones_key, name, zone_file)?;
        zones.insert(compiled_name, compiled_zone);
    }
    Ok(zones)
}

fn compile_zone(
    zones_key: &KeyPath,
    name: String,
    zone_file: BoundaryZoneFile,
) -> Result<(String, CompiledZone), ConfigurationFileError> {
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
            patterns,
            public_files,
            public_file_patterns,
        },
    ))
}

fn compile_rules(
    boundaries_key: &KeyPath,
    zones: &BTreeMap<String, CompiledZone>,
    merged_rules: Vec<BoundaryRuleFile>,
) -> Result<Vec<CompiledRule>, ConfigurationFileError> {
    let known_zones: Vec<String> = zones.keys().cloned().collect();
    let rules_key = boundaries_key.child(KeySegment::Key("rules".to_owned()));
    let mut rules = Vec::with_capacity(merged_rules.len());
    let mut directive_by_from: BTreeMap<String, BoundaryDirective> = BTreeMap::new();
    for (index, rule) in merged_rules.into_iter().enumerate() {
        let rule_key = rules_key.child(KeySegment::Index(index));
        rules.push(compile_rule(
            &rule_key,
            rule,
            zones,
            &known_zones,
            &mut directive_by_from,
        )?);
    }
    Ok(rules)
}

fn compile_rule(
    rule_key: &KeyPath,
    rule: BoundaryRuleFile,
    zones: &BTreeMap<String, CompiledZone>,
    known_zones: &[String],
    directive_by_from: &mut BTreeMap<String, BoundaryDirective>,
) -> Result<CompiledRule, ConfigurationFileError> {
    if rule.from != WILDCARD_ZONE && !zones.contains_key(&rule.from) {
        return Err(ConfigurationFileError::UnknownBoundaryZone {
            file: ConfigFileKind::Json,
            key: rule_key.child(KeySegment::Key("from".to_owned())),
            zone: rule.from,
            known: known_zones.to_vec(),
        });
    }
    let (directive, targets, targets_key_name) =
        extract_rule_directive(rule_key, &rule.from, rule.allow, rule.deny)?;
    if let Some(&previous) = directive_by_from.get(&rule.from)
        && previous != directive
    {
        return Err(ConfigurationFileError::ConflictingBoundaryDirectives {
            file: ConfigFileKind::Json,
            key: rule_key.clone(),
            from: rule.from,
        });
    }
    directive_by_from.insert(rule.from.clone(), directive);
    let targets_key = rule_key.child(KeySegment::Key(targets_key_name.to_owned()));
    validate_rule_targets(&targets_key, &targets, zones, known_zones)?;
    Ok(CompiledRule {
        from: rule.from,
        directive,
        targets,
        reason: rule.reason.filter(|text| !text.trim().is_empty()),
    })
}

fn extract_rule_directive(
    rule_key: &KeyPath,
    from: &str,
    allow: Option<Vec<String>>,
    deny: Option<Vec<String>>,
) -> Result<(BoundaryDirective, Vec<String>, &'static str), ConfigurationFileError> {
    match (allow, deny) {
        (Some(allow), None) => Ok((BoundaryDirective::Allow, allow, "allow")),
        (None, Some(deny)) => Ok((BoundaryDirective::Deny, deny, "deny")),
        (Some(_), Some(_)) => Err(ConfigurationFileError::ConflictingBoundaryRule {
            file: ConfigFileKind::Json,
            key: rule_key.clone(),
            from: from.to_owned(),
        }),
        (None, None) => Err(ConfigurationFileError::MissingBoundaryRuleDirective {
            file: ConfigFileKind::Json,
            key: rule_key.clone(),
            from: from.to_owned(),
        }),
    }
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
) -> Result<Vec<CompiledPattern>, ConfigurationFileError> {
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
            Ok(CompiledPattern {
                matcher: compiled.compile_matcher(),
                literal_chars: literal_char_count(glob),
                pattern_len: glob.len(),
            })
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
    let overridden_from: BTreeSet<String> =
        user_rules.iter().map(|rule| rule.from.clone()).collect();
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
