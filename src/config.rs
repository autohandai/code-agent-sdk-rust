use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const RESTRICTED_MAX_EVENTS: usize = 256;
const RESTRICTED_MAX_EVENT_BYTES: usize = 64 * 1024;
const RESTRICTED_MAX_STDOUT_BYTES: usize = 1024 * 1024;
const RESTRICTED_MAX_STDERR_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientContext {
    Vscode,
    Chrome,
    Blueprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupTrafficClass {
    AutohandDeviceAuthorization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildNetworkPolicy {
    Inherit,
    DenyAll,
    SetupOnly(SetupTrafficClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnswerOnlyProfile {
    private: (),
}

impl AnswerOnlyProfile {
    pub fn blueprint() -> Self {
        Self { private: () }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupOnlyProfile {
    traffic: SetupTrafficClass,
}

impl SetupOnlyProfile {
    pub fn autohand_device_authorization() -> Self {
        Self {
            traffic: SetupTrafficClass::AutohandDeviceAuthorization,
        }
    }

    pub fn traffic(&self) -> SetupTrafficClass {
        self.traffic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeProfile {
    Interactive,
    AnswerOnly(AnswerOnlyProfile),
    SetupOnly(SetupOnlyProfile),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderName {
    AutohandAi,
    OpenRouter,
    Ollama,
    LlamaCpp,
    OpenAi,
    Mlx,
    LlmGateway,
    Azure,
    Zai,
    Sakana,
    Xai,
    Cerebras,
    DeepSeek,
    VertexAi,
    Nvidia,
    Bedrock,
    Custom(String),
}
impl ProviderName {
    pub fn as_str(&self) -> &str {
        match self {
            Self::AutohandAi => "autohandai",
            Self::OpenRouter => "openrouter",
            Self::Ollama => "ollama",
            Self::LlamaCpp => "llamacpp",
            Self::OpenAi => "openai",
            Self::Mlx => "mlx",
            Self::LlmGateway => "llmgateway",
            Self::Azure => "azure",
            Self::Zai => "zai",
            Self::Sakana => "sakana",
            Self::Xai => "xai",
            Self::Cerebras => "cerebras",
            Self::DeepSeek => "deepseek",
            Self::VertexAi => "vertexai",
            Self::Nvidia => "nvidia",
            Self::Bedrock => "bedrock",
            Self::Custom(value) => value,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlagSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_overrides: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_v2: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws_bedrock_provider: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slash_goal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage_status: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_fork: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_clone: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_handoff: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub cwd: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub debug: bool,
    pub timeout: Duration,
    pub unrestricted: bool,
    pub bare: bool,
    pub idle_logout: Option<bool>,
    pub auto_mode: bool,
    pub auto_skill: bool,
    pub auto_commit: bool,
    pub context_compact: Option<bool>,
    pub persist_session: bool,
    pub session_id: Option<String>,
    pub resume: bool,
    pub continue_session: bool,
    pub fork: bool,
    pub session_path: Option<PathBuf>,
    pub auto_save_interval: Option<u32>,
    pub agents_md: Option<bool>,
    pub agents_md_create: bool,
    pub agents_md_path: Option<PathBuf>,
    pub agents_md_auto_update: bool,
    pub max_tokens: Option<u64>,
    pub compression_threshold: Option<f64>,
    pub summarization_threshold: Option<f64>,
    pub max_iterations: Option<u32>,
    pub max_runtime_minutes: Option<u32>,
    pub max_cost: Option<f64>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub system_prompt_file: Option<PathBuf>,
    pub append_system_prompt_file: Option<PathBuf>,
    pub display_language: Option<String>,
    pub mcp_config: Option<PathBuf>,
    pub agents: Option<PathBuf>,
    pub plugin_dir: Option<PathBuf>,
    pub yolo: Option<String>,
    pub yolo_timeout_seconds: Option<u32>,
    pub additional_directories: Vec<PathBuf>,
    pub skills: Vec<String>,
    pub skill_sources: Vec<String>,
    pub install_missing_skills: bool,
    pub provider: Option<ProviderName>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub autohand_ai_plan: Option<String>,
    pub features: Option<FeatureFlagSettings>,
    pub extra_args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub runtime_profile: RuntimeProfile,
    pub clear_environment: bool,
    pub network_policy: ChildNetworkPolicy,
    pub environment_allowlist: BTreeSet<String>,
    pub max_events: usize,
    pub max_event_bytes: usize,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cwd: None,
            cli_path: None,
            debug: false,
            timeout: Duration::from_secs(300),
            unrestricted: false,
            bare: false,
            idle_logout: None,
            auto_mode: false,
            auto_skill: false,
            auto_commit: false,
            context_compact: None,
            persist_session: false,
            session_id: None,
            resume: false,
            continue_session: false,
            fork: false,
            session_path: None,
            auto_save_interval: None,
            agents_md: None,
            agents_md_create: false,
            agents_md_path: None,
            agents_md_auto_update: false,
            max_tokens: None,
            compression_threshold: None,
            summarization_threshold: None,
            max_iterations: None,
            max_runtime_minutes: None,
            max_cost: None,
            model: None,
            temperature: None,
            system_prompt: None,
            append_system_prompt: None,
            system_prompt_file: None,
            append_system_prompt_file: None,
            display_language: None,
            mcp_config: None,
            agents: None,
            plugin_dir: None,
            yolo: None,
            yolo_timeout_seconds: None,
            additional_directories: Vec::new(),
            skills: Vec::new(),
            skill_sources: Vec::new(),
            install_missing_skills: false,
            provider: None,
            api_key: None,
            base_url: None,
            autohand_ai_plan: None,
            features: None,
            extra_args: Vec::new(),
            env: BTreeMap::new(),
            runtime_profile: RuntimeProfile::Interactive,
            clear_environment: false,
            network_policy: ChildNetworkPolicy::Inherit,
            environment_allowlist: BTreeSet::new(),
            max_events: RESTRICTED_MAX_EVENTS,
            max_event_bytes: RESTRICTED_MAX_EVENT_BYTES,
            max_stdout_bytes: RESTRICTED_MAX_STDOUT_BYTES,
            max_stderr_bytes: RESTRICTED_MAX_STDERR_BYTES,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let mut config = Self::default();
        if let Ok(path) = std::env::var("AUTOHAND_CLI_PATH") {
            if !path.is_empty() {
                config.cli_path = Some(path.into());
            }
        }
        if let Ok(key) = std::env::var("AUTOHAND_AI_API_KEY") {
            if !key.is_empty() {
                config.provider = Some(ProviderName::AutohandAi);
                config.api_key = Some(key);
            }
        }
        config.base_url = std::env::var("AUTOHAND_AI_BASE_URL")
            .ok()
            .filter(|v| !v.is_empty());
        config.autohand_ai_plan = std::env::var("AUTOHAND_AI_PLAN")
            .ok()
            .filter(|v| !v.is_empty());
        config
    }

    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn with_cli_path(mut self, cli_path: impl Into<PathBuf>) -> Self {
        self.cli_path = Some(cli_path.into());
        self
    }

    pub fn with_answer_only_profile(mut self, profile: AnswerOnlyProfile) -> Self {
        self.runtime_profile = RuntimeProfile::AnswerOnly(profile);
        self.clear_environment = true;
        self.network_policy = ChildNetworkPolicy::DenyAll;
        self
    }

    pub fn with_setup_only_profile(mut self, profile: SetupOnlyProfile) -> Self {
        self.network_policy = ChildNetworkPolicy::SetupOnly(profile.traffic());
        self.runtime_profile = RuntimeProfile::SetupOnly(profile);
        self.clear_environment = true;
        self
    }

    pub fn allow_environment(mut self, name: impl Into<String>) -> Self {
        self.environment_allowlist.insert(name.into());
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        let instructions = instructions.into();
        self.append_system_prompt = Some(match self.append_system_prompt {
            Some(existing) if !existing.is_empty() => format!("{existing}\n\n{instructions}"),
            _ => instructions,
        });
        self
    }

    pub fn with_skill(mut self, skill: impl Into<String>) -> Self {
        self.skills.push(skill.into());
        self
    }

    pub(crate) fn cli_args(&self) -> Vec<String> {
        let mut args = vec!["--mode".to_string(), "rpc".to_string()];
        match self.runtime_profile {
            RuntimeProfile::Interactive => {}
            RuntimeProfile::AnswerOnly(_) => {
                args.extend([
                    "--answer-only".to_owned(),
                    "--restricted".to_owned(),
                    "--client-context".to_owned(),
                    "blueprint".to_owned(),
                ]);
            }
            RuntimeProfile::SetupOnly(_) => {
                args.extend([
                    "--setup-only".to_owned(),
                    "--restricted".to_owned(),
                    "--client-context".to_owned(),
                    "blueprint".to_owned(),
                ]);
            }
        }
        if let Some(cwd) = &self.cwd {
            args.push("--path".to_string());
            args.push(cwd.display().to_string());
        }
        push_flag(&mut args, self.bare, "--bare");
        push_flag(&mut args, self.unrestricted, "--unrestricted");
        push_flag(&mut args, self.auto_mode, "--auto-mode");
        push_flag(&mut args, self.auto_skill, "--auto-skill");
        push_flag(&mut args, self.auto_commit, "-c");
        if self.idle_logout == Some(false) {
            args.push("--no-idle-logout".into());
        }
        match self.context_compact {
            Some(true) => args.push("--context-compact".to_string()),
            Some(false) => args.push("--no-context-compact".to_string()),
            None => {}
        }
        push_flag(&mut args, self.persist_session, "--persist-session");
        push_value(&mut args, "--session-id", self.session_id.as_deref());
        push_flag(&mut args, self.resume, "--resume");
        push_flag(&mut args, self.continue_session, "--continue");
        push_flag(&mut args, self.fork, "--fork");
        push_value(
            &mut args,
            "--session-path",
            self.session_path
                .as_ref()
                .map(|p| p.to_string_lossy())
                .as_deref(),
        );
        push_value(&mut args, "--auto-save-interval", self.auto_save_interval);
        match self.agents_md {
            Some(true) => args.push("--agents-md".into()),
            Some(false) => args.push("--no-agents-md".into()),
            None => {}
        }
        push_flag(&mut args, self.agents_md_create, "--agents-md-create");
        push_value(
            &mut args,
            "--agents-md-path",
            self.agents_md_path
                .as_ref()
                .map(|p| p.to_string_lossy())
                .as_deref(),
        );
        push_flag(
            &mut args,
            self.agents_md_auto_update,
            "--agents-md-auto-update",
        );
        push_value(&mut args, "--max-tokens", self.max_tokens);
        push_value(
            &mut args,
            "--compression-threshold",
            self.compression_threshold,
        );
        push_value(
            &mut args,
            "--summarization-threshold",
            self.summarization_threshold,
        );
        if !self.skills.is_empty() {
            args.push("--skills".into());
            args.push(self.skills.join(","));
        }
        if !self.skill_sources.is_empty() {
            args.push("--skill-sources".into());
            args.push(self.skill_sources.join(","));
        }
        push_flag(
            &mut args,
            self.install_missing_skills,
            "--install-missing-skills",
        );
        push_value(&mut args, "--max-iterations", self.max_iterations);
        push_value(&mut args, "--max-runtime", self.max_runtime_minutes);
        push_value(&mut args, "--max-cost", self.max_cost);
        push_value(
            &mut args,
            "--display-language",
            self.display_language.as_deref(),
        );
        push_value(&mut args, "--sys-prompt", self.system_prompt.as_deref());
        push_value(
            &mut args,
            "--append-sys-prompt",
            self.append_system_prompt.as_deref(),
        );
        push_value(
            &mut args,
            "--system-prompt-file",
            self.system_prompt_file
                .as_ref()
                .map(|p| p.to_string_lossy())
                .as_deref(),
        );
        push_value(
            &mut args,
            "--append-system-prompt-file",
            self.append_system_prompt_file
                .as_ref()
                .map(|p| p.to_string_lossy())
                .as_deref(),
        );
        push_value(
            &mut args,
            "--mcp-config",
            self.mcp_config
                .as_ref()
                .map(|p| p.to_string_lossy())
                .as_deref(),
        );
        push_value(
            &mut args,
            "--agents",
            self.agents.as_ref().map(|p| p.to_string_lossy()).as_deref(),
        );
        push_value(
            &mut args,
            "--plugin-dir",
            self.plugin_dir
                .as_ref()
                .map(|p| p.to_string_lossy())
                .as_deref(),
        );
        push_value(&mut args, "--model", self.model.as_deref());
        push_value(&mut args, "--temperature", self.temperature);
        push_value(&mut args, "--yolo", self.yolo.as_deref());
        push_value(&mut args, "--yolo-timeout", self.yolo_timeout_seconds);
        for directory in &self.additional_directories {
            args.push("--add-dir".to_string());
            args.push(directory.display().to_string());
        }
        if self.runtime_profile == RuntimeProfile::Interactive {
            args.extend(self.extra_args.iter().cloned());
        }
        args
    }

    pub(crate) fn cli_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        if self.runtime_profile == RuntimeProfile::Interactive {
            env.insert("AUTOHAND_STREAM_TOOL_OUTPUT".into(), "1".into());
        }
        if self.provider == Some(ProviderName::AutohandAi) {
            env.insert(
                "AUTOHAND_AI_PLAN".into(),
                self.autohand_ai_plan
                    .clone()
                    .unwrap_or_else(|| "cloud".into()),
            );
            if let Some(value) = &self.api_key {
                env.insert("AUTOHAND_AI_API_KEY".into(), value.clone());
            }
            if let Some(value) = &self.base_url {
                env.insert("AUTOHAND_AI_BASE_URL".into(), value.clone());
            }
        }
        env.extend(self.env.clone());
        env
    }

    pub(crate) fn inherited_environment(&self) -> BTreeMap<String, String> {
        let mut names = self.environment_allowlist.clone();
        if self.clear_environment {
            for name in minimal_environment_names() {
                names.insert((*name).to_owned());
            }
        }
        names
            .into_iter()
            .filter_map(|name| std::env::var(&name).ok().map(|value| (name, value)))
            .collect()
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        match self.runtime_profile {
            RuntimeProfile::Interactive => {
                if self.network_policy != ChildNetworkPolicy::Inherit {
                    return Err(
                        "interactive profile requires ChildNetworkPolicy::Inherit".to_owned()
                    );
                }
            }
            RuntimeProfile::AnswerOnly(_) => {
                if !self.clear_environment {
                    return Err("answer-only profile must clear the child environment".to_owned());
                }
                if self.network_policy != ChildNetworkPolicy::DenyAll {
                    return Err("answer-only profile requires deny-all child egress".to_owned());
                }
                self.validate_restricted_profile("answer-only")?;
            }
            RuntimeProfile::SetupOnly(profile) => {
                if !self.clear_environment {
                    return Err("setup-only profile must clear the child environment".to_owned());
                }
                if self.network_policy != ChildNetworkPolicy::SetupOnly(profile.traffic()) {
                    return Err(
                        "setup-only profile requires the device-authorization network policy"
                            .to_owned(),
                    );
                }
                self.validate_restricted_profile("setup-only")?;
            }
        }
        if self.max_events == 0
            || self.max_event_bytes == 0
            || self.max_stdout_bytes == 0
            || self.max_stderr_bytes == 0
        {
            return Err("output limits must all be greater than zero".to_owned());
        }
        Ok(())
    }

    fn validate_restricted_profile(&self, profile: &str) -> Result<(), String> {
        if !self.extra_args.is_empty() {
            return Err(format!("{profile} does not accept extra_args"));
        }
        if self.cwd.is_some()
            || self.debug
            || self.unrestricted
            || self.bare
            || self.idle_logout.is_some()
            || self.auto_mode
            || self.auto_skill
            || self.auto_commit
            || self.context_compact.is_some()
            || self.persist_session
            || self.session_id.is_some()
            || self.resume
            || self.continue_session
            || self.fork
            || self.session_path.is_some()
            || self.auto_save_interval.is_some()
            || self.agents_md.is_some()
            || self.agents_md_create
            || self.agents_md_path.is_some()
            || self.agents_md_auto_update
            || self.max_tokens.is_some()
            || self.compression_threshold.is_some()
            || self.summarization_threshold.is_some()
            || self.max_iterations.is_some()
            || self.max_runtime_minutes.is_some()
            || self.max_cost.is_some()
            || self.model.is_some()
            || self.temperature.is_some()
            || self.system_prompt.is_some()
            || self.append_system_prompt.is_some()
            || self.system_prompt_file.is_some()
            || self.append_system_prompt_file.is_some()
            || self.display_language.is_some()
            || self.mcp_config.is_some()
            || self.agents.is_some()
            || self.plugin_dir.is_some()
            || self.yolo.is_some()
            || self.yolo_timeout_seconds.is_some()
            || !self.additional_directories.is_empty()
            || !self.skills.is_empty()
            || !self.skill_sources.is_empty()
            || self.install_missing_skills
            || self.provider.is_some()
            || self.api_key.is_some()
            || self.base_url.is_some()
            || self.autohand_ai_plan.is_some()
            || self.features.is_some()
            || !self.env.is_empty()
        {
            return Err(format!(
                "{profile} accepts only its closed profile, CLI path, timeout, output limits, and explicit environment allowlist"
            ));
        }
        if self.environment_allowlist.iter().any(|name| {
            name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        }) {
            return Err(format!(
                "{profile} environment allowlist contains an invalid variable name"
            ));
        }
        if let Some(name) = self
            .environment_allowlist
            .iter()
            .find(|name| restricted_environment_name_is_forbidden(name))
        {
            return Err(format!(
                "{profile} environment allowlist cannot restore behavior-changing variable {name}"
            ));
        }
        if self.max_events > RESTRICTED_MAX_EVENTS
            || self.max_event_bytes > RESTRICTED_MAX_EVENT_BYTES
            || self.max_stdout_bytes > RESTRICTED_MAX_STDOUT_BYTES
            || self.max_stderr_bytes > RESTRICTED_MAX_STDERR_BYTES
        {
            return Err(format!(
                "{profile} output limits may be tightened but not relaxed"
            ));
        }
        Ok(())
    }
}

fn restricted_environment_name_is_forbidden(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    name.starts_with("AUTOHAND_")
        || matches!(
            name.as_str(),
            "ALL_PROXY"
                | "HTTP_PROXY"
                | "HTTPS_PROXY"
                | "NO_PROXY"
                | "BUN_OPTIONS"
                | "NODE_OPTIONS"
                | "NODE_EXTRA_CA_CERTS"
                | "NODE_TLS_REJECT_UNAUTHORIZED"
                | "SSL_CERT_DIR"
                | "SSL_CERT_FILE"
                | "SSLKEYLOGFILE"
                | "CURL_CA_BUNDLE"
                | "LD_PRELOAD"
                | "LD_AUDIT"
                | "LD_LIBRARY_PATH"
                | "DYLD_INSERT_LIBRARIES"
                | "DYLD_FRAMEWORK_PATH"
                | "DYLD_FALLBACK_FRAMEWORK_PATH"
                | "DYLD_FALLBACK_LIBRARY_PATH"
                | "DYLD_LIBRARY_PATH"
        )
}

#[cfg(unix)]
fn minimal_environment_names() -> &'static [&'static str] {
    &["HOME", "PATH", "TMPDIR"]
}

#[cfg(windows)]
fn minimal_environment_names() -> &'static [&'static str] {
    &[
        "APPDATA",
        "LOCALAPPDATA",
        "PATH",
        "SYSTEMROOT",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "WINDIR",
    ]
}

#[derive(Debug, Clone, Default)]
pub struct PromptOptions {
    pub context: Option<Value>,
    pub images: Vec<Value>,
    pub thinking_level: Option<String>,
    pub extra: Map<String, Value>,
    /// Host predicates, evaluated with OR semantics after each persisted tool step.
    pub stop_when: Vec<crate::StopCondition>,
}

impl PromptOptions {
    pub(crate) fn to_params(&self, message: impl Into<String>) -> Value {
        let mut params = Map::new();
        params.insert("message".to_string(), Value::String(message.into()));
        if let Some(context) = &self.context {
            params.insert("context".to_string(), context.clone());
        }
        if !self.images.is_empty() {
            params.insert("images".to_string(), Value::Array(self.images.clone()));
        }
        if let Some(thinking_level) = &self.thinking_level {
            params.insert(
                "thinkingLevel".to_string(),
                Value::String(thinking_level.clone()),
            );
        }
        for (key, value) in &self.extra {
            params.insert(key.clone(), value.clone());
        }
        if !self.stop_when.is_empty() {
            params.insert("stopWhen".into(), serde_json::json!({"mode": "host"}));
        }
        Value::Object(params)
    }
}

fn push_flag(args: &mut Vec<String>, enabled: bool, flag: &str) {
    if enabled {
        args.push(flag.to_string());
    }
}

fn push_value<T: ToString>(args: &mut Vec<String>, flag: &str, value: Option<T>) {
    if let Some(value) = value {
        args.push(flag.to_string());
        args.push(value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ProviderName};

    #[test]
    fn builds_cli_args_from_options() {
        let mut config = Config::default()
            .with_model("fantail2")
            .with_skill("rust")
            .with_instructions("Prefer small Rust modules.")
            .with_cwd("/tmp/autohand-rust-sdk");
        config.unrestricted = true;
        config.context_compact = Some(false);

        let args = config.cli_args();
        assert!(args.contains(&"--mode".to_string()));
        assert!(args.contains(&"rpc".to_string()));
        assert!(args
            .windows(2)
            .any(|pair| { pair == ["--path".to_string(), "/tmp/autohand-rust-sdk".to_string()] }));
        assert!(args.contains(&"--unrestricted".to_string()));
        assert!(args.contains(&"--no-context-compact".to_string()));
        assert!(args.contains(&"fantail2".to_string()));
        assert!(args.contains(&"rust".to_string()));
        assert!(args.contains(&"Prefer small Rust modules.".to_string()));
    }

    #[test]
    fn builds_current_runtime_flags_and_autohand_ai_environment() {
        let mut config = Config {
            bare: true,
            idle_logout: Some(false),
            fork: true,
            agents_md: Some(false),
            skill_sources: vec!["user".into(), "project".into()],
            install_missing_skills: true,
            display_language: Some("pt-BR".into()),
            system_prompt_file: Some("system.md".into()),
            append_system_prompt_file: Some("append.md".into()),
            mcp_config: Some("mcp.json".into()),
            agents: Some("agents.json".into()),
            plugin_dir: Some("plugins".into()),
            provider: Some(ProviderName::AutohandAi),
            api_key: Some("key".into()),
            base_url: Some("https://api".into()),
            ..Default::default()
        };
        let args = config.cli_args();
        for expected in [
            "--bare",
            "--no-idle-logout",
            "--fork",
            "--no-agents-md",
            "--skill-sources",
            "user,project",
            "--install-missing-skills",
            "--display-language",
            "pt-BR",
            "--system-prompt-file",
            "system.md",
            "--append-system-prompt-file",
            "append.md",
            "--mcp-config",
            "mcp.json",
            "--agents",
            "agents.json",
            "--plugin-dir",
            "plugins",
        ] {
            assert!(
                args.contains(&expected.into()),
                "missing {expected}: {args:?}"
            );
        }
        config.env.insert("AUTOHAND_AI_PLAN".into(), "max".into());
        let env = config.cli_env();
        assert_eq!(
            env.get("AUTOHAND_AI_API_KEY").map(String::as_str),
            Some("key")
        );
        assert_eq!(
            env.get("AUTOHAND_AI_BASE_URL").map(String::as_str),
            Some("https://api")
        );
        assert_eq!(env.get("AUTOHAND_AI_PLAN").map(String::as_str), Some("max"));
    }
}
