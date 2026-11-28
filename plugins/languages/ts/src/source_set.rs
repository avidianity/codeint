use codeint_model::language::SourceSet;
use codeint_model::{has_folder_named, has_generated_header_marker_with_extra, path_file_name};

#[cfg(test)]
mod tests;

const TEST_FOLDERS: [&str; 5] = ["__tests__", "test", "tests", "test-dts", "type-tests"];
const TEST_FILE_MARKERS: [&str; 4] = ["test", "spec", "test-d", "spec-d"];
const EXTRA_GENERATED_MARKERS: [&str; 1] = ["auto generated"];

const KNOWN_CONFIG_STEMS: &[&str] = &[
    "angular",
    "astro",
    "babel",
    "biome",
    "capacitor",
    "commitlint",
    "craco",
    "cypress",
    "docusaurus",
    "drizzle",
    "electron",
    "eleventy",
    "esbuild",
    "eslint",
    "expo",
    "farm",
    "forge",
    "ionic",
    "jest",
    "karma",
    "knip",
    "lerna",
    "lint-staged",
    "metro",
    "mocha",
    "next",
    "nuxt",
    "nx",
    "oxlint",
    "playwright",
    "postcss",
    "prettier",
    "prisma",
    "release",
    "remix",
    "rollup",
    "rsbuild",
    "rspack",
    "storybook",
    "stylelint",
    "svelte",
    "tailwind",
    "tsdown",
    "tsup",
    "turbo",
    "typedoc",
    "unbuild",
    "uno",
    "vite",
    "vitest",
    "vue",
    "webpack",
    "wrangler",
];

const KNOWN_RC_STEMS: &[&str] = &[
    "babelrc",
    "commitlintrc",
    "eslintrc",
    "graphqlrc",
    "lintstagedrc",
    "mocharc",
    "nycrc",
    "parcelrc",
    "postcssrc",
    "prettierrc",
    "releaserc",
    "stylelintrc",
    "svgrrc",
    "swcrc",
];

pub(crate) fn source_set<'text>(
    path: &str,
    mut header_comments: impl Iterator<Item = &'text str>,
) -> SourceSet {
    if header_comments
        .any(|comment| has_generated_header_marker_with_extra(comment, &EXTRA_GENERATED_MARKERS))
    {
        SourceSet::Generated
    } else if has_folder_named(path, &TEST_FOLDERS) || is_test_file_name(path_file_name(path)) {
        SourceSet::Test
    } else {
        SourceSet::Production
    }
}

pub(crate) fn is_tooling_file(path: &str) -> bool {
    if has_storybook_folder(path) {
        return true;
    }
    let lower = path_file_name(path).to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    if !path.contains('/') {
        return is_dotted_config_filename(&lower) || is_leading_dot_config_filename(&lower);
    }
    is_known_nested_config_filename(&lower)
}

fn has_storybook_folder(path: &str) -> bool {
    let Some((dir, _)) = path.rsplit_once('/') else {
        return false;
    };
    dir.split('/')
        .any(|segment| segment.eq_ignore_ascii_case(".storybook"))
}

fn is_dotted_config_filename(lower: &str) -> bool {
    lower
        .find(".config.")
        .is_some_and(|index| index > 0 && index + ".config.".len() < lower.len())
}

fn is_leading_dot_config_filename(lower: &str) -> bool {
    let Some(rest) = lower.strip_prefix('.') else {
        return false;
    };
    if is_dotted_config_filename(rest) {
        return true;
    }
    rest.split('.')
        .next()
        .is_some_and(|stem| stem.ends_with("rc") && stem != "rc" && stem != "src")
}

fn is_known_nested_config_filename(lower: &str) -> bool {
    if let Some(rest) = lower.strip_prefix('.') {
        if let Some(stem) = rest.split('.').next()
            && KNOWN_RC_STEMS.contains(&stem)
        {
            return true;
        }
        return is_known_dotted_config_stem(rest);
    }
    is_known_dotted_config_stem(lower)
}

fn is_known_dotted_config_stem(lower: &str) -> bool {
    let Some(index) = lower.find(".config.") else {
        return false;
    };
    if index == 0 || index + ".config.".len() >= lower.len() {
        return false;
    }
    KNOWN_CONFIG_STEMS.contains(&&lower[..index])
}

fn is_test_file_name(file_name: &str) -> bool {
    let Some((without_extension, _extension)) = file_name.rsplit_once('.') else {
        return false;
    };
    let Some((stem, marker)) = without_extension.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty() && TEST_FILE_MARKERS.contains(&marker)
}
