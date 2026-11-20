use codeint_model::language::{Manifest, WorkspaceError};
use globset::{Glob, GlobMatcher};

use super::PACKAGE_MANIFEST;

pub(crate) const PNPM_WORKSPACE_MANIFEST: &str = "pnpm-workspace.yaml";

const FOLDERS_PNPM_NEVER_SEARCHES: [&str; 2] = ["node_modules", "bower_components"];

pub(super) type ReadPnpmPackagePatterns = fn(&str) -> Result<Vec<String>, String>;

pub(super) fn pnpm_package_patterns(_pnpm_workspace_manifest: &str) -> Result<Vec<String>, String> {
    todo!()
}

pub(super) fn pnpm_workspace_packages(
    manifests: &[Manifest],
    patterns: &[String],
) -> Result<Vec<String>, WorkspaceError> {
    let patterns = patterns
        .iter()
        .filter_map(|written| PnpmPattern::parse(written).transpose())
        .collect::<Result<Vec<PnpmPattern<'_>>, WorkspaceError>>()?;
    let (excluded, included): (Vec<PnpmPattern<'_>>, Vec<PnpmPattern<'_>>) =
        patterns.into_iter().partition(|pattern| pattern.excludes);
    Ok(manifests
        .iter()
        .filter_map(|manifest| manifest.path.strip_suffix(&format!("/{PACKAGE_MANIFEST}")))
        .filter(|folder| {
            let segments: Vec<&str> = folder.split('/').collect();
            !segments
                .iter()
                .any(|segment| FOLDERS_PNPM_NEVER_SEARCHES.contains(segment))
                && included
                    .iter()
                    .any(|pattern| pattern.matches(&segments, Wildcards::SkipDotNames))
                && !excluded
                    .iter()
                    .any(|pattern| pattern.matches(&segments, Wildcards::MatchDotNames))
        })
        .map(str::to_owned)
        .collect())
}

struct PnpmPattern<'manifest> {
    segments: Vec<Segment<'manifest>>,
    excludes: bool,
}

enum Segment<'manifest> {
    AnyDepth,
    Name {
        written: &'manifest str,
        matcher: GlobMatcher,
    },
}

#[derive(Clone, Copy)]
enum Wildcards {
    SkipDotNames,
    MatchDotNames,
}

impl<'manifest> PnpmPattern<'manifest> {
    fn parse(written: &'manifest str) -> Result<Option<PnpmPattern<'manifest>>, WorkspaceError> {
        let (excludes, folders) = match written.strip_prefix('!') {
            Some(folders) => (true, folders),
            None => (false, written),
        };
        if folders.starts_with('/') {
            return Ok(None);
        }
        let segments = normalized_segments(folders)
            .into_iter()
            .map(|segment| Segment::parse(written, segment))
            .collect::<Result<Vec<Segment<'manifest>>, WorkspaceError>>()?;
        Ok(Some(PnpmPattern { segments, excludes }))
    }

    fn matches(&self, folder: &[&str], wildcards: Wildcards) -> bool {
        segments_match(&self.segments, folder, wildcards)
    }
}

impl<'manifest> Segment<'manifest> {
    fn parse(pattern: &str, written: &'manifest str) -> Result<Segment<'manifest>, WorkspaceError> {
        if written == "**" {
            return Ok(Segment::AnyDepth);
        }
        let glob = Glob::new(written).map_err(|error| WorkspaceError {
            path: PNPM_WORKSPACE_MANIFEST.to_owned(),
            message: format!(
                "has the package pattern {pattern:?}, which is not a valid glob: {error}"
            ),
        })?;
        Ok(Segment::Name {
            written,
            matcher: glob.compile_matcher(),
        })
    }

    fn matches(&self, name: &str, wildcards: Wildcards) -> bool {
        match self {
            Segment::AnyDepth => wildcards.may_match(name),
            Segment::Name { written, matcher } => {
                (written.starts_with('.') || wildcards.may_match(name)) && matcher.is_match(name)
            }
        }
    }
}

impl Wildcards {
    fn may_match(self, name: &str) -> bool {
        match self {
            Wildcards::SkipDotNames => !name.starts_with('.'),
            Wildcards::MatchDotNames => true,
        }
    }
}

fn segments_match(segments: &[Segment<'_>], folder: &[&str], wildcards: Wildcards) -> bool {
    match segments.split_first() {
        None => folder.is_empty(),
        Some((any_depth @ Segment::AnyDepth, rest)) => {
            segments_match(rest, folder, wildcards)
                || folder.split_first().is_some_and(|(name, below)| {
                    any_depth.matches(name, wildcards) && segments_match(segments, below, wildcards)
                })
        }
        Some((segment, rest)) => folder.split_first().is_some_and(|(name, below)| {
            segment.matches(name, wildcards) && segments_match(rest, below, wildcards)
        }),
    }
}

fn normalized_segments(folders: &str) -> Vec<&str> {
    let mut segments: Vec<&str> = Vec::new();
    for segment in folders.split('/') {
        match segment {
            "" | "." => {}
            ".." if segments.last().is_some_and(|last| *last != "..") => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }
    segments
}
