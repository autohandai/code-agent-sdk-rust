use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

use crate::{Error, Result};

pub const BLUEPRINT_ANSWER_CONTRACT_VERSION: u16 = 1;
pub const AUTOHAND_LOGIN_CONTRACT_VERSION: u16 = 1;
pub const BLUEPRINT_ANSWER_MAX_INPUT_BYTES: usize = 8 * 1024;
pub const BLUEPRINT_ANSWER_MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintArtifactClass {
    Code,
    SourceSnippet,
    Symbol,
    RepositoryPath,
    Comment,
    Diff,
    Lineage,
    Rationale,
    DesignRecord,
    DocumentChunk,
    MediaChunk,
    BinaryMedia,
    Credential,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClassifiedArtifact {
    pub id: String,
    pub class: BlueprintArtifactClass,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClassifiedAnswerEnvelope {
    pub contract_version: u16,
    pub policy_hash: String,
    pub artifacts: Vec<ClassifiedArtifact>,
    pub output_schema: Value,
}

impl ClassifiedAnswerEnvelope {
    pub fn new(
        policy_hash: impl Into<String>,
        artifacts: Vec<ClassifiedArtifact>,
        schema: &StrictJsonSchema,
    ) -> Result<Self> {
        let envelope = Self {
            contract_version: BLUEPRINT_ANSWER_CONTRACT_VERSION,
            policy_hash: policy_hash.into(),
            artifacts,
            output_schema: schema.as_value().clone(),
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub(crate) fn validate(&self) -> Result<()> {
        if self.contract_version != BLUEPRINT_ANSWER_CONTRACT_VERSION {
            return Err(Error::UnsupportedContractVersion {
                expected: BLUEPRINT_ANSWER_CONTRACT_VERSION,
                observed: self.contract_version,
            });
        }
        if !is_sha256(&self.policy_hash) {
            return Err(Error::InvalidInput(
                "policy_hash must be a lowercase SHA-256 digest".to_owned(),
            ));
        }
        if self.artifacts.is_empty() || self.artifacts.len() > 64 {
            return Err(Error::InvalidInput(
                "classified answer envelope requires 1 to 64 artifacts".to_owned(),
            ));
        }
        let mut artifact_ids = BTreeSet::new();
        for artifact in &self.artifacts {
            if artifact.id.is_empty()
                || artifact.id.len() > 128
                || !artifact.id.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                })
            {
                return Err(Error::InvalidInput(
                    "classified artifact id violates contract version 1".to_owned(),
                ));
            }
            if artifact.content.is_empty() || artifact.content.chars().count() > 8192 {
                return Err(Error::InvalidInput(
                    "classified artifact content violates contract version 1".to_owned(),
                ));
            }
            if !artifact_ids.insert(artifact.id.as_str()) {
                return Err(Error::InvalidInput(
                    "classified artifact ids must be unique".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrictJsonSchema(Value);

impl StrictJsonSchema {
    pub fn new(schema: Value) -> Result<Self> {
        let root = schema.as_object().ok_or_else(|| {
            Error::InvalidInput("strict JSON output schema must be an object".to_owned())
        })?;
        if root.get("type").and_then(Value::as_str) != Some("object")
            || !root.contains_key("required")
        {
            return Err(Error::InvalidInput(
                "strict JSON output schema requires an object root and required array".to_owned(),
            ));
        }
        validate_schema_definition(&schema, "$")?;
        Ok(Self(schema))
    }

    pub fn as_value(&self) -> &Value {
        &self.0
    }

    pub fn validate(&self, value: &Value) -> Result<()> {
        validate_json_value(&self.0, value, "$")
    }
}

impl TryFrom<Value> for StrictJsonSchema {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuredRunLimits {
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
}

impl Default for StructuredRunLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: BLUEPRINT_ANSWER_MAX_INPUT_BYTES,
            max_output_bytes: BLUEPRINT_ANSWER_MAX_OUTPUT_BYTES,
        }
    }
}

impl StructuredRunLimits {
    pub(crate) fn validate(self) -> Result<()> {
        if self.max_input_bytes > BLUEPRINT_ANSWER_MAX_INPUT_BYTES
            || self.max_output_bytes > BLUEPRINT_ANSWER_MAX_OUTPUT_BYTES
        {
            return Err(Error::InvalidInput(
                "structured run limits may tighten but not relax answer contract version 1"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticationState {
    NotRequired,
    Configured,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InferenceDestination {
    InProcess {
        #[serde(default)]
        provider: Option<String>,
    },
    LocalSubprocess {
        provider: String,
    },
    LocalService {
        provider: String,
        origin: String,
    },
    Hosted {
        provider: String,
        #[serde(default)]
        origin: Option<String>,
    },
    Opaque,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliSymlinkIdentity {
    pub path: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliPackageIdentity {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliArtifactIdentity {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CliIdentity {
    pub invocation_path: String,
    pub resolved_path: String,
    pub symlink_chain: Vec<CliSymlinkIdentity>,
    pub package: CliPackageIdentity,
    pub artifacts: Vec<CliArtifactIdentity>,
    pub identity_hash: String,
}

impl CliIdentity {
    fn validate(&self) -> Result<()> {
        if self.invocation_path.trim().is_empty()
            || self.resolved_path.trim().is_empty()
            || self.package.name.trim().is_empty()
            || self.package.version.trim().is_empty()
            || self
                .package
                .commit
                .as_ref()
                .is_some_and(|commit| commit.trim().is_empty())
            || self.artifacts.is_empty()
        {
            return Err(Error::Protocol(
                "CLI identity is missing required execution facts".to_owned(),
            ));
        }
        if !is_sha256(&self.identity_hash)
            || self
                .artifacts
                .iter()
                .any(|artifact| artifact.path.trim().is_empty() || !is_sha256(&artifact.sha256))
        {
            return Err(Error::Protocol(
                "CLI identity contains an invalid SHA-256 digest".to_owned(),
            ));
        }
        if self
            .symlink_chain
            .iter()
            .any(|hop| hop.path.trim().is_empty() || hop.target.trim().is_empty())
        {
            return Err(Error::Protocol(
                "CLI identity contains an invalid symlink hop".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerOnlyCapabilities {
    pub client_context: crate::ClientContext,
    pub answer_only: bool,
    pub restricted: bool,
    pub tools_enabled: bool,
    pub hooks_enabled: bool,
    pub mcp_enabled: bool,
    pub memory_enabled: bool,
    pub session_persistence_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFacts {
    pub cli_version: String,
    pub answer_contract_version: u16,
    pub cli_identity: CliIdentity,
    pub provider_id: String,
    pub model: Option<String>,
    pub authentication: AuthenticationState,
    pub inference_destination: InferenceDestination,
    pub capabilities: AnswerOnlyCapabilities,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RuntimeFactsWire {
    cli_version: String,
    answer_contract_version: u16,
    cli_identity: CliIdentity,
    provider_id: String,
    #[serde(default)]
    model: Option<String>,
    authentication: AuthenticationState,
    client_context: crate::ClientContext,
    answer_only: bool,
    permission_mode: RestrictedPermissionMode,
    tools_enabled: bool,
    hooks_enabled: bool,
    mcp_enabled: bool,
    memory_enabled: bool,
    session_persistence_enabled: bool,
    inference_destination: InferenceDestination,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RestrictedPermissionMode {
    Restricted,
}

impl<'de> Deserialize<'de> for RuntimeFacts {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = RuntimeFactsWire::deserialize(deserializer)?;
        let _ = wire.permission_mode;
        Ok(Self {
            cli_version: wire.cli_version,
            answer_contract_version: wire.answer_contract_version,
            cli_identity: wire.cli_identity,
            provider_id: wire.provider_id,
            model: wire.model,
            authentication: wire.authentication,
            inference_destination: wire.inference_destination,
            capabilities: AnswerOnlyCapabilities {
                client_context: wire.client_context,
                answer_only: wire.answer_only,
                restricted: true,
                tools_enabled: wire.tools_enabled,
                hooks_enabled: wire.hooks_enabled,
                mcp_enabled: wire.mcp_enabled,
                memory_enabled: wire.memory_enabled,
                session_persistence_enabled: wire.session_persistence_enabled,
            },
        })
    }
}

impl RuntimeFacts {
    pub(crate) fn validate_answer_only(&self) -> Result<()> {
        if self.answer_contract_version != BLUEPRINT_ANSWER_CONTRACT_VERSION {
            return Err(Error::UnsupportedContractVersion {
                expected: BLUEPRINT_ANSWER_CONTRACT_VERSION,
                observed: self.answer_contract_version,
            });
        }
        self.cli_identity.validate()?;
        let capabilities = &self.capabilities;
        if capabilities.client_context != crate::ClientContext::Blueprint
            || !capabilities.answer_only
            || !capabilities.restricted
            || capabilities.tools_enabled
            || capabilities.hooks_enabled
            || capabilities.mcp_enabled
            || capabilities.memory_enabled
            || capabilities.session_persistence_enabled
        {
            return Err(Error::Protocol(
                "CLI did not enforce the closed Blueprint answer-only profile".to_owned(),
            ));
        }
        if self.cli_version.trim().is_empty()
            || self.provider_id.trim().is_empty()
            || self
                .model
                .as_ref()
                .is_some_and(|model| model.trim().is_empty())
        {
            return Err(Error::Protocol(
                "runtime facts omitted a required runtime identity field".to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_answer_destination(&self) -> Result<()> {
        let destination = match &self.inference_destination {
            InferenceDestination::InProcess { .. }
            | InferenceDestination::LocalSubprocess { .. } => return Ok(()),
            InferenceDestination::LocalService { .. } => "local_service",
            InferenceDestination::Hosted { .. } => "hosted",
            InferenceDestination::Opaque => "opaque",
        };
        Err(Error::InferenceDestinationBlocked { destination })
    }
}

#[derive(Debug)]
pub struct StructuredAnswerRun<T> {
    pub result: T,
    pub provider_id: String,
    pub model: Option<String>,
    pub inference_destination: InferenceDestination,
    pub runtime_facts: RuntimeFacts,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct AnswerRpcResult {
    pub contract_version: u16,
    pub result: Value,
    pub provider_id: String,
    #[serde(default)]
    pub model: Option<String>,
    pub inference_destination: InferenceDestination,
}

pub struct LoginSession {
    pub(crate) id: String,
    pub(crate) sdk: crate::AutohandSdk,
    terminal: std::sync::atomic::AtomicBool,
}

impl std::fmt::Debug for LoginSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoginSession")
            .field("state", &"opaque")
            .finish()
    }
}

impl LoginSession {
    pub(crate) fn new(id: String, sdk: crate::AutohandSdk) -> Self {
        Self {
            id,
            sdk,
            terminal: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub(crate) fn finish(&self) {
        self.terminal
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for LoginSession {
    fn drop(&mut self) {
        if !self.terminal.load(std::sync::atomic::Ordering::SeqCst) {
            self.sdk.abort_process_tree_now();
        }
    }
}

#[derive(Debug)]
pub struct LoginChallenge {
    pub session: LoginSession,
    pub user_code: String,
    pub verification_uri_complete: String,
    pub expires_at_unix_ms: u64,
    pub poll_after_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginProblemCode {
    AdapterUnavailable,
    NetworkDenied,
    InitiationFailed,
    InvalidChallenge,
    RateLimited,
    PollFailed,
    CancelFailed,
    CleanupFailed,
    CredentialPersistenceFailed,
    ProtocolMismatch,
}

impl LoginProblemCode {
    pub(crate) fn from_rpc_kind(kind: &str) -> Option<Self> {
        Some(match kind {
            "adapter_unavailable" => Self::AdapterUnavailable,
            "network_denied" => Self::NetworkDenied,
            "initiation_failed" => Self::InitiationFailed,
            "invalid_challenge" | "invalid_auth_challenge" => Self::InvalidChallenge,
            "rate_limited" => Self::RateLimited,
            "poll_failed" => Self::PollFailed,
            "cancel_failed" => Self::CancelFailed,
            "cleanup_failed" => Self::CleanupFailed,
            "credential_persistence_failed" => Self::CredentialPersistenceFailed,
            "protocol_mismatch" => Self::ProtocolMismatch,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginProblem {
    pub code: LoginProblemCode,
    pub message: String,
    pub retryable: bool,
}

impl std::fmt::Display for LoginProblem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl LoginProblem {
    pub(crate) fn from_rpc(
        code: LoginProblemCode,
        message: String,
        retryable: bool,
    ) -> Result<Self> {
        let problem = Self {
            code,
            message,
            retryable,
        };
        validate_login_problem(&problem)?;
        Ok(problem)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginStatus {
    Pending { poll_after_ms: u64 },
    Authorized,
    Expired,
    Cancelled,
    Failed { problem: LoginProblem },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LoginChallengeWire {
    pub contract_version: u16,
    pub session_id: String,
    pub user_code: String,
    pub verification_uri_complete: String,
    pub expires_at_unix_ms: u64,
    pub poll_after_ms: u64,
}

impl LoginChallengeWire {
    pub(crate) fn validate(self, sdk: crate::AutohandSdk) -> Result<LoginChallenge> {
        validate_login_contract(self.contract_version)?;
        if self.session_id.len() != 32 || !is_lowercase_hex(&self.session_id) {
            return Err(Error::Protocol(
                "Autohand login challenge session ID violates contract version 1".to_owned(),
            ));
        }
        if self.user_code.trim().is_empty()
            || self.user_code.chars().count() > 32
            || self.user_code.chars().any(char::is_control)
        {
            return Err(Error::Protocol(
                "Autohand login challenge user code violates contract version 1".to_owned(),
            ));
        }
        if self.verification_uri_complete.len() > 2048
            || self.expires_at_unix_ms == 0
            || !(1_000..=30_000).contains(&self.poll_after_ms)
        {
            return Err(Error::Protocol(
                "Autohand login challenge bounds violate contract version 1".to_owned(),
            ));
        }
        validate_verification_uri(&self.verification_uri_complete, &self.user_code)?;
        Ok(LoginChallenge {
            session: LoginSession::new(self.session_id, sdk),
            user_code: self.user_code,
            verification_uri_complete: self.verification_uri_complete,
            expires_at_unix_ms: self.expires_at_unix_ms,
            poll_after_ms: self.poll_after_ms,
        })
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum LoginStatusWire {
    Pending {
        contract_version: u16,
        poll_after_ms: u64,
    },
    Authorized {
        contract_version: u16,
    },
    Expired {
        contract_version: u16,
    },
    Cancelled {
        contract_version: u16,
    },
    Failed {
        contract_version: u16,
        problem: LoginProblem,
    },
}

impl LoginStatusWire {
    pub(crate) fn validate(self) -> Result<LoginStatus> {
        let (version, status) = match self {
            Self::Pending {
                contract_version,
                poll_after_ms,
            } if (1_000..=30_000).contains(&poll_after_ms) => {
                (contract_version, LoginStatus::Pending { poll_after_ms })
            }
            Self::Pending { .. } => {
                return Err(Error::Protocol(
                    "pending login status poll interval violates contract version 1".to_owned(),
                ));
            }
            Self::Authorized { contract_version } => (contract_version, LoginStatus::Authorized),
            Self::Expired { contract_version } => (contract_version, LoginStatus::Expired),
            Self::Cancelled { contract_version } => (contract_version, LoginStatus::Cancelled),
            Self::Failed {
                contract_version,
                problem,
            } => {
                validate_login_problem(&problem)?;
                (contract_version, LoginStatus::Failed { problem })
            }
        };
        validate_login_contract(version)?;
        Ok(status)
    }
}

fn validate_login_contract(observed: u16) -> Result<()> {
    if observed != AUTOHAND_LOGIN_CONTRACT_VERSION {
        return Err(Error::UnsupportedLoginContractVersion {
            expected: AUTOHAND_LOGIN_CONTRACT_VERSION,
            observed,
        });
    }
    Ok(())
}

fn validate_login_problem(problem: &LoginProblem) -> Result<()> {
    if problem.message.trim().is_empty()
        || problem.message.chars().count() > 256
        || problem.message.chars().any(char::is_control)
    {
        return Err(Error::Protocol(
            "login failure contained an unsafe public message".to_owned(),
        ));
    }
    Ok(())
}

fn validate_verification_uri(uri: &str, user_code: &str) -> Result<()> {
    const PREFIX: &str = "https://autohand.ai/signin?";
    let query = uri.strip_prefix(PREFIX).ok_or_else(|| {
        Error::Protocol("verificationUriComplete must use https://autohand.ai/signin".to_owned())
    })?;
    if query.is_empty() || query.contains('#') {
        return Err(Error::Protocol(
            "verificationUriComplete requires a query and no fragment".to_owned(),
        ));
    }
    let mut continuation = None;
    let mut observed_user_code = None;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').ok_or_else(|| {
            Error::Protocol("verificationUriComplete contains an invalid query pair".to_owned())
        })?;
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(Error::Protocol(
                "verificationUriComplete contains an unsafe query value".to_owned(),
            ));
        }
        match key {
            "continue" if continuation.is_none() => continuation = Some(value),
            "user_code" if observed_user_code.is_none() => observed_user_code = Some(value),
            _ => {
                return Err(Error::Protocol(
                    "verificationUriComplete contains an unapproved query key".to_owned(),
                ));
            }
        }
    }
    if continuation.is_none() || observed_user_code != Some(user_code) {
        return Err(Error::Protocol(
            "verificationUriComplete does not bind the returned user code".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn decode_answer<T: DeserializeOwned>(
    response: AnswerRpcResult,
    runtime_facts: RuntimeFacts,
    schema: &StrictJsonSchema,
    limits: StructuredRunLimits,
) -> Result<StructuredAnswerRun<T>> {
    if response.contract_version != BLUEPRINT_ANSWER_CONTRACT_VERSION {
        return Err(Error::UnsupportedContractVersion {
            expected: BLUEPRINT_ANSWER_CONTRACT_VERSION,
            observed: response.contract_version,
        });
    }
    if response.provider_id != runtime_facts.provider_id
        || response.model != runtime_facts.model
        || response.inference_destination != runtime_facts.inference_destination
    {
        return Err(Error::Protocol(
            "answer provenance does not match inspected runtime facts".to_owned(),
        ));
    }
    let output_bytes = serde_json::to_vec(&response.result)?;
    if output_bytes.len() > limits.max_output_bytes {
        return Err(Error::OutputLimitExceeded {
            stream: "structured answer",
            limit: limits.max_output_bytes,
        });
    }
    schema.validate(&response.result)?;
    let result = serde_json::from_value(response.result)?;
    Ok(StructuredAnswerRun {
        result,
        provider_id: response.provider_id,
        model: response.model,
        inference_destination: response.inference_destination,
        runtime_facts,
    })
}

fn validate_schema_definition(schema: &Value, path: &str) -> Result<()> {
    let object = schema.as_object().ok_or_else(|| {
        Error::InvalidInput(format!("strict JSON schema at {path} must be an object"))
    })?;
    if object.contains_key("type") && !object.get("type").is_some_and(Value::is_string) {
        return Err(Error::InvalidInput(format!(
            "type at {path} must be a supported string"
        )));
    }
    if let Some(values) = object.get("enum") {
        let values = values.as_array().ok_or_else(|| {
            Error::InvalidInput(format!("enum at {path} must be a non-empty array"))
        })?;
        if values.is_empty() {
            return Err(Error::InvalidInput(format!(
                "enum at {path} must be a non-empty array"
            )));
        }
    }
    let schema_type = object.get("type").and_then(Value::as_str);
    if schema_type.is_none() && object.get("enum").and_then(Value::as_array).is_some() {
        validate_schema_keywords(object, &["enum"], path)?;
        return Ok(());
    }
    let schema_type = schema_type.ok_or_else(|| {
        Error::InvalidInput(format!(
            "strict JSON schema at {path} requires a type or enum"
        ))
    })?;
    match schema_type {
        "object" => {
            validate_schema_keywords(
                object,
                &[
                    "type",
                    "enum",
                    "properties",
                    "required",
                    "additionalProperties",
                ],
                path,
            )?;
            if object.get("additionalProperties") != Some(&Value::Bool(false)) {
                return Err(Error::InvalidInput(format!(
                    "object schema at {path} must set additionalProperties to false"
                )));
            }
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    Error::InvalidInput(format!("object schema at {path} requires properties"))
                })?;
            if properties.len() > 128 {
                return Err(Error::InvalidInput(format!(
                    "object schema at {path} exceeds 128 properties"
                )));
            }
            for (name, property) in properties {
                validate_schema_definition(property, &format!("{path}.{name}"))?;
            }
            if let Some(required) = object.get("required") {
                let required = required.as_array().ok_or_else(|| {
                    Error::InvalidInput(format!("required at {path} must be an array"))
                })?;
                if required.len() > 128 {
                    return Err(Error::InvalidInput(format!(
                        "required at {path} exceeds 128 entries"
                    )));
                }
                let mut unique = BTreeSet::new();
                for name in required {
                    let name = name.as_str().ok_or_else(|| {
                        Error::InvalidInput(format!("required at {path} must contain strings"))
                    })?;
                    if !unique.insert(name) {
                        return Err(Error::InvalidInput(format!(
                            "required at {path} must contain unique names"
                        )));
                    }
                    if !properties.contains_key(name) {
                        return Err(Error::InvalidInput(format!(
                            "required property {path}.{name} is not declared"
                        )));
                    }
                }
            }
        }
        "array" => {
            validate_schema_keywords(
                object,
                &["type", "enum", "items", "minItems", "maxItems"],
                path,
            )?;
            let items = object.get("items").ok_or_else(|| {
                Error::InvalidInput(format!("array schema at {path} requires items"))
            })?;
            validate_schema_definition(items, &format!("{path}[]"))?;
            validate_u64_bounds(object, "minItems", "maxItems", path)?;
        }
        "string" => {
            validate_schema_keywords(object, &["type", "enum", "minLength", "maxLength"], path)?;
            validate_u64_bounds(object, "minLength", "maxLength", path)?;
        }
        "integer" | "number" | "boolean" | "null" => {
            validate_schema_keywords(object, &["type", "enum"], path)?;
        }
        other => {
            return Err(Error::InvalidInput(format!(
                "unsupported strict JSON schema type {other} at {path}"
            )));
        }
    }
    Ok(())
}

fn validate_schema_keywords(
    object: &serde_json::Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<()> {
    if let Some(keyword) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(Error::InvalidInput(format!(
            "unsupported strict JSON schema keyword {keyword} at {path}"
        )));
    }
    Ok(())
}

fn validate_u64_bounds(
    object: &serde_json::Map<String, Value>,
    minimum_name: &str,
    maximum_name: &str,
    path: &str,
) -> Result<()> {
    let minimum = object
        .get(minimum_name)
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                Error::InvalidInput(format!("{minimum_name} at {path} must be an integer"))
            })
        })
        .transpose()?;
    let maximum = object
        .get(maximum_name)
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                Error::InvalidInput(format!("{maximum_name} at {path} must be an integer"))
            })
        })
        .transpose()?;
    if matches!((minimum, maximum), (Some(minimum), Some(maximum)) if minimum > maximum) {
        return Err(Error::InvalidInput(format!(
            "{minimum_name} exceeds {maximum_name} at {path}"
        )));
    }
    Ok(())
}

fn validate_json_value(schema: &Value, value: &Value, path: &str) -> Result<()> {
    let object = schema.as_object().ok_or_else(|| {
        Error::StructuredOutput(format!("invalid schema object while validating {path}"))
    })?;
    if let Some(values) = object.get("enum").and_then(Value::as_array) {
        if !values.contains(value) {
            return Err(Error::StructuredOutput(format!(
                "value at {path} is not in the schema enum"
            )));
        }
    }
    match object.get("type").and_then(Value::as_str) {
        Some("object") => {
            let value = value
                .as_object()
                .ok_or_else(|| Error::StructuredOutput(format!("expected object at {path}")))?;
            let properties = object
                .get("properties")
                .and_then(Value::as_object)
                .ok_or_else(|| Error::StructuredOutput(format!("missing properties at {path}")))?;
            if let Some(required) = object.get("required").and_then(Value::as_array) {
                for required in required {
                    let required = required.as_str().ok_or_else(|| {
                        Error::StructuredOutput(format!("invalid required property at {path}"))
                    })?;
                    if !value.contains_key(required) {
                        return Err(Error::StructuredOutput(format!(
                            "missing required property {path}.{required}"
                        )));
                    }
                }
            }
            for (name, child) in value {
                let child_schema = properties.get(name).ok_or_else(|| {
                    Error::StructuredOutput(format!(
                        "unexpected property {path}.{name} in strict result"
                    ))
                })?;
                validate_json_value(child_schema, child, &format!("{path}.{name}"))?;
            }
        }
        Some("array") => {
            let value = value
                .as_array()
                .ok_or_else(|| Error::StructuredOutput(format!("expected array at {path}")))?;
            if let Some(minimum) = object.get("minItems").and_then(Value::as_u64) {
                if value.len() < minimum as usize {
                    return Err(Error::StructuredOutput(format!(
                        "array at {path} has fewer than minItems"
                    )));
                }
            }
            if let Some(maximum) = object.get("maxItems").and_then(Value::as_u64) {
                if value.len() > maximum as usize {
                    return Err(Error::StructuredOutput(format!(
                        "array at {path} has more than maxItems"
                    )));
                }
            }
            let items = object
                .get("items")
                .ok_or_else(|| Error::StructuredOutput(format!("missing items at {path}")))?;
            for (index, child) in value.iter().enumerate() {
                validate_json_value(items, child, &format!("{path}[{index}]"))?;
            }
        }
        Some("string") if !value.is_string() => {
            return Err(Error::StructuredOutput(format!(
                "expected string at {path}"
            )));
        }
        Some("string") => {
            let text = value.as_str().expect("string type checked");
            if let Some(minimum) = object.get("minLength").and_then(Value::as_u64) {
                if text.chars().count() < minimum as usize {
                    return Err(Error::StructuredOutput(format!(
                        "string at {path} is shorter than minLength"
                    )));
                }
            }
            if let Some(maximum) = object.get("maxLength").and_then(Value::as_u64) {
                if text.chars().count() > maximum as usize {
                    return Err(Error::StructuredOutput(format!(
                        "string at {path} is longer than maxLength"
                    )));
                }
            }
        }
        Some("integer") if value.as_i64().is_none() && value.as_u64().is_none() => {
            return Err(Error::StructuredOutput(format!(
                "expected integer at {path}"
            )));
        }
        Some("number") if !value.is_number() => {
            return Err(Error::StructuredOutput(format!(
                "expected number at {path}"
            )));
        }
        Some("boolean") if !value.is_boolean() => {
            return Err(Error::StructuredOutput(format!(
                "expected boolean at {path}"
            )));
        }
        Some("null") if !value.is_null() => {
            return Err(Error::StructuredOutput(format!("expected null at {path}")));
        }
        None if object.get("enum").and_then(Value::as_array).is_some() => {}
        Some(_) => {}
        None => {
            return Err(Error::StructuredOutput(format!(
                "schema omitted type at {path}"
            )));
        }
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && is_lowercase_hex(value)
}

fn is_lowercase_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::{
        AnswerOnlyCapabilities, AuthenticationState, CliArtifactIdentity, CliIdentity,
        CliPackageIdentity, InferenceDestination, LoginChallengeWire, LoginStatus, LoginStatusWire,
        RuntimeFacts,
    };
    use crate::{AutohandSdk, ClientContext, Config, Error};

    const VALID_SETUP: &str = include_str!("../schema/blueprint-setup-contract-v1.valid.json");
    const INVALID_SETUP: &str = include_str!("../schema/blueprint-setup-contract-v1.invalid.json");

    #[test]
    fn canonical_setup_vectors_decode_through_the_private_wire_types() {
        let valid: Value = serde_json::from_str(VALID_SETUP).expect("canonical setup vector");
        let challenge_wire: LoginChallengeWire =
            serde_json::from_value(valid["begin"]["result"].clone())
                .expect("canonical challenge wire");
        let challenge = challenge_wire
            .validate(AutohandSdk::new(Config::default()))
            .expect("canonical challenge");
        assert_eq!(challenge.user_code, "ABCD-EFGH");
        challenge.session.finish();

        let poll: LoginStatusWire =
            serde_json::from_value(valid["poll"]["result"].clone()).expect("canonical poll wire");
        assert_eq!(
            poll.validate().expect("canonical pending status"),
            LoginStatus::Pending {
                poll_after_ms: 2_000
            }
        );
        let cancel: LoginStatusWire = serde_json::from_value(valid["cancel"]["result"].clone())
            .expect("canonical cancel wire");
        assert_eq!(
            cancel.validate().expect("canonical cancelled status"),
            LoginStatus::Cancelled
        );
        let failed: LoginStatusWire = serde_json::from_value(valid["failed"]["result"].clone())
            .expect("canonical failed wire");
        assert!(matches!(
            failed.validate().expect("canonical failed status"),
            LoginStatus::Failed { problem }
                if problem.code
                    == super::LoginProblemCode::CredentialPersistenceFailed
                    && problem.retryable
        ));

        let invalid: Value =
            serde_json::from_str(INVALID_SETUP).expect("canonical invalid setup vector");
        assert!(
            serde_json::from_value::<LoginChallengeWire>(invalid["begin"]["result"].clone())
                .is_err(),
            "private device state and an incomplete challenge must be rejected"
        );
        assert!(
            serde_json::from_value::<LoginStatusWire>(invalid["failed"]["result"].clone()).is_err(),
            "free-form failed status and incompatible poll fields must be rejected"
        );
    }

    #[test]
    fn setup_status_tag_controls_the_complete_allowed_shape() {
        for invalid in [
            json!({"contractVersion": 1, "status": "pending"}),
            json!({
                "contractVersion": 1,
                "status": "pending",
                "pollAfterMs": 1_000,
                "deviceCode": "private"
            }),
            json!({"contractVersion": 1, "status": "authorized", "pollAfterMs": 1_000}),
            json!({"contractVersion": 1, "status": "failed", "problem": "poll failed"}),
        ] {
            assert!(
                serde_json::from_value::<LoginStatusWire>(invalid).is_err(),
                "status-incompatible, missing, and untyped fields must fail closed"
            );
        }
    }

    fn runtime_facts(destination: InferenceDestination) -> RuntimeFacts {
        RuntimeFacts {
            cli_version: "1.0.0".to_owned(),
            answer_contract_version: 1,
            cli_identity: CliIdentity {
                invocation_path: "/fixture/autohand".to_owned(),
                resolved_path: "/fixture/autohand".to_owned(),
                symlink_chain: Vec::new(),
                package: CliPackageIdentity {
                    name: "autohand".to_owned(),
                    version: "1.0.0".to_owned(),
                    commit: None,
                },
                artifacts: vec![CliArtifactIdentity {
                    path: "/fixture/autohand".to_owned(),
                    size: 1,
                    sha256: "a".repeat(64),
                }],
                identity_hash: "b".repeat(64),
            },
            provider_id: "provider".to_owned(),
            model: None,
            authentication: AuthenticationState::NotRequired,
            inference_destination: destination,
            capabilities: AnswerOnlyCapabilities {
                client_context: ClientContext::Blueprint,
                answer_only: true,
                restricted: true,
                tools_enabled: false,
                hooks_enabled: false,
                mcp_enabled: false,
                memory_enabled: false,
                session_persistence_enabled: false,
            },
        }
    }

    #[test]
    fn only_in_process_and_local_subprocess_inference_can_receive_evidence() {
        for allowed in [
            InferenceDestination::InProcess { provider: None },
            InferenceDestination::LocalSubprocess {
                provider: "local".to_owned(),
            },
        ] {
            runtime_facts(allowed)
                .validate_answer_destination()
                .expect("offline destination");
        }
        for (blocked, expected) in [
            (
                InferenceDestination::LocalService {
                    provider: "ollama".to_owned(),
                    origin: "http://127.0.0.1:11434".to_owned(),
                },
                "local_service",
            ),
            (
                InferenceDestination::Hosted {
                    provider: "hosted".to_owned(),
                    origin: None,
                },
                "hosted",
            ),
            (InferenceDestination::Opaque, "opaque"),
        ] {
            assert!(matches!(
                runtime_facts(blocked).validate_answer_destination(),
                Err(Error::InferenceDestinationBlocked { destination })
                    if destination == expected
            ));
        }
    }
}
