use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use codeint_config::{
    CalendarDay, ChurnFormat, ChurnSince, ChurnWindow, CouplingRatio, CouplingSettings,
    FindingsShown, GuardFormat, ImpactFormat, PluginHostConfig, ReportFormat, TraceFormat,
    TrendFormat, TrendRange,
};
use codeint_report::McpToolSchema;
use serde_json::{Value, json};

use crate::{
    CheckOptions, ChurnOptions, CouplingOptions, DiffOptions, ImpactOptions, TraceOptions,
    TrendOptions, check, churn, coupling, diff, guard, impact, trace, trend,
};

const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

pub(crate) fn serve_stdio(
    default_folder: &Path,
    result_cache: Option<PathBuf>,
    plugin_host: PluginHostConfig,
) -> Result<(), std::io::Error> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    serve(
        &mut reader,
        &mut writer,
        default_folder,
        result_cache,
        plugin_host,
    )
}

fn serve<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    default_folder: &Path,
    result_cache: Option<PathBuf>,
    plugin_host: PluginHostConfig,
) -> Result<(), std::io::Error> {
    let ctx = ServerContext {
        default_folder,
        result_cache,
        plugin_host,
    };
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_line(trimmed, &ctx) {
            writeln!(writer, "{response}")?;
            writer.flush()?;
        }
    }
    Ok(())
}

struct ServerContext<'a> {
    default_folder: &'a Path,
    result_cache: Option<PathBuf>,
    plugin_host: PluginHostConfig,
}

fn handle_line(line: &str, ctx: &ServerContext<'_>) -> Option<Value> {
    let request: Value = match serde_json::from_str(line) {
        Ok(val) => val,
        Err(err) => {
            return Some(rpc_error(
                Value::Null,
                -32700,
                &format!("Parse error: {err}"),
            ));
        }
    };
    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = request.get("params");
    let response_id = id?;
    let result = dispatch(method, params, ctx);
    Some(match result {
        Ok(value) => json!({ "jsonrpc": "2.0", "id": response_id, "result": value }),
        Err((code, message)) => rpc_error(response_id, code, &message),
    })
}

fn dispatch(
    method: &str,
    params: Option<&Value>,
    ctx: &ServerContext<'_>,
) -> Result<Value, (i64, String)> {
    match method {
        "initialize" => Ok(handle_initialize(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(handle_tools_list()),
        "tools/call" => handle_tools_call(params, ctx),
        other => Err((-32601, format!("Method not found: {other}"))),
    }
}

fn handle_initialize(params: Option<&Value>) -> Value {
    let protocol_version = params
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "codeint",
            "version": env!("CARGO_PKG_VERSION"),
        }
    })
}

