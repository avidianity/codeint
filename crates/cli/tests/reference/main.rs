mod examples;
mod fixture;
mod help;
mod keys;
mod markdown;
#[path = "../wasm_fixtures/mod.rs"]
mod wasm_fixtures;

use std::collections::{BTreeMap, BTreeSet};

use rayon::prelude::*;

use examples::{CommandLine, Example, ExampleSet, OutputSource};
use fixture::Template;
use help::{CommandHelp, HelpEntry, every_command_help};
use markdown::{Page, code_spans, reference_path, table_rows};
use serde_json::Value;

const CONFIGURATION_HEADING: &str = "## `codeint.json`";
const EXIT_CODES_HEADING: &str = "## Exit codes";
const LANGUAGES_HEADING: &str = "### Languages";
const HELP_TABLE_HEADERS: [&str; 3] = ["Command", "Argument", "Option"];

fn example_set(page: &Page) -> ExampleSet {
    examples::examples(page).unwrap_or_else(|errors| {
        panic!(
            "docs/reference.md breaks the example conventions of docs/agents/documentation.md:\n{}",
            errors.join("\n")
        )
    })
}

fn first_code_span(cell: &str) -> String {
    code_spans(cell).into_iter().next().unwrap_or_default()
}

fn documented_help_rows(page: &Page, help: &CommandHelp) -> BTreeMap<String, String> {
    let section = page.section(&help.heading());
    HELP_TABLE_HEADERS
        .iter()
        .flat_map(|header| table_rows(section, header))
        .map(|row| (first_code_span(&row[0]), row.join(" | ")))
        .collect()
}

