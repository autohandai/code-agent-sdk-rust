use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct Config {
    pub cwd: Option<PathBuf>,
    pub cli_path: Option<PathBuf>,
    pub debug: bool,
    pub timeout: Duration,
    pub unrestricted: bool,
    pub auto_mode: bool,
    pub auto_skill: bool,
    pub auto_commit: bool,
    pub context_compact: Option<bool>,
    pub max_iterations: Option<u32>,
    pub max_runtime_minutes: Option<u32>,
    pub max_cost: Option<f64>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub yolo: Option<String>,
    pub yolo_timeout_seconds: Option<u32>,
    pub additional_directories: Vec<PathBuf>,
    pub skills: Vec<String>,
    pub extra_args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cwd: None,
            cli_path: None,
            debug: false,
            timeout: Duration::from_secs(300),
            unrestricted: false,
            auto_mode: false,
            auto_skill: false,
            auto_commit: false,
            context_compact: None,
            max_iterations: None,
            max_runtime_minutes: None,
            max_cost: None,
            model: None,
            temperature: None,
            system_prompt: None,
            append_system_prompt: None,
            yolo: None,
            yolo_timeout_seconds: None,
            additional_directories: Vec::new(),
            skills: Vec::new(),
            extra_args: Vec::new(),
            env: BTreeMap::new(),
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
        push_flag(&mut args, self.unrestricted, "--unrestricted");
        push_flag(&mut args, self.auto_mode, "--auto-mode");
        push_flag(&mut args, self.auto_skill, "--auto-skill");
        push_flag(&mut args, self.auto_commit, "-c");
        match self.context_compact {
            Some(true) => args.push("--context-compact".to_string()),
            Some(false) => args.push("--no-context-compact".to_string()),
            None => {}
        }
        push_value(&mut args, "--max-iterations", self.max_iterations);
        push_value(&mut args, "--max-runtime", self.max_runtime_minutes);
        push_value(&mut args, "--max-cost", self.max_cost);
        push_value(&mut args, "--model", self.model.as_deref());
        push_value(&mut args, "--temperature", self.temperature);
        push_value(&mut args, "--sys-prompt", self.system_prompt.as_deref());
        push_value(
            &mut args,
            "--append-sys-prompt",
            self.append_system_prompt.as_deref(),
        );
        push_value(&mut args, "--yolo", self.yolo.as_deref());
        push_value(&mut args, "--yolo-timeout", self.yolo_timeout_seconds);
        if !self.skills.is_empty() {
            args.push("--skills".to_string());
            args.push(self.skills.join(","));
        }
        for directory in &self.additional_directories {
            args.push("--add-dir".to_string());
            args.push(directory.display().to_string());
        }
        args.extend(self.extra_args.iter().cloned());
        args
    }
}

#[derive(Debug, Clone, Default)]
pub struct PromptOptions {
    pub context: Option<Value>,
    pub images: Vec<Value>,
    pub thinking_level: Option<String>,
    pub extra: Map<String, Value>,
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
    use super::Config;

    #[test]
    fn builds_cli_args_from_options() {
        let mut config = Config::default()
            .with_model("fantail2")
            .with_skill("rust")
            .with_instructions("Prefer small Rust modules.");
        config.unrestricted = true;
        config.context_compact = Some(false);

        let args = config.cli_args();
        assert!(args.contains(&"--mode".to_string()));
        assert!(args.contains(&"rpc".to_string()));
        assert!(args.contains(&"--unrestricted".to_string()));
        assert!(args.contains(&"--no-context-compact".to_string()));
        assert!(args.contains(&"fantail2".to_string()));
        assert!(args.contains(&"rust".to_string()));
        assert!(args.contains(&"Prefer small Rust modules.".to_string()));
    }
}
