use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use codeint_config::AgentClient;
use codeint_report::{
    AgentInstallAction, AgentInstallEntry, AgentInstallReport, AgentStatusEntry, AgentStatusReport,
};
use serde_json::{Map as JsonMap, Value as JsonValue};

const CODEINT_SERVER_NAME: &str = "codeint";
const CODEINT_COMMAND: &str = "codeint";
const CODEINT_MCP_ARG: &str = "mcp";
const JSON_MCP_KEY: &str = "mcpServers";
const TOML_MCP_KEY: &str = "mcp_servers";

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, thiserror::Error)]
pub(crate) enum AgentError {
    #[error("cannot read agent configuration file {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("agent configuration file {} is not valid JSON", path.display())]
    InvalidJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("agent configuration file {} is not valid TOML", path.display())]
    InvalidToml {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("cannot serialize JSON configuration for {}", path.display())]
    SerializeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("cannot serialize TOML configuration for {}", path.display())]
    SerializeToml {
        path: PathBuf,
        #[source]
        source: toml::ser::Error,
    },
    #[error(
        "agent configuration file {} must contain a top-level JSON object",
        path.display()
    )]
    ExpectedRootObject { path: PathBuf },
    #[error(
        "agent configuration file {} has a non-object {key} entry",
        path.display()
    )]
    InvalidMcpSection { path: PathBuf, key: &'static str },
    #[error(
        "agent configuration file {} has a non-object {key} server entry",
        path.display()
    )]
    InvalidCodeintEntry { path: PathBuf, key: &'static str },
    #[error("cannot create folder {}", path.display())]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write agent configuration file {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigFormat {
    Json,
    Toml,
}

enum MergeOutcome {
    Unchanged,
    Changed(String),
}

struct PlannedUpdate {
    client: AgentClient,
    config_path: PathBuf,
    action: AgentInstallAction,
    content: Option<String>,
}

trait ConfigDocument: Sized {
    const MCP_KEY: &'static str;

    fn empty() -> Self;
    fn parse(path: &Path, raw: &str) -> Result<Self, AgentError>;
    fn render(&self, path: &Path) -> Result<String, AgentError>;
    fn child_object(&self, key: &str) -> Result<Option<&Self>, ()>;
    fn ensure_child_object(&mut self, key: &str) -> Result<&mut Self, ()>;
    fn matches_codeint_mcp(&self) -> bool;
    fn write_codeint_mcp(&mut self);
}

impl ConfigDocument for JsonMap<String, JsonValue> {
    const MCP_KEY: &'static str = JSON_MCP_KEY;

    fn empty() -> Self {
        JsonMap::new()
    }

    fn parse(path: &Path, raw: &str) -> Result<Self, AgentError> {
        let parsed: JsonValue =
            serde_json::from_str(raw).map_err(|source| AgentError::InvalidJson {
                path: path.to_path_buf(),
                source,
            })?;
        match parsed {
            JsonValue::Object(map) => Ok(map),
            _ => Err(AgentError::ExpectedRootObject {
                path: path.to_path_buf(),
            }),
        }
    }

    fn render(&self, path: &Path) -> Result<String, AgentError> {
        let mut rendered =
            serde_json::to_string_pretty(&JsonValue::Object(self.clone())).map_err(|source| {
                AgentError::SerializeJson {
                    path: path.to_path_buf(),
                    source,
                }
            })?;
        rendered.push('\n');
        Ok(rendered)
    }

    fn child_object(&self, key: &str) -> Result<Option<&Self>, ()> {
        match self.get(key) {
            None => Ok(None),
            Some(JsonValue::Object(map)) => Ok(Some(map)),
            Some(_) => Err(()),
        }
    }

    fn ensure_child_object(&mut self, key: &str) -> Result<&mut Self, ()> {
        self.entry(key.to_owned())
            .or_insert_with(|| JsonValue::Object(JsonMap::new()))
            .as_object_mut()
            .ok_or(())
    }

    fn matches_codeint_mcp(&self) -> bool {
        self.get("command") == Some(&JsonValue::String(CODEINT_COMMAND.to_owned()))
            && self.get("args") == Some(&serde_json::json!([CODEINT_MCP_ARG]))
    }

    fn write_codeint_mcp(&mut self) {
        self.insert(
            "command".to_owned(),
            JsonValue::String(CODEINT_COMMAND.to_owned()),
        );
        self.insert("args".to_owned(), serde_json::json!([CODEINT_MCP_ARG]));
    }
}

impl ConfigDocument for toml::Table {
    const MCP_KEY: &'static str = TOML_MCP_KEY;

    fn empty() -> Self {
        toml::Table::new()
    }

    fn parse(path: &Path, raw: &str) -> Result<Self, AgentError> {
        toml::from_str::<toml::Table>(raw).map_err(|source| AgentError::InvalidToml {
            path: path.to_path_buf(),
            source,
        })
    }

