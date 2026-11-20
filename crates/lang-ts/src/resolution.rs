use std::collections::HashSet;

use codeint_model::language::{ImportRequest, Resolution};

const PROBED_EXTENSIONS: [&str; 8] = ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"];

const TYPESCRIPT_SOURCES_OF_JAVASCRIPT_OUTPUT: [(&str, &[&str]); 4] = [
    ("js", &["ts", "tsx"]),
    ("jsx", &["tsx"]),
    ("mjs", &["mts"]),
    ("cjs", &["cts"]),
];

pub(crate) struct RelativeResolver<'paths> {
    repository_paths: HashSet<&'paths str>,
}

impl<'paths> RelativeResolver<'paths> {
    pub(crate) fn new(repository_paths: &'paths [String]) -> RelativeResolver<'paths> {
        RelativeResolver {
            repository_paths: repository_paths.iter().map(String::as_str).collect(),
        }
    }

    pub(crate) fn resolve(&self, request: &ImportRequest) -> Resolution {
        let specifier = request.specifier.as_str();
        if specifier.starts_with('/') {
            return Resolution::Unresolved(format!(
                "{specifier:?} is an absolute path, which depends on the machine"
            ));
        }
        if !is_relative(specifier) {
            return Resolution::External;
        }
        let Some(base) = join(directory_of(&request.importer), specifier) else {
            return Resolution::Unresolved(format!("{specifier:?} leads outside the repository"));
        };
        candidates(&base)
            .into_iter()
            .find(|candidate| self.repository_paths.contains(candidate.as_str()))
            .map_or_else(
                || {
                    Resolution::Unresolved(format!(
                        "no file of the repository matches {specifier:?}"
                    ))
                },
                Resolution::Resolved,
            )
    }
}

fn is_relative(specifier: &str) -> bool {
    matches!(specifier, "." | "..") || specifier.starts_with("./") || specifier.starts_with("../")
}

fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(directory, _)| directory)
}

fn join(directory: &str, specifier: &str) -> Option<String> {
    let mut segments: Vec<&str> = directory
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    for segment in specifier.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            name => segments.push(name),
        }
    }
    Some(segments.join("/"))
}

fn candidates(base: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut index_prefix = String::new();
    if !base.is_empty() {
        candidates.push(base.to_owned());
        if let Some((stem, extension)) = base.rsplit_once('.') {
            for (javascript, typescript) in TYPESCRIPT_SOURCES_OF_JAVASCRIPT_OUTPUT {
                if extension == javascript {
                    candidates.extend(typescript.iter().map(|source| format!("{stem}.{source}")));
                }
            }
        }
        candidates.extend(
            PROBED_EXTENSIONS
                .iter()
                .map(|extension| format!("{base}.{extension}")),
        );
        index_prefix = format!("{base}/");
    }
    candidates.extend(
        PROBED_EXTENSIONS
            .iter()
            .map(|extension| format!("{index_prefix}index.{extension}")),
    );
    candidates
}
