use crate::language::{FrameworkProfileSelection, Manifest};

#[cfg(test)]
mod tests;

pub fn strip_package_root<'p>(package_root: &str, full: &'p str) -> Option<&'p str> {
    if package_root.is_empty() {
        Some(full)
    } else {
        full.strip_prefix(package_root)?.strip_prefix('/')
    }
}

pub fn has_ignored_dir(path: &str, ignored_dirs: &[&str]) -> bool {
    path.split('/')
        .any(|segment| ignored_dirs.contains(&segment))
}

pub fn package_rel_path<'p>(
    package_root: &str,
    full: &'p str,
    ignored_dirs: &[&str],
) -> Option<&'p str> {
    let rel = strip_package_root(package_root, full)?;
    (!has_ignored_dir(rel, ignored_dirs)).then_some(rel)
}

pub fn find_package_manifest<'a>(
    package_root: &str,
    file_name: &str,
    manifests: &'a [Manifest],
    ignored_dirs: &[&str],
) -> Option<&'a Manifest> {
    let expected = if package_root.is_empty() {
        file_name.to_owned()
    } else {
        format!("{package_root}/{file_name}")
    };
    manifests.iter().find(|manifest| {
        manifest.path == expected && !has_ignored_dir(&manifest.path, ignored_dirs)
    })
}

pub fn matching_package_files<P: AsRef<str>>(
    package_root: &str,
    files: &[P],
    ignored_dirs: &[&str],
    predicate: impl Fn(&str) -> bool,
) -> Vec<String> {
    files
        .iter()
        .map(AsRef::as_ref)
        .filter(|full| package_rel_path(package_root, full, ignored_dirs).is_some_and(&predicate))
        .map(str::to_owned)
        .collect()
}

pub fn has_matching_package_file<P: AsRef<str>>(
    package_root: &str,
    files: &[P],
    ignored_dirs: &[&str],
    predicate: impl Fn(&str) -> bool,
) -> bool {
    files.iter().any(|full| {
        package_rel_path(package_root, full.as_ref(), ignored_dirs).is_some_and(&predicate)
    })
}

pub fn select_framework_profiles<T: Copy>(
    profiles: &[T],
    name_of: impl Fn(T) -> &'static str,
    detect: impl Fn(T) -> bool,
    selection: FrameworkProfileSelection,
    unknown_error: impl Fn(&str) -> String,
) -> Result<Vec<T>, String> {
    match selection {
        FrameworkProfileSelection::Disabled => Ok(Vec::new()),
        FrameworkProfileSelection::Auto => Ok(profiles
            .iter()
            .copied()
            .filter(|&profile| detect(profile))
            .collect()),
        FrameworkProfileSelection::Exclude(excluded) => Ok(profiles
            .iter()
            .copied()
            .filter(|&profile| {
                let name = name_of(profile);
                !excluded.iter().any(|item| item == name) && detect(profile)
            })
            .collect()),
        FrameworkProfileSelection::Only(only) => {
            for requested in &only {
                if !profiles
                    .iter()
                    .any(|&profile| name_of(profile) == requested)
                {
                    return Err(unknown_error(requested));
                }
            }
            Ok(profiles
                .iter()
                .copied()
                .filter(|&profile| {
                    let name = name_of(profile);
                    only.iter().any(|item| item == name)
                })
                .collect())
        }
    }
}