    fn render(&self, path: &Path) -> Result<String, AgentError> {
        let mut rendered =
            toml::to_string_pretty(self).map_err(|source| AgentError::SerializeToml {
                path: path.to_path_buf(),
                source,
            })?;
        if !rendered.ends_with('\n') {
            rendered.push('\n');
        }
        Ok(rendered)
    }

    fn child_object(&self, key: &str) -> Result<Option<&Self>, ()> {
        self.get(key)
            .map(|value| value.as_table().ok_or(()))
            .transpose()
    }

    fn ensure_child_object(&mut self, key: &str) -> Result<&mut Self, ()> {
        if !self.contains_key(key) {
            self.insert(key.to_owned(), toml::Value::Table(toml::Table::new()));
        }
        self.get_mut(key)
            .and_then(toml::Value::as_table_mut)
            .ok_or(())
    }

    fn matches_codeint_mcp(&self) -> bool {
        matches!(
            (self.get("command"), self.get("args")),
            (Some(toml::Value::String(cmd)), Some(toml::Value::Array(args)))
                if cmd == CODEINT_COMMAND
                    && args.len() == 1
                    && args[0].as_str() == Some(CODEINT_MCP_ARG)
        )
    }

    fn write_codeint_mcp(&mut self) {
        let args = vec![toml::Value::from(CODEINT_MCP_ARG)];
        self.insert("command".to_owned(), toml::Value::from(CODEINT_COMMAND));
        self.insert("args".to_owned(), toml::Value::from(args));
    }
}

pub(crate) fn status(
    config_root: &Path,
    selected: Option<AgentClient>,
) -> Result<AgentStatusReport, AgentError> {
    let clients = selected.map_or_else(|| AgentClient::ALL.to_vec(), |client| vec![client]);
    let mut agents = Vec::with_capacity(clients.len());
    for client in clients {
        agents.push(inspect_client(config_root, client)?);
    }
    Ok(AgentStatusReport {
        config_root: config_root.display().to_string(),
        agents,
    })
}

pub(crate) fn install(
    config_root: &Path,
    selected: Option<AgentClient>,
    dry_run: bool,
) -> Result<AgentInstallReport, AgentError> {
    let targets = resolve_install_targets(config_root, selected);
    if targets.is_empty() {
        return Ok(AgentInstallReport {
            config_root: config_root.display().to_string(),
            dry_run,
            results: Vec::new(),
            note: Some(
                "No supported coding agents detected; pass --client <name> to configure one explicitly."
                    .to_owned(),
            ),
        });
    }
    let mut results = Vec::with_capacity(targets.len());
    for client in targets {
        let plan = plan_client_update(config_root, client)?;
        results.push(apply_or_preview(plan, dry_run)?);
    }
    Ok(AgentInstallReport {
        config_root: config_root.display().to_string(),
        dry_run,
        results,
        note: None,
    })
}

fn resolve_install_targets(config_root: &Path, selected: Option<AgentClient>) -> Vec<AgentClient> {
    match selected {
        Some(client) => vec![client],
        None => AgentClient::ALL
            .into_iter()
            .filter(|client| is_client_detected(config_root, *client))
            .collect(),
    }
}

fn apply_or_preview(plan: PlannedUpdate, dry_run: bool) -> Result<AgentInstallEntry, AgentError> {
    let (written, preview) = if dry_run {
        (false, plan.content)
    } else if let Some(content) = plan.content {
        atomic_write(&plan.config_path, content.as_bytes())?;
        (true, None)
    } else {
        (false, None)
    };
    Ok(AgentInstallEntry {
        client: plan.client.as_str(),
        name: plan.client.display_name(),
        config_path: plan.config_path.display().to_string(),
        action: plan.action,
        written,
        preview,
    })
}

fn inspect_client(config_root: &Path, client: AgentClient) -> Result<AgentStatusEntry, AgentError> {
    let config_path = client_config_path(config_root, client);
    let detected = is_client_detected(config_root, client);
    let config_exists = config_path.is_file();
    let registered = if config_exists {
        let raw = read_config_file(&config_path)?;
        match client_format(client) {
            ConfigFormat::Json => {
                is_registered_in_doc::<JsonMap<String, JsonValue>>(&config_path, &raw)?
            }
            ConfigFormat::Toml => is_registered_in_doc::<toml::Table>(&config_path, &raw)?,
        }
    } else {
        false
    };
    Ok(AgentStatusEntry {
        client: client.as_str(),
        name: client.display_name(),
        config_path: config_path.display().to_string(),
        detected,
        config_exists,
        registered,
    })
}