#[test]
fn the_reference_lists_every_command_argument_and_option_that_help_prints_and_no_other() {
    let page = Page::read();
    let mut problems = Vec::new();
    for help in every_command_help() {
        let documented = documented_help_rows(&page, &help);
        let printed: BTreeSet<&str> = help
            .entries
            .iter()
            .map(|entry| entry.spelling.as_str())
            .collect();
        for spelling in &printed {
            if !documented.contains_key(*spelling) {
                problems.push(format!(
                    "{}: add a row for `{spelling}`, which --help prints",
                    help.heading()
                ));
            }
        }
        for spelling in documented.keys() {
            if !printed.contains(spelling.as_str()) {
                problems.push(format!(
                    "{}: remove the row for `{spelling}`, which --help does not print",
                    help.heading()
                ));
            }
        }
        for entry in &help.entries {
            let Some(row) = documented.get(&entry.spelling) else {
                continue;
            };
            let shown = code_spans(row);
            for value in &entry.possible_values {
                if !shown.contains(value) {
                    problems.push(format!(
                        "{}: the row for `{}` does not name the value `{value}`",
                        help.heading(),
                        entry.spelling
                    ));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "docs/reference.md is stale:\n{}",
        problems.join("\n")
    );
}

fn placeholders(page: &Page) -> BTreeMap<String, Vec<String>> {
    table_rows(page.section(CONFIGURATION_HEADING), "Placeholder")
        .iter()
        .map(|row| (first_code_span(&row[0]), code_spans(&row[1])))
        .collect()
}

fn key_patterns(page: &Page) -> Vec<String> {
    table_rows(page.section(CONFIGURATION_HEADING), "Key")
        .iter()
        .map(|row| first_code_span(&row[0]))
        .collect()
}

#[test]
fn the_reference_lists_every_key_of_the_configuration_schema_and_no_other() {
    let page = Page::read();
    let placeholders = placeholders(&page);
    let documented: BTreeSet<String> = key_patterns(&page)
        .iter()
        .flat_map(|pattern| keys::expand(pattern, &placeholders))
        .collect();
    let schema = keys::schema_key_paths();
    let missing: Vec<&String> = schema.difference(&documented).collect();
    let unknown: Vec<&String> = documented.difference(&schema).collect();
    assert!(
        missing.is_empty() && unknown.is_empty(),
        "docs/reference.md is stale: the schema has keys the reference does not list: {missing:?}; the reference lists keys the schema does not have: {unknown:?}"
    );
}

struct Mismatch {
    example_line: usize,
    source: OutputSource,
    content_lines: std::ops::Range<usize>,
    actual: String,
}

fn run_example_command(
    template: &Template,
    example: &Example,
    command: &CommandLine,
) -> (Vec<String>, Vec<Mismatch>) {
    let mut problems = Vec::new();
    let mut mismatches = Vec::new();
    let run = template.run(example.configuration.as_deref(), command);
    let code = run.output.status.code();
    if code != Some(example.exit_code) {
        problems.push(format!(
            "line {}: `{}` exits with {code:?}, not {}; standard error: {}",
            example.line,
            command.text,
            example.exit_code,
            String::from_utf8_lossy(&run.output.stderr)
        ));
    }
    for expected in &example.outputs {
        let bytes = match &expected.source {
            OutputSource::StandardOutput => run.output.stdout.clone(),
            OutputSource::StandardError => run.output.stderr.clone(),
            OutputSource::File(path) => std::fs::read(run.root().join(path)).unwrap_or_default(),
        };
        let actual = run.shown(&String::from_utf8_lossy(&bytes));
        if actual != expected.text {
            mismatches.push(Mismatch {
                example_line: example.line,
                source: expected.source.clone(),
                content_lines: expected.content_lines.clone(),
                actual,
            });
        }
    }
    (problems, mismatches)
}

#[test]
fn every_example_in_the_reference_runs_as_written_against_the_fixture() {
    let page = Page::read();
    let set = example_set(&page);
    let template = Template::build();
    let mut problems = Vec::new();
    let mut mismatches = Vec::new();
    let jobs: Vec<(&Example, &CommandLine)> = set
        .examples
        .iter()
        .flat_map(|example| {
            example
                .commands
                .iter()
                .map(move |command| (example, command))
        })
        .collect();
    let outcomes: Vec<(Vec<String>, Vec<Mismatch>)> = jobs
        .par_iter()
        .map(|&(example, command)| run_example_command(&template, example, command))
        .collect();
    for (local_problems, local_mismatches) in outcomes {
        problems.extend(local_problems);
        mismatches.extend(local_mismatches);
    }
    if !mismatches.is_empty() {
        let replacements: Vec<_> = mismatches
            .iter()
            .map(|mismatch| (mismatch.content_lines.clone(), mismatch.actual.clone()))
            .collect();
        std::fs::write(reference_path(), page.with_replaced_contents(&replacements))
            .expect("rewrite docs/reference.md");
        problems.extend(mismatches.iter().map(|mismatch| {
            format!(
                "line {}: the {:?} of the example differs from what codeint prints; docs/reference.md now holds the printed output, so review it, commit it, and run the tests again",
                mismatch.example_line, mismatch.source
            )
        }));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

fn invocations_of<'a>(set: &'a ExampleSet, help: &CommandHelp) -> Vec<&'a [String]> {
    set.examples
        .iter()
        .flat_map(|example| &example.commands)
        .map(|command: &CommandLine| command.arguments.as_slice())
        .filter_map(|arguments| match &help.subcommand {
            Some(name) => arguments
                .split_first()
                .filter(|(first, _)| *first == name)
                .map(|(_, rest)| rest),
            None => Some(arguments),
        })
        .collect()
}

fn has_positional(help: &CommandHelp, arguments: &[String]) -> bool {
    let mut previous_takes_value = false;
    for argument in arguments {
        if !argument.starts_with('-') && !previous_takes_value {
            return true;
        }
        previous_takes_value = help.takes_value(argument);
    }
    false
}

fn entry_is_used(help: &CommandHelp, entry: &HelpEntry, invocations: &[&[String]]) -> Vec<String> {
    let spelling = entry.spelling.as_str();
    if spelling.starts_with('[') || spelling.starts_with('<') {
        let used = invocations
            .iter()
            .any(|arguments| has_positional(help, arguments));
        return if used {
            Vec::new()
        } else {
            vec![format!("an example that passes {spelling}")]
        };
    }
    if !spelling.starts_with('-') {
        let used = invocations
            .iter()
            .any(|arguments| arguments.first().map(String::as_str) == Some(spelling));
        return if used {
            Vec::new()
        } else {
            vec![format!("an example of codeint {spelling}")]
        };
    }
    let flags: Vec<&str> = spelling
        .split(", ")
        .filter_map(|flag| flag.split(' ').next())
        .collect();
    let mut missing = Vec::new();
    if !invocations.iter().any(|arguments| {
        arguments
            .iter()
            .any(|argument| flags.contains(&argument.as_str()))
    }) {
        missing.push(format!("an example that passes {spelling}"));
    }
    for value in &entry.possible_values {
        let used = invocations.iter().any(|arguments| {
            arguments
                .windows(2)
                .any(|pair| flags.contains(&pair[0].as_str()) && pair[1] == *value)
        });
        if !used {
            missing.push(format!("an example that passes {} {value}", flags[0]));
        }
    }
    missing
}

#[test]
fn the_examples_use_every_command_option_format_and_configuration_key() {
    let page = Page::read();
    let set = example_set(&page);
    let mut missing = Vec::new();
    for help in every_command_help() {
        let invocations = invocations_of(&set, &help);
        for entry in &help.entries {
            let help_of_a_subcommand = help.subcommand.is_some() && entry.spelling == "-h, --help";
            if help_of_a_subcommand {
                continue;
            }
            for need in entry_is_used(&help, entry, &invocations) {
                missing.push(format!("{}: {need}", help.heading()));
            }
        }
    }
    let shown_keys: BTreeSet<String> = set
        .configuration_files
        .iter()
        .flat_map(|text| {
            let stripped: String = text
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            let configuration: Value = serde_json::from_str(&stripped).unwrap_or_else(|error| {
                panic!("a codeint.json example is not JSON: {error}\n{text}")
            });
            keys::configuration_key_paths(&configuration)
        })
        .collect();
    let placeholders = placeholders(&page);
    for pattern in key_patterns(&page) {
        if !keys::expand(&pattern, &placeholders)
            .iter()
            .any(|path| shown_keys.contains(path))
        {
            missing.push(format!(
                "{CONFIGURATION_HEADING}: a codeint.json example that sets `{pattern}`"
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "docs/reference.md needs:\n{}",
        missing.join("\n")
    );
}

#[test]
fn the_examples_show_every_exit_code_that_the_reference_lists_and_no_other() {
    let page = Page::read();
    let set = example_set(&page);
    let listed: BTreeSet<i32> = table_rows(page.section(EXIT_CODES_HEADING), "Exit code")
        .iter()
        .map(|row| {
            first_code_span(&row[0])
                .parse()
                .expect("an exit code is a number")
        })
        .collect();
    let shown: BTreeSet<i32> = set
        .examples
        .iter()
        .map(|example| example.exit_code)
        .collect();
    assert_eq!(
        shown, listed,
        "the examples of docs/reference.md show exit codes {shown:?}, and its exit code table lists {listed:?}"
    );
}

const COUNT_WORDS: [&str; 10] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
];

fn count_word(count: usize) -> Option<&'static str> {
    count
        .checked_sub(1)
        .and_then(|index| COUNT_WORDS.get(index).copied())
}

#[test]
fn the_reference_lists_every_built_in_language_plugin_and_extension() {
    let page = Page::read();
    let section = page.section(LANGUAGES_HEADING);
    let documented: BTreeMap<String, BTreeSet<String>> = table_rows(section, "Plugin name")
        .iter()
        .map(|row| {
            let extensions = code_spans(&row[1]).into_iter().collect();
            (first_code_span(&row[0]), extensions)
        })
        .collect();
    let schema = serde_json::to_value(codeint_cli::schema()).expect("serialize schema");
    let expected: BTreeMap<String, BTreeSet<String>> = schema["languages"]
        .as_array()
        .expect("schema languages array")
        .iter()
        .map(|entry| {
            let name = entry["name"].as_str().expect("language name").to_owned();
            let extensions = entry["extensions"]
                .as_array()
                .expect("language extensions")
                .iter()
                .map(|ext| format!(".{}", ext.as_str().expect("extension string")))
                .collect();
            (name, extensions)
        })
        .collect();
    assert_eq!(
        documented, expected,
        "docs/reference.md ### Languages table drifted from built-in Language plugins"
    );
    let expected_sentence = match count_word(expected.len()) {
        Some(word) => format!("codeint ships {word} built-in Language plugins:"),
        None => format!(
            "codeint ships {} built-in Language plugins:",
            expected.len()
        ),
    };
    assert!(
        section.iter().any(|line| line.contains(&expected_sentence)),
        "docs/reference.md ### Languages section must contain {expected_sentence:?}"
    );
}
