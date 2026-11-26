use std::collections::{BTreeMap, BTreeSet};

use clap::{Arg, Command as ClapCommand, CommandFactory};
use serde::Serialize;

use crate::cli::Arguments;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliSchema {
    pub name: String,
    pub version: String,
    pub description: String,
    pub commands: Vec<CliCommandSchema>,
    pub output_formats: Vec<OutputFormatSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliCommandSchema {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub arguments: Vec<CliArgumentSchema>,
    pub flags: Vec<CliFlagSchema>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subcommands: Vec<CliCommandSchema>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliArgumentSchema {
    pub name: String,
    pub value_name: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CliFlagSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    pub takes_value: bool,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub possible_values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutputFormatSchema {
    pub name: String,
    pub commands: Vec<String>,
}

pub fn cli_schema() -> CliSchema {
    let root = Arguments::command();
    let commands: Vec<CliCommandSchema> = root
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set() && sub.get_name() != "help")
        .map(describe_command)
        .collect();
    let output_formats = collect_output_formats(&commands);
    CliSchema {
        name: root.get_name().to_owned(),
        version: root
            .get_version()
            .unwrap_or(env!("CARGO_PKG_VERSION"))
            .to_owned(),
        description: root
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default(),
        commands,
        output_formats,
    }
}

fn describe_command(command: &ClapCommand) -> CliCommandSchema {
    let mut arguments = Vec::new();
    let mut flags = Vec::new();
    for arg in command.get_arguments().filter(|arg| !arg.is_hide_set()) {
        if arg.is_positional() {
            arguments.push(describe_argument(arg));
        } else {
            flags.push(describe_flag(arg));
        }
    }
    let subcommands = command
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set() && sub.get_name() != "help")
        .map(describe_command)
        .collect();
    CliCommandSchema {
        name: command.get_name().to_owned(),
        description: command.get_about().map(ToString::to_string),
        arguments,
        flags,
        subcommands,
    }
}

fn describe_argument(arg: &Arg) -> CliArgumentSchema {
    let name = arg.get_id().as_str().to_owned();
    let value_name = first_value_name(arg).unwrap_or_else(|| name.to_ascii_uppercase());
    CliArgumentSchema {
        name,
        value_name,
        required: arg.is_required_set(),
        description: arg.get_help().map(ToString::to_string),
    }
}

fn describe_flag(arg: &Arg) -> CliFlagSchema {
    let takes_value = arg.get_action().takes_values();
    let value_name = takes_value.then(|| first_value_name(arg)).flatten();
    let default_value = takes_value
        .then(|| {
            arg.get_default_values()
                .first()
                .and_then(|value| value.to_str())
                .map(str::to_owned)
        })
        .flatten();
    let possible_values = if takes_value {
        arg.get_possible_values()
            .into_iter()
            .filter(|value| !value.is_hide_set())
            .map(|value| value.get_name().to_owned())
            .collect()
    } else {
        Vec::new()
    };
    CliFlagSchema {
        short: arg.get_short().map(|short| format!("-{short}")),
        long: arg.get_long().map(|long| format!("--{long}")),
        value_name,
        takes_value,
        required: arg.is_required_set(),
        default_value,
        possible_values,
        description: arg.get_help().map(ToString::to_string),
    }
}

fn first_value_name(arg: &Arg) -> Option<String> {
    arg.get_value_names()
        .and_then(|names| names.first())
        .map(|name| name.as_str().to_owned())
}

fn collect_output_formats(commands: &[CliCommandSchema]) -> Vec<OutputFormatSchema> {
    let mut by_format: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    record_command_formats(commands, "", &mut by_format);
    by_format
        .into_iter()
        .map(|(name, commands)| OutputFormatSchema {
            name,
            commands: commands.into_iter().collect(),
        })
        .collect()
}

fn record_command_formats(
    commands: &[CliCommandSchema],
    prefix: &str,
    by_format: &mut BTreeMap<String, BTreeSet<String>>,
) {
    for command in commands {
        let full_name = if prefix.is_empty() {
            command.name.clone()
        } else {
            format!("{prefix} {}", command.name)
        };
        for flag in &command.flags {
            if flag.long.as_deref() == Some("--format") && full_name != "init" {
                for format in &flag.possible_values {
                    by_format
                        .entry(format.clone())
                        .or_default()
                        .insert(full_name.clone());
                }
            }
        }
        record_command_formats(&command.subcommands, &full_name, by_format);
    }
}
