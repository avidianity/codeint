use std::ops::Range;

use crate::markdown::{FencedBlock, Page};

pub(crate) const EXAMPLE_LANGUAGE: &str = "sh";
const CONFIGURATION_ATTRIBUTE: &str = "codeint.json";
const ILLUSTRATION_ATTRIBUTE: &str = "illustration";

pub(crate) struct Example {
    pub(crate) line: usize,
    pub(crate) commands: Vec<CommandLine>,
    pub(crate) exit_code: i32,
    pub(crate) configuration: Option<String>,
    pub(crate) outputs: Vec<ExpectedOutput>,
}

pub(crate) struct CommandLine {
    pub(crate) text: String,
    pub(crate) environment: Vec<(String, String)>,
    pub(crate) arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OutputSource {
    StandardOutput,
    StandardError,
    File(String),
}

pub(crate) struct ExpectedOutput {
    pub(crate) source: OutputSource,
    pub(crate) text: String,
    pub(crate) content_lines: Range<usize>,
}

pub(crate) struct ExampleSet {
    pub(crate) examples: Vec<Example>,
    pub(crate) configuration_files: Vec<String>,
}

enum BlockRole {
    Example { exit_code: i32 },
    Output(OutputSource),
    Configuration,
    Illustration,
}

fn role(block: &FencedBlock) -> Result<BlockRole, String> {
    let line = block.content_lines.start;
    let attribute = match block.attributes.as_slice() {
        [] => {
            return Err(format!(
                "line {line}: a fenced block needs one attribute after its language: exit={{code}}, stdout, stderr, file={{path}}, {CONFIGURATION_ATTRIBUTE}, or {ILLUSTRATION_ATTRIBUTE}"
            ));
        }
        [attribute] => attribute.as_str(),
        _ => {
            return Err(format!(
                "line {line}: a fenced block takes one attribute, not {:?}",
                block.attributes
            ));
        }
    };
    if let Some(code) = attribute.strip_prefix("exit=") {
        if block.language != EXAMPLE_LANGUAGE {
            return Err(format!(
                "line {line}: an example is a {EXAMPLE_LANGUAGE} block"
            ));
        }
        return code
            .parse()
            .map(|exit_code| BlockRole::Example { exit_code })
            .map_err(|_| format!("line {line}: exit={code} is not an exit code"));
    }
    if block.language == EXAMPLE_LANGUAGE {
        return Err(format!(
            "line {line}: every {EXAMPLE_LANGUAGE} block is an example and needs exit={{code}}"
        ));
    }
    match attribute {
        "stdout" => Ok(BlockRole::Output(OutputSource::StandardOutput)),
        "stderr" => Ok(BlockRole::Output(OutputSource::StandardError)),
        CONFIGURATION_ATTRIBUTE => Ok(BlockRole::Configuration),
        ILLUSTRATION_ATTRIBUTE => Ok(BlockRole::Illustration),
        _ => attribute
            .strip_prefix("file=")
            .map(|path| BlockRole::Output(OutputSource::File(path.to_owned())))
            .ok_or_else(|| format!("line {line}: unknown attribute {attribute:?}")),
    }
}

fn command_line(text: &str, line: usize) -> Result<CommandLine, String> {
    if text.contains(['"', '\'', '\\', '|', '>', '<', '$', '`']) {
        return Err(format!(
            "line {line}: write an example command without quotes, pipes, redirects, or variables: {text}"
        ));
    }
    let mut environment = Vec::new();
    let mut words = text.split_whitespace();
    for word in words.by_ref() {
        if word == "codeint" {
            return Ok(CommandLine {
                text: text.to_owned(),
                environment,
                arguments: words.map(str::to_owned).collect(),
            });
        }
        match word.split_once('=') {
            Some((name, value)) => environment.push((name.to_owned(), value.to_owned())),
            None => break,
        }
    }
    Err(format!(
        "line {line}: an example command runs codeint: {text}"
    ))
}

pub(crate) fn examples(page: &Page) -> Result<ExampleSet, Vec<String>> {
    let mut errors = Vec::new();
    let mut examples: Vec<Example> = Vec::new();
    let mut configuration_files = Vec::new();
    let mut configuration: Option<(Option<usize>, String)> = None;
    let mut previous_accepts_output = false;
    for block in page.fenced_blocks() {
        let line = block.content_lines.start;
        let configuration_of_this_section = configuration
            .as_ref()
            .filter(|(heading, _)| *heading == block.heading_line)
            .map(|(_, text)| text.clone());
        let accepts_output = match role(&block) {
            Err(error) => {
                errors.push(error);
                false
            }
            Ok(BlockRole::Illustration) => false,
            Ok(BlockRole::Configuration) => {
                configuration_files.push(block.content.clone());
                configuration = Some((block.heading_line, block.content.clone()));
                false
            }
            Ok(BlockRole::Example { exit_code }) => {
                let commands: Vec<CommandLine> = block
                    .content
                    .lines()
                    .filter_map(|text| {
                        command_line(text, line)
                            .map_err(|error| errors.push(error))
                            .ok()
                    })
                    .collect();
                if commands.is_empty() {
                    errors.push(format!(
                        "line {line}: an example holds at least one command"
                    ));
                }
                examples.push(Example {
                    line,
                    commands,
                    exit_code,
                    configuration: configuration_of_this_section,
                    outputs: Vec::new(),
                });
                true
            }
            Ok(BlockRole::Output(source)) => {
                match examples.last_mut() {
                    Some(example) if previous_accepts_output && example.commands.len() == 1 => {
                        if let OutputSource::File(path) = &source
                            && path.as_str() == CONFIGURATION_ATTRIBUTE
                        {
                            configuration_files.push(block.content.clone());
                        }
                        example.outputs.push(ExpectedOutput {
                            source,
                            text: block.content.clone(),
                            content_lines: block.content_lines.clone(),
                        });
                    }
                    _ => errors.push(format!(
                        "line {line}: an output block follows an example of one command, or another output block of it"
                    )),
                }
                true
            }
        };
        previous_accepts_output = accepts_output;
    }
    if errors.is_empty() {
        Ok(ExampleSet {
            examples,
            configuration_files,
        })
    } else {
        Err(errors)
    }
}
