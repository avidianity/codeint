use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use codeint_config::{
    CalendarDay, ChurnSince, ChurnWindow, CouplingRatio, CouplingSettings, FindingsShown,
    PluginHostConfig, TrendRange,
};
use codeint_git::{Repository, ResultCache};
use codeint_report::McpToolSchema;
use serde::Serialize;
use serde_json::{Value, json};

use crate::check::{CheckError, CheckScope, WorkspaceRef};
use crate::{
    CheckOptions, CliError, CodeintError, TraceOptions, TrendOptions, check, churn, coupling, diff,
    guard, impact, trace, trend,
};

mod workspace_cache;
use workspace_cache::WorkspaceCache;

const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum McpFormat {
    Toon,
    Json,
}

#[derive(Serialize)]
struct OutcomeEnvelope<'a, R> {
    passed: bool,
    failures: &'a [String],
    report: &'a R,
}

#[derive(Serialize)]
struct JsonRpcSuccess<T: Serialize> {
    jsonrpc: &'static str,
    id: Value,
    result: T,
}

#[derive(Serialize)]
struct JsonRpcErrorBody<'a> {
    code: i64,
    message: &'a str,
}

#[derive(Serialize)]
struct JsonRpcError<'a> {
    jsonrpc: &'static str,
    id: Value,
    error: JsonRpcErrorBody<'a>,
}

#[derive(Serialize)]
struct ToolTextContent {
    #[serde(rename = "type")]
    content_type: &'static str,
    text: String,
}

#[derive(Serialize)]
struct ToolCallResult {
    content: [ToolTextContent; 1],
    #[serde(rename = "isError")]
    is_error: bool,
}

#[derive(Serialize)]
struct EmptyObject {}

#[derive(Serialize)]
struct InitializeCapabilities {
    tools: EmptyObject,
}

#[derive(Serialize)]
struct ServerInfo {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct InitializeResult<'a> {
    #[serde(rename = "protocolVersion")]
    protocol_version: &'a str,
    capabilities: InitializeCapabilities,
    #[serde(rename = "serverInfo")]
    server_info: ServerInfo,
}

#[derive(Serialize)]
struct ToolsListResult {
    tools: Vec<McpToolSchema>,
}

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
    let mut ctx = ServerContext::new(default_folder, result_cache, plugin_host);
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
        if handle_line_to_writer(trimmed, &mut ctx, writer)? {
            writer.flush()?;
        }
    }
    Ok(())
}

struct ServerContext<'a> {
    default_folder: &'a Path,
    result_cache: Option<PathBuf>,
    plugin_host: PluginHostConfig,
    workspace_cache: WorkspaceCache,
}

impl<'a> ServerContext<'a> {
    fn new(
        default_folder: &'a Path,
        result_cache: Option<PathBuf>,
        plugin_host: PluginHostConfig,
    ) -> ServerContext<'a> {
        ServerContext {
            default_folder,
            result_cache,
            plugin_host,
            workspace_cache: WorkspaceCache::default(),
        }
    }

    fn with_workspace<T, E>(
        &mut self,
        folder: &Path,
        map_check_error: fn(CheckError) -> E,
        run: impl FnOnce(WorkspaceRef<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let repository = Repository::discover(folder)
            .map_err(CheckError::Git)
            .map_err(map_check_error)?;
        let (configuration, plugins) = self
            .workspace_cache
            .workspace(repository.root(), &self.plugin_host)
            .map_err(map_check_error)?;
        run(WorkspaceRef {
            repository: &repository,
            configuration,
            plugins,
        })
    }

    fn with_check_workspace<T>(
        &mut self,
        folder: &Path,
        run: impl FnOnce(WorkspaceRef<'_>) -> Result<T, CheckError>,
    ) -> Result<T, CheckError> {
        self.with_workspace(folder, std::convert::identity, run)
    }
}

fn write_json_line<W: Write, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), std::io::Error> {
    serde_json::to_writer(&mut *writer, value).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")
}