pub(crate) fn tool_definitions() -> Vec<McpToolSchema> {
    vec![
        McpToolSchema {
            name: "check".to_owned(),
            description: "Measure Software entropy of a Git repository working tree (Erosion, Cognitive complexity, Duplication, Verbosity, Import cycles, Boundaries, Orphan files, Dependency hygiene) and report any Budget failures.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "package": { "type": "string", "description": "Optional Workspace package path to scope measurement and output." },
                    "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." }
                }
            }),
        },
        McpToolSchema {
            name: "diff".to_owned(),
            description: "Compare the working tree (or a head Git ref) against the merge-base of base_ref and classify Findings as new, resolved, or unchanged.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["base_ref"],
                "properties": {
                    "base_ref": { "type": "string", "description": "Base Git ref such as main or origin/main." },
                    "head_ref": { "type": "string", "description": "Optional head Git ref instead of the working tree." },
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "package": { "type": "string", "description": "Optional Workspace package path to scope measurement and output." },
                    "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." }
                }
            }),
        },
        McpToolSchema {
            name: "trend".to_owned(),
            description: "Measure each main-line commit over time (latest 20 commits or since YYYY-MM-DD).".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "since": { "type": "string", "description": "UTC date YYYY-MM-DD to start the trend from." }
                }
            }),
        },
        McpToolSchema {
            name: "guard".to_owned(),
            description: "Query the architectural boundary zone, allowed and denied target zones, public_files constraints, and allowed importers for an existing or planned file path.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["file"],
                "properties": {
                    "file": { "type": "string", "description": "Repository-relative or absolute path to the existing or planned file." },
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." }
                }
            }),
        },
        McpToolSchema {
            name: "impact".to_owned(),
            description: "Compute the transitive reverse-dependency blast radius of one or more repository files (or files changed since a base Git ref), separating Production and Test dependents and listing affected Workspace packages.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "paths": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Repository-relative file paths to analyze."
                    },
                    "changed": { "type": "string", "description": "Optional base Git ref; analyze files changed since its merge-base with HEAD." },
                    "depth": { "type": "integer", "minimum": 0, "description": "Optional maximum reverse-dependency hop distance to traverse." },
                    "package": { "type": "string", "description": "Optional Workspace package path to scope reported dependents." }
                }
            }),
        },
        McpToolSchema {
            name: "trace".to_owned(),
            description: "Find the shortest runtime import path(s) connecting two repository files.".to_owned(),
            input_schema: json!({
                "type": "object",
                "required": ["from", "to"],
                "properties": {
                    "from": { "type": "string", "description": "Repository-relative path of the starting module." },
                    "to": { "type": "string", "description": "Repository-relative path of the target module." },
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "package": { "type": "string", "description": "Optional Workspace package path to scope the query." },
                    "all": { "type": "boolean", "description": "Return all shortest paths up to max_paths instead of a single shortest path." },
                    "max_paths": { "type": "integer", "minimum": 1, "description": "Maximum number of shortest paths to return when all is true (default 10)." }
                }
            }),
        },
        McpToolSchema {
            name: "churn".to_owned(),
            description: "Rank eroded and duplicated files alongside their Git commit churn independently per Metric over a configurable lookback window.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "since": { "type": "string", "description": "Lookback window in days (such as 90d) or UTC date (YYYY-MM-DD); mutually exclusive with commits." },
                    "commits": { "type": "integer", "minimum": 1, "description": "Walk at most this many main-line commits from HEAD; mutually exclusive with since." },
                    "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." }
                }
            }),
        },
        McpToolSchema {
            name: "coupling".to_owned(),
            description: "Detect pairs of files that repeatedly change together in the same commits, excluding bulk commits and flagging direct runtime imports.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "since": { "type": "string", "description": "Lookback window in days (such as 90d) or UTC date (YYYY-MM-DD); mutually exclusive with commits." },
                    "commits": { "type": "integer", "minimum": 1, "description": "Walk at most this many main-line commits from HEAD; mutually exclusive with since." },
                    "max_commit_files": { "type": "integer", "minimum": 1, "description": "Exclude commits touching more than this many files (defaults to 50)." },
                    "min_co_changes": { "type": "integer", "minimum": 1, "description": "Minimum co-changing commits required to report a pair (defaults to 3)." },
                    "min_ratio": { "type": "number", "minimum": 0, "maximum": 1, "description": "Minimum coupling ratio between 0 and 1 required to report a pair (defaults to 0.3)." },
                    "include_tests": { "type": "boolean", "description": "Include Test Source set files alongside Production files (defaults to false)." },
                    "all": { "type": "boolean", "description": "Return all coupled pairs instead of the top 20." }
                }
            }),
        },
    ]
}

fn handle_tools_list() -> Value {
    json!({
        "tools": tool_definitions(),
    })
}

fn handle_tools_call(
    params: Option<&Value>,
    ctx: &ServerContext<'_>,
) -> Result<Value, (i64, String)> {
    let name = params
        .and_then(|p| p.get("name"))
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "Missing tool name".to_owned()))?;
    let args = match parse_tool_arguments(params) {
        Ok(args) => args,
        Err(err) => return Ok(err),
    };
    dispatch_tool(name, args, ctx)
}

fn parse_tool_arguments(params: Option<&Value>) -> Result<Option<&Value>, Value> {
    match params.and_then(|p| p.get("arguments")) {
        None => Ok(None),
        Some(value) if value.is_object() => Ok(Some(value)),
        Some(_) => Err(tool_err("Tool arguments must be a JSON object")),
    }
}

fn dispatch_tool(
    name: &str,
    args: Option<&Value>,
    ctx: &ServerContext<'_>,
) -> Result<Value, (i64, String)> {
    let result = match name {
        "check" => call_check(args, ctx),
        "diff" => call_diff(args, ctx),
        "trend" => call_trend(args, ctx),
        "guard" => call_guard(args, ctx),
        "impact" => call_impact(args, ctx),
        "trace" => call_trace(args, ctx),
        "churn" => call_churn(args, ctx),
        "coupling" => call_coupling(args, ctx),
        other => return Err((-32602, format!("Unknown tool: {other}"))),
    };
    Ok(result.unwrap_or_else(|err| err))
}

fn call_check(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let folder = resolve_folder(args, ctx.default_folder)?;
    let options = CheckOptions {
        findings: parse_findings(args)?,
        package: parse_package(args),
        owner: None,
        plugin_host: ctx.plugin_host.clone(),
    };
    Ok(match check(&folder, &options) {
        Ok(outcome) => render_outcome(
            outcome.passed,
            outcome.failures,
            outcome.report.render(ReportFormat::Json),
        ),
        Err(err) => tool_err(&err.to_string()),
    })
}

