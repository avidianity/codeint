use std::collections::BTreeMap;

use codeint_model::{RepoPath, SourceSet};
use globset::{GlobBuilder, GlobMatcher, GlobSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::codeint_json::{ConfigurationFileError, KeyPath, KeySegment, glob_set};

const WILDCARD_ZONE: &str = "*";

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
    zones: BTreeMap<String, CompiledZone>,
    rules: Vec<CompiledRule>,
    strict_zones: bool,
    source_sets: Vec<SourceSet>,
}

impl Boundaries {
    pub(crate) fn from_file(file: BoundariesFile) -> Result<Boundaries, ConfigurationFileError> {
        let boundaries_key = KeyPath::default().child(KeySegment::Key("boundaries".to_owned()));
        let zones_key = boundaries_key.child(KeySegment::Key("zones".to_owned()));
        let mut zones = BTreeMap::new();
        for (name, zone_file) in file.zones {
            let zone_key = zones_key.child(KeySegment::Key(name.clone()));
            if name.trim().is_empty() || name == WILDCARD_ZONE {
                return Err(ConfigurationFileError::InvalidBoundaryZoneName {
                    key: zone_key,
                    name,
                });
            }
            if zone_file.patterns.is_empty() {
                return Err(ConfigurationFileError::EmptyBoundaryZonePatterns {
                    key: zone_key.child(KeySegment::Key("patterns".to_owned())),
                    zone: name,
                });
            }
            let patterns_key = zone_key.child(KeySegment::Key("patterns".to_owned()));
            let patterns = compile_patterns(&patterns_key, &zone_file.patterns)?;
            let public_file_patterns = zone_file.public_files;
            let public_files = if public_file_patterns.is_empty() {
                None
            } else {
                let public_key = zone_key.child(KeySegment::Key("public_files".to_owned()));
                Some(glob_set(&public_key, &public_file_patterns)?)
            };
            zones.insert(
                name,
                CompiledZone {
                    patterns,
                    public_files,
                    public_file_patterns,
                },
            );
        }

        let known_zones: Vec<String> = zones.keys().cloned().collect();
        let rules_key = boundaries_key.child(KeySegment::Key("rules".to_owned()));
        let mut rules = Vec::with_capacity(file.rules.len());
        let mut directive_by_from: BTreeMap<String, BoundaryDirective> = BTreeMap::new();

        for (index, rule) in file.rules.into_iter().enumerate() {
            let rule_key = rules_key.child(KeySegment::Index(index));
            if rule.from != WILDCARD_ZONE && !zones.contains_key(&rule.from) {
                return Err(ConfigurationFileError::UnknownBoundaryZone {
                    key: rule_key.child(KeySegment::Key("from".to_owned())),
                    zone: rule.from,
                    known: known_zones,
                });
            }
            let (directive, targets, targets_key_name) = match (rule.allow, rule.deny) {
                (Some(allow), None) => (BoundaryDirective::Allow, allow, "allow"),
                (None, Some(deny)) => (BoundaryDirective::Deny, deny, "deny"),
                (Some(_), Some(_)) => {
                    return Err(ConfigurationFileError::ConflictingBoundaryRule {
                        key: rule_key,
                        from: rule.from,
                    });
                }
                (None, None) => {
                    return Err(ConfigurationFileError::MissingBoundaryRuleDirective {
                        key: rule_key,
                        from: rule.from,
                    });
                }
            };
            if let Some(&previous) = directive_by_from.get(&rule.from)
                && previous != directive
            {
                return Err(ConfigurationFileError::ConflictingBoundaryDirectives {
                    key: rule_key,
                    from: rule.from,
                });
            }
            directive_by_from.insert(rule.from.clone(), directive);

            let targets_key = rule_key.child(KeySegment::Key(targets_key_name.to_owned()));
            for (target_index, target) in targets.iter().enumerate() {
                if target != WILDCARD_ZONE && !zones.contains_key(target) {
                    return Err(ConfigurationFileError::UnknownBoundaryZone {
                        key: targets_key.child(KeySegment::Index(target_index)),
                        zone: target.clone(),
                        known: known_zones,
                    });
                }
            }
            rules.push(CompiledRule {
                from: rule.from,
                directive,
                targets,
                reason: rule.reason.filter(|text| !text.trim().is_empty()),
            });
        }

        let source_sets = if file.source_sets.is_empty() {
            vec![SourceSet::Production]
        } else {
            file.source_sets.into_iter().map(SourceSet::from).collect()
        };

        Ok(Boundaries {
            zones,
            rules,
            strict_zones: file.strict_zones,
            source_sets,
        })
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