fn write_rpc_success<W: Write, T: Serialize>(
    writer: &mut W,
    id: Value,
    result: T,
) -> Result<(), std::io::Error> {
    write_json_line(
        writer,
        &JsonRpcSuccess {
            jsonrpc: "2.0",
            id,
            result,
        },
    )
}

fn write_rpc_error<W: Write>(
    writer: &mut W,
    id: Value,
    code: i64,
    message: &str,
) -> Result<(), std::io::Error> {
    write_json_line(
        writer,
        &JsonRpcError {
            jsonrpc: "2.0",
            id,
            error: JsonRpcErrorBody { code, message },
        },
    )
}

fn handle_line_to_writer<W: Write>(
    line: &str,
    ctx: &mut ServerContext<'_>,
    writer: &mut W,
) -> Result<bool, std::io::Error> {
    let request: Value = match serde_json::from_str(line) {
        Ok(val) => val,
        Err(err) => {
            write_rpc_error(writer, Value::Null, -32700, &format!("Parse error: {err}"))?;
            return Ok(true);
        }
    };
    let Some(response_id) = request.get("id").cloned() else {
        return Ok(false);
    };
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    dispatch_rpc_method(writer, response_id, method, request.get("params"), ctx)?;
    Ok(true)
}

fn dispatch_rpc_method<W: Write>(
    writer: &mut W,
    response_id: Value,
    method: &str,
    params: Option<&Value>,
    ctx: &mut ServerContext<'_>,
) -> Result<(), std::io::Error> {
    match method {
        "initialize" => write_rpc_success(writer, response_id, handle_initialize(params)),
        "ping" => write_rpc_success(writer, response_id, EmptyObject {}),
        "tools/list" => write_rpc_success(
            writer,
            response_id,
            ToolsListResult {
                tools: tool_definitions(),
            },
        ),
        "tools/call" => write_tools_call_response(writer, response_id, params, ctx),
        other => write_rpc_error(
            writer,
            response_id,
            -32601,
            &format!("Method not found: {other}"),
        ),
    }
}

fn write_tools_call_response<W: Write>(
    writer: &mut W,
    response_id: Value,
    params: Option<&Value>,
    ctx: &mut ServerContext<'_>,
) -> Result<(), std::io::Error> {
    match handle_tools_call(params, ctx) {
        Ok(result) => write_rpc_success(writer, response_id, result),
        Err((code, message)) => write_rpc_error(writer, response_id, code, &message),
    }
}

fn handle_initialize(params: Option<&Value>) -> InitializeResult<'_> {
    let protocol_version = params
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    InitializeResult {
        protocol_version,
        capabilities: InitializeCapabilities {
            tools: EmptyObject {},
        },
        server_info: ServerInfo {
            name: "codeint",
            version: env!("CARGO_PKG_VERSION"),
        },
    }
}

fn format_property() -> Value {
    json!({
        "type": "string",
        "enum": ["toon", "json"],
        "default": "toon",
        "description": "Output format: toon (default) or json."
    })
}

pub(crate) fn tool_definitions() -> Vec<McpToolSchema> {
    vec![
        McpToolSchema {
            name: "check".to_owned(),
            description: "Measure Software entropy of a Git repository working tree (Erosion, Cognitive complexity, Duplication, Verbosity, Import cycles, Boundaries, Orphan files, Dependency hygiene, Test gap, Hazards) and report any Budget failures; Findings listed in .codeint/baseline.json are hidden unless ignore_baseline is true.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "package": { "type": "string", "description": "Optional Workspace package path to scope measurement and output." },
                    "coverage_reports": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional LCOV or Cobertura XML Coverage report paths; when provided, replaces coverage_reports in codeint.json."
                    },
                    "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." },
                    "ignore_baseline": { "type": "boolean", "description": "Ignore .codeint/baseline.json: show every Finding and evaluate Budgets on every Finding (default false)." },
                    "format": format_property()
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
                    "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." },
                    "format": format_property()
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
                    "since": { "type": "string", "description": "UTC date YYYY-MM-DD to start the trend from." },
                    "format": format_property()
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
                    "path": { "type": "string", "description": "Folder inside the Git repository (defaults to server working directory)." },
                    "format": format_property()
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
                    "package": { "type": "string", "description": "Optional Workspace package path to scope reported dependents." },
                    "format": format_property()
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
                    "max_paths": { "type": "integer", "minimum": 1, "description": "Maximum number of shortest paths to return when all is true (default 10)." },
                    "format": format_property()
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
                    "all": { "type": "boolean", "description": "Return all Findings instead of the 20 worst per Metric." },
                    "format": format_property()
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
                    "all": { "type": "boolean", "description": "Return all coupled pairs instead of the top 20." },
                    "format": format_property()
                }
            }),
        },
    ]
}