fn call_diff(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let base_ref = required_str_arg(args, "base_ref")?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    let options = DiffOptions {
        head_ref: args
            .and_then(|a| a.get("head_ref"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        findings: parse_findings(args)?,
        package: parse_package(args),
        owner: None,
        result_cache: ctx.result_cache.clone(),
        plugin_host: ctx.plugin_host.clone(),
    };
    Ok(match diff(&folder, base_ref, &options) {
        Ok(outcome) => render_outcome(
            outcome.passed,
            outcome.failures,
            outcome.report.render(ReportFormat::Json),
        ),
        Err(err) => tool_err(&err.to_string()),
    })
}

fn render_outcome(
    passed: bool,
    failures: Vec<String>,
    rendered: Result<String, codeint_report::ReportError>,
) -> Value {
    match rendered {
        Ok(text) => {
            let report: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            tool_ok(json!({
                "passed": passed,
                "failures": failures,
                "report": report,
            }))
        }
        Err(err) => tool_err(&err.to_string()),
    }
}

fn render_json_tool<T, E: std::fmt::Display>(
    result: Result<T, E>,
    render: impl FnOnce(T) -> Result<String, codeint_report::ReportError>,
) -> Result<Value, Value> {
    Ok(match result {
        Ok(report) => match render(report) {
            Ok(rendered) => {
                let payload: Value = serde_json::from_str(&rendered).unwrap_or(Value::Null);
                tool_ok(payload)
            }
            Err(err) => tool_err(&err.to_string()),
        },
        Err(err) => tool_err(&err.to_string()),
    })
}

fn required_str_arg<'a>(args: Option<&'a Value>, name: &str) -> Result<&'a str, Value> {
    args.and_then(|a| a.get(name))
        .and_then(Value::as_str)
        .ok_or_else(|| tool_err(&format!("Missing required argument: {name}")))
}

fn call_guard(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let file = required_str_arg(args, "file")?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    render_json_tool(guard(&folder, Path::new(file)), |report| {
        report.render(GuardFormat::Json)
    })
}

fn call_trend(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let folder = resolve_folder(args, ctx.default_folder)?;
    let range = match args.and_then(|a| a.get("since")).and_then(Value::as_str) {
        Some(day_str) => match CalendarDay::from_str(day_str) {
            Ok(day) => TrendRange::Since(day),
            Err(err) => return Err(tool_err(&format!("Invalid since date {day_str:?}: {err}"))),
        },
        None => TrendOptions::default().range,
    };
    let options = TrendOptions {
        range,
        result_cache: ctx.result_cache.clone(),
        plugin_host: ctx.plugin_host.clone(),
    };
    render_json_tool(trend(&folder, &options), |report| {
        report.render(TrendFormat::Json)
    })
}

fn call_impact(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let folder = resolve_folder(args, ctx.default_folder)?;
    let paths = parse_string_array(args, "paths")?;
    let changed = args
        .and_then(|a| a.get("changed"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let depth = match args.and_then(|a| a.get("depth")) {
        Some(value) => Some(parse_usize_arg(
            value,
            "Invalid depth argument: expected a non-negative integer",
        )?),
        None => None,
    };
    let options = ImpactOptions {
        paths,
        changed,
        depth,
        package: parse_package(args),
        plugin_host: ctx.plugin_host.clone(),
    };
    render_json_tool(impact(&folder, &options), |report| {
        report.render(ImpactFormat::Json)
    })
}

fn call_trace(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let from = required_str_arg(args, "from")?;
    let to = required_str_arg(args, "to")?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    let max_paths = match args.and_then(|a| a.get("max_paths")) {
        Some(value) => parse_usize_arg(
            value,
            "Invalid argument: max_paths must be a positive integer",
        )?,
        None => TraceOptions::default().max_paths,
    };
    let options = TraceOptions {
        all: args
            .and_then(|a| a.get("all"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        max_paths,
        package: parse_package(args),
        plugin_host: ctx.plugin_host.clone(),
    };
    render_json_tool(trace(&folder, from, to, &options), |report| {
        report.render(TraceFormat::Json)
    })
}

fn call_churn(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let options = ChurnOptions {
        window: parse_churn_window(args)?,
        findings: parse_findings(args)?,
        plugin_host: ctx.plugin_host.clone(),
    };
    let folder = resolve_folder(args, ctx.default_folder)?;
    render_json_tool(churn(&folder, &options), |report| {
        report.render(ChurnFormat::Json)
    })
}

fn call_coupling(args: Option<&Value>, ctx: &ServerContext<'_>) -> Result<Value, Value> {
    let options = CouplingOptions {
        settings: parse_coupling_settings(args)?,
        findings: parse_findings(args)?,
        plugin_host: ctx.plugin_host.clone(),
    };
    let folder = resolve_folder(args, ctx.default_folder)?;
    render_json_tool(coupling(&folder, &options), |report| {
        report.render(ChurnFormat::Json)
    })
}

fn parse_string_array(args: Option<&Value>, name: &str) -> Result<Vec<String>, Value> {
    match args.and_then(|a| a.get(name)) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(str::to_owned).ok_or_else(|| {
                    tool_err(&format!(
                        "Invalid {name} argument: every entry must be a string"
                    ))
                })
            })
            .collect(),
        Some(_) => Err(tool_err(&format!(
            "Invalid {name} argument: expected an array of strings"
        ))),
        None => Ok(Vec::new()),
    }
}

fn parse_usize_arg(value: &Value, error_message: &str) -> Result<usize, Value> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| tool_err(error_message))
}

