use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use codeint_config::{
    CalendarDay, FindingsShown, GuardFormat, ImpactFormat, PluginHostConfig, ReportFormat,
    TraceFormat, TrendFormat, TrendRange,
};
use codeint_report::McpToolSchema;
use serde_json::{Value, json};

use crate::{
    CheckOptions, DiffOptions, ImpactOptions, TraceOptions, TrendOptions, check, diff, guard,
    impact, trace, trend,
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
            description: "Measure Software entropy of a Git repository working tree (Erosion, Duplication, Verbosity, Import cycles) and report any Budget failures.".to_owned(),
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
    let args = params.and_then(|p| p.get("arguments"));
    match name {
        "check" => Ok(call_check(args, ctx)),
        "diff" => Ok(call_diff(args, ctx)),
        "trend" => Ok(call_trend(args, ctx)),
        "guard" => Ok(call_guard(args, ctx)),
        "impact" => Ok(call_impact(args, ctx)),
        "trace" => Ok(call_trace(args, ctx)),
        other => Err((-32602, format!("Unknown tool: {other}"))),
    }
}

fn call_check(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let folder = resolve_folder(args, ctx.default_folder);
    let options = CheckOptions {
        findings: parse_findings(args),
        package: parse_package(args),
        plugin_host: ctx.plugin_host.clone(),
    };
    match check(&folder, &options) {
        Ok(outcome) => render_outcome(
            outcome.passed,
            outcome.failures,
            outcome.report.render(ReportFormat::Json),
        ),
        Err(err) => tool_err(&err.to_string()),
    }
}

fn call_diff(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let base_ref = match required_str_arg(args, "base_ref") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let folder = resolve_folder(args, ctx.default_folder);
    let options = DiffOptions {
        head_ref: args
            .and_then(|a| a.get("head_ref"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        findings: parse_findings(args),
        package: parse_package(args),
        result_cache: ctx.result_cache.clone(),
        plugin_host: ctx.plugin_host.clone(),
    };
    match diff(&folder, base_ref, &options) {
        Ok(outcome) => render_outcome(
            outcome.passed,
            outcome.failures,
            outcome.report.render(ReportFormat::Json),
        ),
        Err(err) => tool_err(&err.to_string()),
    }
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
) -> Value {
    match result {
        Ok(report) => match render(report) {
            Ok(rendered) => {
                let payload: Value = serde_json::from_str(&rendered).unwrap_or(Value::Null);
                tool_ok(payload)
            }
            Err(err) => tool_err(&err.to_string()),
        },
        Err(err) => tool_err(&err.to_string()),
    }
}

fn required_str_arg<'a>(args: Option<&'a Value>, name: &str) -> Result<&'a str, Value> {
    args.and_then(|a| a.get(name))
        .and_then(Value::as_str)
        .ok_or_else(|| tool_err(&format!("Missing required argument: {name}")))
}

fn call_trend(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let folder = resolve_folder(args, ctx.default_folder);
    let range = match args.and_then(|a| a.get("since")).and_then(Value::as_str) {
        Some(day_str) => match CalendarDay::from_str(day_str) {
            Ok(day) => TrendRange::Since(day),
            Err(err) => return tool_err(&format!("Invalid since date {day_str:?}: {err}")),
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

fn call_guard(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let file = match required_str_arg(args, "file") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let folder = resolve_folder(args, ctx.default_folder);
    render_json_tool(guard(&folder, Path::new(file)), |report| {
        report.render(GuardFormat::Json)
    })
}

fn call_impact(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let folder = resolve_folder(args, ctx.default_folder);
    let paths: Vec<String> = match args.and_then(|a| a.get("paths")) {
        Some(Value::Array(items)) => {
            let mut parsed = Vec::with_capacity(items.len());
            for item in items {
                let Some(path) = item.as_str() else {
                    return tool_err("Invalid paths argument: every entry must be a string");
                };
                parsed.push(path.to_owned());
            }
            parsed
        }
        Some(_) => return tool_err("Invalid paths argument: expected an array of strings"),
        None => Vec::new(),
    };
    let changed = args
        .and_then(|a| a.get("changed"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let depth = match args.and_then(|a| a.get("depth")) {
        Some(value) => match value.as_u64().and_then(|n| usize::try_from(n).ok()) {
            Some(depth) => Some(depth),
            None => return tool_err("Invalid depth argument: expected a non-negative integer"),
        },
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

fn call_trace(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let from = match required_str_arg(args, "from") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let to = match required_str_arg(args, "to") {
        Ok(value) => value,
        Err(err) => return err,
    };
    let folder = resolve_folder(args, ctx.default_folder);
    let all = args
        .and_then(|a| a.get("all"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_paths = match args.and_then(|a| a.get("max_paths")) {
        Some(value) => match value.as_u64().and_then(|n| usize::try_from(n).ok()) {
            Some(n) => n,
            None => return tool_err("Invalid argument: max_paths must be a positive integer"),
        },
        None => TraceOptions::default().max_paths,
    };
    let options = TraceOptions {
        all,
        max_paths,
        package: parse_package(args),
        plugin_host: ctx.plugin_host.clone(),
    };
    render_json_tool(trace(&folder, from, to, &options), |report| {
        report.render(TraceFormat::Json)
    })
}

fn parse_package(args: Option<&Value>) -> Option<String> {
    args.and_then(|a| a.get("package"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn resolve_folder(args: Option<&Value>, default_folder: &Path) -> PathBuf {
    match args.and_then(|a| a.get("path")).and_then(Value::as_str) {
        Some(p) if !p.is_empty() => {
            let candidate = PathBuf::from(p);
            if candidate.is_absolute() {
                candidate
            } else {
                default_folder.join(candidate)
            }
        }
        _ => default_folder.to_path_buf(),
    }
}

fn parse_findings(args: Option<&Value>) -> FindingsShown {
    if args
        .and_then(|a| a.get("all"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        FindingsShown::All
    } else {
        CheckOptions::default().findings
    }
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