fn handle_tools_call(
    params: Option<&Value>,
    ctx: &mut ServerContext<'_>,
) -> Result<ToolCallResult, (i64, String)> {
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

fn parse_tool_arguments(params: Option<&Value>) -> Result<Option<&Value>, ToolCallResult> {
    match params.and_then(|p| p.get("arguments")) {
        None => Ok(None),
        Some(value) if value.is_object() => Ok(Some(value)),
        Some(_) => Err(tool_err("Tool arguments must be a JSON object")),
    }
}

fn dispatch_tool(
    name: &str,
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
) -> Result<ToolCallResult, (i64, String)> {
    let handler: fn(
        Option<&Value>,
        &mut ServerContext<'_>,
        McpFormat,
    ) -> Result<ToolCallResult, ToolCallResult> = match name {
        "check" => call_check,
        "diff" => call_diff,
        "trend" => call_trend,
        "guard" => call_guard,
        "impact" => call_impact,
        "trace" => call_trace,
        "churn" => call_churn,
        "coupling" => call_coupling,
        other => return Err((-32602, format!("Unknown tool: {other}"))),
    };
    let result = parse_format(args).and_then(|format| handler(args, ctx, format));
    Ok(result.unwrap_or_else(|err| err))
}

fn call_check(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let folder = resolve_folder(args, ctx.default_folder)?;
    let coverage_reports: Vec<PathBuf> = parse_string_array(args, "coverage_reports")?
        .into_iter()
        .map(PathBuf::from)
        .collect();
    let ignore_baseline = parse_optional_arg(args, "ignore_baseline", false, |value| {
        parse_bool_arg(value, "ignore_baseline")
    })?;
    let findings = parse_findings(args)?;
    let package = parse_package(args);
    let scope = CheckScope {
        findings,
        package: package.as_deref(),
        owner: None,
        coverage_reports: &coverage_reports,
        ignore_baseline,
    };
    let result = ctx.with_check_workspace(&folder, |workspace| {
        check::check_with_workspace(&folder, scope, workspace)
    });
    render_check_outcome(result, codeint_report::Report::failure_messages, format)
}

fn call_diff(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let base_ref = required_str_arg(args, "base_ref")?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    let head_ref = args
        .and_then(|a| a.get("head_ref"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let findings = parse_findings(args)?;
    let package = parse_package(args);
    let cache = ctx.result_cache.clone().map(ResultCache::in_folder);
    let refs = diff::Refs {
        base: base_ref,
        head: head_ref.as_deref(),
    };
    let result = ctx.with_check_workspace(&folder, |workspace| {
        diff::diff_with_workspace(
            workspace,
            refs,
            findings,
            package.as_deref(),
            None,
            cache.as_ref(),
        )
    });
    render_check_outcome(result, codeint_report::DiffReport::failure_messages, format)
}

fn render_check_outcome<R: Serialize>(
    result: Result<R, CheckError>,
    failure_messages: fn(&R) -> Vec<String>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    Ok(match result {
        Ok(report) => {
            let failures = failure_messages(&report);
            let passed = failures.is_empty();
            render_outcome(passed, &failures, &report, format)
        }
        Err(err) => tool_err(&CodeintError::from_cli_error(CliError::Check(err)).mcp_text()),
    })
}

fn render_outcome<R: Serialize>(
    passed: bool,
    failures: &[String],
    report: &R,
    format: McpFormat,
) -> ToolCallResult {
    let envelope = OutcomeEnvelope {
        passed,
        failures,
        report,
    };
    render_mcp_value(&envelope, format)
}

fn render_cli_tool<T: Serialize, E>(
    result: Result<T, E>,
    wrap_error: fn(E) -> CliError,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    Ok(match result {
        Ok(report) => render_mcp_value(&report, format),
        Err(err) => tool_err(&CodeintError::from_cli_error(wrap_error(err)).mcp_text()),
    })
}

fn render_mcp_value<T: Serialize>(value: &T, format: McpFormat) -> ToolCallResult {
    let rendered = match format {
        McpFormat::Toon => codeint_report::render_toon(value).map_err(|err| err.to_string()),
        McpFormat::Json => serde_json::to_string(value).map_err(|err| err.to_string()),
    };
    match rendered {
        Ok(text) => tool_ok_text(text),
        Err(err) => tool_err(&err),
    }
}

fn required_str_arg<'a>(args: Option<&'a Value>, name: &str) -> Result<&'a str, ToolCallResult> {
    args.and_then(|a| a.get(name))
        .and_then(Value::as_str)
        .ok_or_else(|| tool_err(&format!("Missing required argument: {name}")))
}

fn call_guard(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let file = required_str_arg(args, "file")?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    let result = guard::guard_with_config(&folder, Path::new(file), |root| {
        ctx.workspace_cache.configuration(root)
    });
    render_cli_tool(result, CliError::Guard, format)
}

fn call_trend(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let folder = resolve_folder(args, ctx.default_folder)?;
    let range = match args.and_then(|a| a.get("since")).and_then(Value::as_str) {
        Some(day_str) => match CalendarDay::from_str(day_str) {
            Ok(day) => TrendRange::Since(day),
            Err(err) => return Err(tool_err(&format!("Invalid since date {day_str:?}: {err}"))),
        },
        None => TrendOptions::default().range,
    };
    let cache = ctx.result_cache.clone().map(ResultCache::in_folder);
    let result = ctx.with_check_workspace(&folder, |workspace| {
        trend::trend_with_workspace(workspace, range, cache.as_ref())
    });
    render_cli_tool(result, CliError::Check, format)
}

fn call_impact(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let folder = resolve_folder(args, ctx.default_folder)?;
    let paths = parse_string_array(args, "paths")?;
    let changed = args
        .and_then(|a| a.get("changed"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if paths.is_empty() && changed.is_none() {
        return Err(tool_err(
            "Provide at least one target file path in paths, or pass changed with a base Git ref to analyze changed files.",
        ));
    }
    let depth = match args.and_then(|a| a.get("depth")) {
        Some(value) => Some(parse_usize_arg(
            value,
            "Invalid depth argument: expected a non-negative integer",
        )?),
        None => None,
    };
    let package = parse_package(args);
    let plugin_host = ctx.plugin_host.clone();
    let request = impact::ImpactRequest {
        paths: &paths,
        changed: changed.as_deref(),
        depth,
        package: package.as_deref(),
        plugin_host: &plugin_host,
    };
    let result = ctx.with_workspace(&folder, impact::ImpactCommandError::Check, |workspace| {
        impact::impact_with_workspace(&folder, workspace, &request)
    });
    render_cli_tool(result, CliError::Impact, format)
}

fn call_trace(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let from = required_str_arg(args, "from")?;
    let to = required_str_arg(args, "to")?;
    let max_paths = parse_optional_arg(
        args,
        "max_paths",
        TraceOptions::default().max_paths,
        |value| parse_positive_usize(value, "max_paths"),
    )?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    let all = args
        .and_then(|a| a.get("all"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let package = parse_package(args);
    let result = ctx.with_workspace(&folder, trace::TraceCommandError::Check, |workspace| {
        trace::trace_with_workspace(
            &folder,
            workspace,
            from,
            to,
            all,
            max_paths,
            package.as_deref(),
        )
    });
    render_cli_tool(result, CliError::Trace, format)
}

fn call_churn(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let window = parse_churn_window(args)?;
    let findings = parse_findings(args)?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    let result = ctx.with_check_workspace(&folder, |workspace| {
        churn::churn_with_workspace(workspace, window, findings)
    });
    render_cli_tool(result, CliError::Check, format)
}

fn call_coupling(
    args: Option<&Value>,
    ctx: &mut ServerContext<'_>,
    format: McpFormat,
) -> Result<ToolCallResult, ToolCallResult> {
    let settings = parse_coupling_settings(args)?;
    let findings = parse_findings(args)?;
    let folder = resolve_folder(args, ctx.default_folder)?;
    let result = ctx.with_check_workspace(&folder, |workspace| {
        coupling::coupling_with_workspace(workspace, settings, findings)
    });
    render_cli_tool(result, CliError::Check, format)
}

fn parse_format(args: Option<&Value>) -> Result<McpFormat, ToolCallResult> {
    parse_optional_arg(args, "format", McpFormat::Toon, |value| {
        match value.as_str() {
            Some("toon") => Ok(McpFormat::Toon),
            Some("json") => Ok(McpFormat::Json),
            _ => Err(tool_err(
                "Invalid format argument: expected \"toon\" or \"json\"",
            )),
        }
    })
}

fn parse_string_array(args: Option<&Value>, name: &str) -> Result<Vec<String>, ToolCallResult> {
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

fn parse_usize_arg(value: &Value, error_message: &str) -> Result<usize, ToolCallResult> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| tool_err(error_message))
}

fn parse_coupling_settings(args: Option<&Value>) -> Result<CouplingSettings, ToolCallResult> {
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

fn parse_bool_arg(value: &Value, name: &str) -> Result<bool, ToolCallResult> {
    value
        .as_bool()
        .ok_or_else(|| tool_err(&format!("Argument {name} must be a boolean")))
}

fn parse_optional_arg<T>(
    args: Option<&Value>,
    name: &str,
    default: T,
    parse: impl FnOnce(&Value) -> Result<T, ToolCallResult>,
) -> Result<T, ToolCallResult> {
    match args.and_then(|a| a.get(name)).filter(|v| !v.is_null()) {
        Some(value) => parse(value),
        None => Ok(default),
    }
}

fn parse_positive_usize(value: &Value, name: &str) -> Result<usize, ToolCallResult> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|&n| n > 0)
        .ok_or_else(|| tool_err(&format!("Argument {name} must be a positive integer")))
}

fn parse_ratio(value: &Value) -> Result<CouplingRatio, ToolCallResult> {
    let ratio = value
        .as_f64()
        .ok_or_else(|| tool_err("Argument min_ratio must be a number between 0 and 1"))?;
    CouplingRatio::new(ratio).map_err(|err| tool_err(&format!("Invalid min_ratio: {err}")))
}

fn parse_churn_window(args: Option<&Value>) -> Result<ChurnWindow, ToolCallResult> {
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

fn resolve_folder(args: Option<&Value>, default_folder: &Path) -> Result<PathBuf, ToolCallResult> {
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

fn parse_findings(args: Option<&Value>) -> Result<FindingsShown, ToolCallResult> {
    let all = parse_optional_arg(args, "all", false, |value| parse_bool_arg(value, "all"))?;
    Ok(if all {
        FindingsShown::All
    } else {
        CheckOptions::default().findings
    })
}

fn tool_ok_text(text: String) -> ToolCallResult {
    ToolCallResult {
        content: [ToolTextContent {
            content_type: "text",
            text,
        }],
        is_error: false,
    }
}

fn tool_err(message: &str) -> ToolCallResult {
    ToolCallResult {
        content: [ToolTextContent {
            content_type: "text",
            text: message.to_owned(),
        }],
        is_error: true,
    }
}

#[cfg(test)]
mod tests;