fn parse_coupling_settings(args: Option<&Value>) -> Result<CouplingSettings, Value> {
    let defaults = CouplingSettings::default();
    let window = parse_churn_window(args)?;
    let max_commit_files = parse_optional_arg(
        args,
        "max_commit_files",
        defaults.max_commit_files,
        |value| parse_positive_usize(value, "max_commit_files"),
    )?;
    let min_co_changes =
        parse_optional_arg(args, "min_co_changes", defaults.min_co_changes, |value| {
            parse_positive_usize(value, "min_co_changes")
        })?;
    let min_ratio = parse_optional_arg(args, "min_ratio", defaults.min_ratio, parse_ratio)?;
    let include_tests =
        parse_optional_arg(args, "include_tests", defaults.include_tests, |value| {
            parse_bool_arg(value, "include_tests")
        })?;
    Ok(CouplingSettings {
        window,
        max_commit_files,
        min_co_changes,
        min_ratio,
        include_tests,
    })
}

fn parse_bool_arg(value: &Value, name: &str) -> Result<bool, Value> {
    value
        .as_bool()
        .ok_or_else(|| tool_err(&format!("Argument {name} must be a boolean")))
}

fn parse_optional_arg<T>(
    args: Option<&Value>,
    name: &str,
    default: T,
    parse: impl FnOnce(&Value) -> Result<T, Value>,
) -> Result<T, Value> {
    match args.and_then(|a| a.get(name)).filter(|v| !v.is_null()) {
        Some(value) => parse(value),
        None => Ok(default),
    }
}

fn parse_positive_usize(value: &Value, name: &str) -> Result<usize, Value> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|&n| n > 0)
        .ok_or_else(|| tool_err(&format!("Argument {name} must be a positive integer")))
}

fn parse_ratio(value: &Value) -> Result<CouplingRatio, Value> {
    let ratio = value
        .as_f64()
        .ok_or_else(|| tool_err("Argument min_ratio must be a number between 0 and 1"))?;
    CouplingRatio::new(ratio).map_err(|err| tool_err(&format!("Invalid min_ratio: {err}")))
}

fn parse_churn_window(args: Option<&Value>) -> Result<ChurnWindow, Value> {
    let since_val = args.and_then(|a| a.get("since")).filter(|v| !v.is_null());
    let commits_val = args.and_then(|a| a.get("commits")).filter(|v| !v.is_null());
    if since_val.is_some() && commits_val.is_some() {
        return Err(tool_err(
            "Arguments since and commits are mutually exclusive",
        ));
    }
    if let Some(value) = since_val {
        let text = value
            .as_str()
            .ok_or_else(|| tool_err("Argument since must be a string"))?;
        return ChurnSince::from_str(text)
            .map(ChurnWindow::from)
            .map_err(|err| tool_err(&format!("Invalid since window {text:?}: {err}")));
    }
    if let Some(value) = commits_val {
        let count = value
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|&n| n > 0)
            .ok_or_else(|| tool_err("Argument commits must be a positive integer"))?;
        return Ok(ChurnWindow::Commits(count));
    }
    Ok(ChurnWindow::default())
}

fn parse_package(args: Option<&Value>) -> Option<String> {
    args.and_then(|a| a.get("package"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn resolve_folder(args: Option<&Value>, default_folder: &Path) -> Result<PathBuf, Value> {
    parse_optional_arg(args, "path", default_folder.to_path_buf(), |value| {
        let path = value
            .as_str()
            .ok_or_else(|| tool_err("Argument path must be a string"))?;
        if path.is_empty() {
            return Ok(default_folder.to_path_buf());
        }
        let candidate = PathBuf::from(path);
        Ok(if candidate.is_absolute() {
            candidate
        } else {
            default_folder.join(candidate)
        })
    })
}

fn parse_findings(args: Option<&Value>) -> Result<FindingsShown, Value> {
    let all = parse_optional_arg(args, "all", false, |value| parse_bool_arg(value, "all"))?;
    Ok(if all {
        FindingsShown::All
    } else {
        CheckOptions::default().findings
    })
}

fn tool_ok(payload: Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": payload.to_string() }],
        "isError": false,
    })
}

fn tool_err(message: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "isError": true,
    })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}