fn plan_client_update(
    config_root: &Path,
    client: AgentClient,
) -> Result<PlannedUpdate, AgentError> {
    let config_path = client_config_path(config_root, client);
    let existed = config_path.is_file();
    let existing = if existed {
        Some(read_config_file(&config_path)?)
    } else {
        None
    };
    let merge = match client_format(client) {
        ConfigFormat::Json => {
            merge_document::<JsonMap<String, JsonValue>>(&config_path, existing.as_deref())?
        }
        ConfigFormat::Toml => merge_document::<toml::Table>(&config_path, existing.as_deref())?,
    };
    let (action, content) = match merge {
        MergeOutcome::Unchanged => (AgentInstallAction::Unchanged, None),
        MergeOutcome::Changed(rendered) => {
            let action = if existed {
                AgentInstallAction::Update
            } else {
                AgentInstallAction::Create
            };
            (action, Some(rendered))
        }
    };
    Ok(PlannedUpdate {
        client,
        config_path,
        action,
        content,
    })
}

fn is_registered_in_doc<D: ConfigDocument>(path: &Path, raw: &str) -> Result<bool, AgentError> {
    if raw.trim().is_empty() {
        return Ok(false);
    }
    let root = D::parse(path, raw)?;
    doc_has_codeint_mcp(path, &root)
}

fn doc_has_codeint_mcp<D: ConfigDocument>(path: &Path, root: &D) -> Result<bool, AgentError> {
    let Some(servers) =
        root.child_object(D::MCP_KEY)
            .map_err(|()| AgentError::InvalidMcpSection {
                path: path.to_path_buf(),
                key: D::MCP_KEY,
            })?
    else {
        return Ok(false);
    };
    let Ok(Some(entry)) = servers.child_object(CODEINT_SERVER_NAME) else {
        return Ok(false);
    };
    Ok(entry.matches_codeint_mcp())
}

fn merge_document<D: ConfigDocument>(
    path: &Path,
    existing: Option<&str>,
) -> Result<MergeOutcome, AgentError> {
    let mut root = match existing.map(str::trim).filter(|text| !text.is_empty()) {
        Some(raw) => D::parse(path, raw)?,
        None => D::empty(),
    };
    if doc_has_codeint_mcp(path, &root)? {
        return Ok(MergeOutcome::Unchanged);
    }
    let servers =
        root.ensure_child_object(D::MCP_KEY)
            .map_err(|()| AgentError::InvalidMcpSection {
                path: path.to_path_buf(),
                key: D::MCP_KEY,
            })?;
    let entry = servers
        .ensure_child_object(CODEINT_SERVER_NAME)
        .map_err(|()| AgentError::InvalidCodeintEntry {
            path: path.to_path_buf(),
            key: CODEINT_SERVER_NAME,
        })?;
    entry.write_codeint_mcp();
    Ok(MergeOutcome::Changed(root.render(path)?))
}

fn client_config_path(config_root: &Path, client: AgentClient) -> PathBuf {
    match client {
        AgentClient::ClaudeCode => config_root.join(".claude.json"),
        AgentClient::Cursor => config_root.join(".cursor").join("mcp.json"),
        AgentClient::Codex => config_root.join(".codex").join("config.toml"),
    }
}

fn is_client_detected(config_root: &Path, client: AgentClient) -> bool {
    match client {
        AgentClient::ClaudeCode => {
            config_root.join(".claude.json").is_file() || config_root.join(".claude").is_dir()
        }
        AgentClient::Cursor => {
            config_root.join(".cursor").is_dir()
                || config_root.join(".cursor").join("mcp.json").is_file()
        }
        AgentClient::Codex => {
            config_root.join(".codex").is_dir()
                || config_root.join(".codex").join("config.toml").is_file()
        }
    }
}

fn client_format(client: AgentClient) -> ConfigFormat {
    match client {
        AgentClient::ClaudeCode | AgentClient::Cursor => ConfigFormat::Json,
        AgentClient::Codex => ConfigFormat::Toml,
    }
}

fn read_config_file(path: &Path) -> Result<String, AgentError> {
    fs::read_to_string(path).map_err(|source| AgentError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn atomic_write(target: &Path, bytes: &[u8]) -> Result<(), AgentError> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| AgentError::CreateDir {
        path: parent.to_path_buf(),
        source,
    })?;
    let (temp_path, mut file) = create_temp_file(parent, target)?;
    let commit = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, target)
    })();
    if let Err(source) = commit {
        let _ = fs::remove_file(&temp_path);
        return Err(AgentError::Write {
            path: target.to_path_buf(),
            source,
        });
    }
    Ok(())
}

fn create_temp_file(parent: &Path, target: &Path) -> Result<(PathBuf, fs::File), AgentError> {
    let base_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");
    let pid = std::process::id();
    for _ in 0..64 {
        let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(".{base_name}.codeint-tmp-{pid}-{seq}"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(AgentError::Write {
                    path: target.to_path_buf(),
                    source,
                });
            }
        }
    }
    Err(AgentError::Write {
        path: target.to_path_buf(),
        source: std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a unique temporary file",
        ),
    })
}
