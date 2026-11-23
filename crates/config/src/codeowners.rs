use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub enum CodeownersError {
    #[error("cannot read {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid owner filter pattern {pattern:?}")]
    InvalidOwnerPattern {
        pattern: String,
        #[source]
        source: globset::Error,
    },
}

#[derive(Debug, Clone, Default)]
pub struct Codeowners {
    rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
struct Rule {
    matchers: Vec<GlobMatcher>,
    owners: Vec<String>,
}

impl Codeowners {
    pub const STANDARD_PATHS: [&'static str; 3] =
        [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"];

    pub fn load(repository_root: &Path) -> Result<Option<Codeowners>, CodeownersError> {
        for relative in Self::STANDARD_PATHS {
            let path = repository_root.join(relative);
            if path.is_file() {
                let text = std::fs::read_to_string(&path)
                    .map_err(|source| CodeownersError::Read { path, source })?;
                return Ok(Some(Codeowners::parse(&text)));
            }
        }
        Ok(None)
    }

    pub fn parse(text: &str) -> Codeowners {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let rules = text.lines().filter_map(parse_rule).collect();
        Codeowners { rules }
    }

    pub fn owners_for(&self, path: &str) -> Vec<String> {
        let normalized = path.trim_start_matches('/');
        self.rules
            .iter()
            .rev()
            .find(|rule| rule.matches(normalized))
            .map(|rule| rule.owners.clone())
            .unwrap_or_default()
    }

    pub fn owners_for_many<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let mut combined = Vec::new();
        for path in paths {
            for owner in self.owners_for(path) {
                if !combined.contains(&owner) {
                    combined.push(owner);
                }
            }
        }
        combined
    }
}

impl Rule {
    fn matches(&self, path: &str) -> bool {
        self.matchers.iter().any(|matcher| matcher.is_match(path))
    }
}

#[derive(Debug, Clone)]
pub struct OwnerFilter {
    matcher: GlobMatcher,
}

impl OwnerFilter {
    pub fn new(pattern: &str) -> Result<OwnerFilter, CodeownersError> {
        let matcher = GlobBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .map_err(|source| CodeownersError::InvalidOwnerPattern {
                pattern: pattern.to_owned(),
                source,
            })?
            .compile_matcher();
        Ok(OwnerFilter { matcher })
    }

    pub fn matches(&self, owner: &str) -> bool {
        self.matcher.is_match(owner)
    }

    pub fn matches_any(&self, owners: &[String]) -> bool {
        owners.iter().any(|owner| self.matches(owner))
    }
}

fn parse_rule(line: &str) -> Option<Rule> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (pattern, rest) = take_pattern(trimmed);
    if pattern.is_empty() {
        return None;
    }
    let matchers = compile_pattern(&pattern)?;
    let owners = parse_owners(rest);
    Some(Rule { matchers, owners })
}

fn take_pattern(line: &str) -> (String, &str) {
    let mut pattern = String::new();
    let mut chars = line.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '\\' {
            if let Some(&(_, next)) = chars.peek() {
                if matches!(next, '\\' | '*' | '?' | '[' | ']' | '{' | '}') {
                    pattern.push('\\');
                }
                pattern.push(next);
                chars.next();
            }
            continue;
        }
        if ch.is_whitespace() {
            return (pattern, &line[index..]);
        }
        pattern.push(ch);
    }
    (pattern, "")
}

fn parse_owners(rest: &str) -> Vec<String> {
    rest.split_whitespace()
        .take_while(|token| !token.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn compile_pattern(pattern: &str) -> Option<Vec<GlobMatcher>> {
    let (has_leading_slash, without_leading) = match pattern.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, pattern),
    };
    let (has_trailing_slash, core) = match without_leading.strip_suffix('/') {
        Some(rest) => (true, rest),
        None => (false, without_leading),
    };
    if core.is_empty() {
        return None;
    }
    let anchored = has_leading_slash || core.contains('/');
    let prefix = if anchored { "" } else { "**/" };
    let mut patterns = Vec::new();
    if has_trailing_slash {
        patterns.push(format!("{prefix}{core}/**"));
    } else {
        patterns.push(format!("{prefix}{core}"));
        if !last_segment_has_wildcard(core) {
            patterns.push(format!("{prefix}{core}/**"));
        }
    }
    let mut matchers = Vec::with_capacity(patterns.len());
    for glob in patterns {
        let matcher = GlobBuilder::new(&glob)
            .literal_separator(true)
            .backslash_escape(true)
            .build()
            .ok()?
            .compile_matcher();
        matchers.push(matcher);
    }
    Some(matchers)
}

fn last_segment_has_wildcard(pattern: &str) -> bool {
    let last = pattern.rsplit('/').next().unwrap_or(pattern);
    let mut chars = last.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            chars.next();
            continue;
        }
        if matches!(ch, '*' | '?' | '[') {
            return true;
        }
    }
    false
}
