use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use codeint_config::{
    CalendarDay, FindingsShown, GuardFormat, PluginHostConfig, ReportFormat, TrendFormat,
    TrendRange,
};
use serde_json::{Value, json};

use crate::{CheckOptions, DiffOptions, TrendOptions, check, diff, guard, trend};

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

fn handle_tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "check",
                "description": "Measure Software entropy of a Git repository working tree (Erosion, Duplication, Verbosity, Import cycles) and report any Budget failures.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                        "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." }
                    }
                }
            },
            {
                "name": "diff",
                "description": "Compare the working tree (or a head Git ref) against the merge-base of base_ref and classify Findings as new, resolved, or unchanged.",
                "inputSchema": {
                    "type": "object",
                    "required": ["base_ref"],
                    "properties": {
                        "base_ref": { "type": "string", "description": "Base Git ref such as main or origin/main." },
                        "head_ref": { "type": "string", "description": "Optional head Git ref instead of the working tree." },
                        "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                        "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." }
                    }
                }
            },
            {
                "name": "trend",
                "description": "Measure each main-line commit over time (latest 20 commits or since YYYY-MM-DD).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                        "since": { "type": "string", "description": "UTC date YYYY-MM-DD to start the trend from." }
                    }
                }
            },
            {
                "name": "guard",
                "description": "Query the architectural boundary zone, allowed and denied target zones, public_files constraints, and allowed importers for an existing or planned file path.",
                "inputSchema": {
                    "type": "object",
                    "required": ["file"],
                    "properties": {
                        "file": { "type": "string", "description": "Repository-relative or absolute path to the existing or planned file." },
                        "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." }
                    }
                }
            }
        ]
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
        other => Err((-32602, format!("Unknown tool: {other}"))),
    }
}

fn call_check(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let folder = resolve_folder(args, ctx.default_folder);
    let options = CheckOptions {
        findings: parse_findings(args),
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
    let Some(base_ref) = args.and_then(|a| a.get("base_ref")).and_then(Value::as_str) else {
        return tool_err("Missing required argument: base_ref");
    };
    let folder = resolve_folder(args, ctx.default_folder);
    let options = DiffOptions {
        head_ref: args
            .and_then(|a| a.get("head_ref"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        findings: parse_findings(args),
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
    match trend(&folder, &options) {
        Ok(report) => match report.render(TrendFormat::Json) {
            Ok(rendered) => {
                let payload: Value = serde_json::from_str(&rendered).unwrap_or(Value::Null);
                tool_ok(payload)
            }
            Err(err) => tool_err(&err.to_string()),
        },
        Err(err) => tool_err(&err.to_string()),
    }
}

fn call_guard(args: Option<&Value>, ctx: &ServerContext<'_>) -> Value {
    let Some(file) = args.and_then(|a| a.get("file")).and_then(Value::as_str) else {
        return tool_err("Missing required argument: file");
    };
    let folder = resolve_folder(args, ctx.default_folder);
    match guard(&folder, Path::new(file)) {
        Ok(report) => match report.render(GuardFormat::Json) {
            Ok(rendered) => {
                let payload: Value = serde_json::from_str(&rendered).unwrap_or(Value::Null);
                tool_ok(payload)
            }
            Err(err) => tool_err(&err.to_string()),
        },
        Err(err) => tool_err(&err.to_string()),
    }
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
