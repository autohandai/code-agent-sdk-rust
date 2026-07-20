mod agent;
mod autoresearch;
mod command;
mod config;
mod discovery;
mod error;
mod event;
mod goal;
mod json_output;
mod transport;

pub use agent::{Agent, JsonRunOptions, Run, RunResult};
pub use autoresearch::*;
pub use command::format_slash_command;
pub use config::{Config, FeatureFlagSettings, PromptOptions, ProviderName};
pub use discovery::*;
pub use error::{Error, Result};
pub use event::{SdkEvent, TokenUsageStatus, TurnEndUsage};
pub use goal::*;
pub use json_output::{parse_json_text, StructuredOutputError};
pub use transport::AutohandSdk;

/// Performs idempotent eager initialization of the public SDK runtime.
pub fn initialize() {
    static SDK_RUNTIME: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    SDK_RUNTIME.get_or_init(|| ());
}
