use std::process::Command;

pub(crate) const HELP_COMMAND: &str = "help";

pub(crate) struct HelpEntry {
    pub(crate) spelling: String,
    pub(crate) possible_values: Vec<String>,
}

pub(crate) struct CommandHelp {
    pub(crate) subcommand: Option<String>,
    pub(crate) entries: Vec<HelpEntry>,
}

impl CommandHelp {
    pub(crate) fn heading(&self) -> String {
        match &self.subcommand {
            Some(name) => format!("### `codeint {name}`"),
            None => "### `codeint`".to_owned(),
        }
    }

    pub(crate) fn takes_value(&self, argument: &str) -> bool {
        self.entries.iter().any(|entry| {
            entry.spelling.starts_with("--")
                && entry.spelling.contains(" <")
                && entry.spelling.split(' ').next() == Some(argument)
        })
    }
}

pub(crate) fn every_command_help() -> Vec<CommandHelp> {
    let top = command_help(None);
    let subcommands: Vec<String> = top
        .entries
        .iter()
        .filter(|entry| !entry.spelling.starts_with('-'))
        .map(|entry| entry.spelling.clone())
        .filter(|name| name != HELP_COMMAND)
        .collect();
    let mut every = vec![top];
    every.extend(subcommands.into_iter().map(|name| command_help(Some(name))));
    every
}

fn command_help(subcommand: Option<String>) -> CommandHelp {
    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(subcommand.iter())
        .arg("--help")
        .output()
        .expect("run codeint --help");
    assert!(
        output.status.success(),
        "codeint {subcommand:?} --help failed"
    );
    let text = String::from_utf8(output.stdout).expect("help is UTF-8");
    let mut entries = Vec::new();
    let mut in_list = false;
    for line in text.lines() {
        if matches!(line, "Commands:" | "Arguments:" | "Options:") {
            in_list = true;
        } else if line.is_empty() || !line.starts_with(' ') {
            in_list = false;
        } else if in_list {
            entries.push(help_entry(line.trim_start()));
        }
    }
    CommandHelp {
        subcommand,
        entries,
    }
}

fn help_entry(line: &str) -> HelpEntry {
    let (spelling, description) = line.split_once("  ").unwrap_or((line, ""));
    let possible_values = description
        .split_once("[possible values: ")
        .and_then(|(_, rest)| rest.split_once(']'))
        .map(|(values, _)| values.split(", ").map(str::to_owned).collect())
        .unwrap_or_default();
    HelpEntry {
        spelling: spelling.to_owned(),
        possible_values,
    }
}
