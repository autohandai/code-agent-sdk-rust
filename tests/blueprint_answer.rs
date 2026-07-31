#![cfg(target_os = "macos")]

use std::{fs, net::TcpListener, os::unix::fs::PermissionsExt, path::Path};

use autohand_sdk::{
    AnswerOnlyProfile, BlueprintArtifactClass, ClassifiedAnswerEnvelope, ClassifiedArtifact,
    Config, Error, InferenceDestination, StrictJsonSchema, StructuredRunLimits,
    BLUEPRINT_ANSWER_CONTRACT_VERSION,
};
use serde::Deserialize;
use serde_json::json;
use tempfile::tempdir;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Answer {
    summary: String,
}

fn write_cli(
    path: &Path,
    request_log: &Path,
    leaked: &Path,
    network_probe: Option<(&Path, u16)>,
    runtime_contract_version: u16,
    response_provider: &str,
) {
    let identity_hash = "a".repeat(64);
    let artifact_hash = "b".repeat(64);
    let network_probe = network_probe.map_or_else(String::new, |(marker, port)| {
        format!(
            r#"if /usr/bin/curl --silent --show-error --max-time 1 "http://127.0.0.1:{port}/probe" >/dev/null 2>&1; then
  printf connected > "{marker}"
else
  printf denied > "{marker}"
fi
"#,
            marker = marker.display()
        )
    });
    fs::write(
        path,
        format!(
            r#"#!/bin/sh
{network_probe}
printf '%s\n%s\n%s' "${{AUTOHAND_SDK_SECRET_SHOULD_NOT_LEAK-unset}}" "${{HOME-unset}}" "$(pwd)" > "{leaked}"
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([0-9][0-9]*\).*/\1/p')
  case "$line" in
    *autohand.runtimeInspect*)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"cliVersion":"9.9.9","answerContractVersion":{runtime_contract_version},"cliIdentity":{{"invocationPath":"/fixture/autohand","resolvedPath":"/fixture/runtime","symlinkChain":[{{"path":"/fixture/autohand","target":"/fixture/runtime"}}],"package":{{"name":"autohand","version":"9.9.9","commit":"abc123"}},"artifacts":[{{"path":"/fixture/runtime","size":42,"sha256":"{artifact_hash}"}}],"identityHash":"{identity_hash}"}},"providerId":"blueprint-local","model":"qwen","authentication":"not_required","clientContext":"blueprint","answerOnly":true,"permissionMode":"restricted","toolsEnabled":false,"hooksEnabled":false,"mcpEnabled":false,"memoryEnabled":false,"sessionPersistenceEnabled":false,"inferenceDestination":{{"kind":"in_process","provider":"blueprint-local"}}}}}}\n' "$id"
      ;;
    *autohand.answer*)
      printf '%s\n' "$line" > "{request_log}"
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"contractVersion":1,"result":{{"summary":"grounded"}},"providerId":"{response_provider}","model":"qwen","inferenceDestination":{{"kind":"in_process","provider":"blueprint-local"}}}}}}\n' "$id"
      ;;
    *)
      printf '{{"jsonrpc":"2.0","id":%s,"error":{{"code":-32601,"message":"method not allowed"}}}}\n' "$id"
      ;;
  esac
done
"#,
            leaked = leaked.display(),
            request_log = request_log.display(),
        ),
    )
    .expect("write fixture");
    let mut permissions = fs::metadata(path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

fn answer_schema() -> StrictJsonSchema {
    StrictJsonSchema::new(json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" }
        },
        "required": ["summary"],
        "additionalProperties": false
    }))
    .expect("strict schema")
}

#[tokio::test]
async fn classified_answer_is_strict_typed_and_environment_is_cleared() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let request_log = directory.path().join("answer-request.json");
    let leaked = directory.path().join("leaked-env.txt");
    write_cli(&cli, &request_log, &leaked, None, 1, "blueprint-local");

    std::env::set_var("AUTOHAND_SDK_SECRET_SHOULD_NOT_LEAK", "private");
    let config = Config::default()
        .with_cli_path(cli)
        .with_answer_only_profile(AnswerOnlyProfile::blueprint());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start answer-only fixture");
    let schema = answer_schema();
    let envelope = ClassifiedAnswerEnvelope::new(
        "3b8f9ffb1c1962b70c60a86d3ebfe3c2422e677865057c1b8bc31e813c1db2ed",
        vec![ClassifiedArtifact {
            id: "node-7".to_owned(),
            class: BlueprintArtifactClass::SourceSnippet,
            content: "fn grounded() {}".to_owned(),
        }],
        &schema,
    )
    .expect("classified envelope");

    let run = sdk
        .run_answer::<Answer>(envelope, schema, StructuredRunLimits::default())
        .await
        .expect("strict answer");
    assert_eq!(
        run.result,
        Answer {
            summary: "grounded".to_owned()
        }
    );
    assert_eq!(run.provider_id, "blueprint-local");
    assert_eq!(
        run.inference_destination,
        InferenceDestination::InProcess {
            provider: Some("blueprint-local".to_owned())
        }
    );
    assert_eq!(
        run.runtime_facts.answer_contract_version,
        BLUEPRINT_ANSWER_CONTRACT_VERSION
    );
    let environment = fs::read_to_string(leaked).expect("read environment marker");
    let mut environment = environment.lines();
    assert_eq!(environment.next(), Some("unset"));
    assert_eq!(
        environment.next(),
        std::env::var("HOME").ok().as_deref(),
        "the minimal credential-owner path remains available without inheriting unrelated values"
    );
    assert_eq!(
        std::path::PathBuf::from(environment.next().expect("child working directory"))
            .canonicalize()
            .expect("canonical child working directory"),
        std::env::temp_dir()
            .canonicalize()
            .expect("canonical neutral temporary directory"),
        "restricted execution must not inherit the host workspace as its working directory"
    );
    let request: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(request_log).expect("answer request"))
            .expect("request JSON");
    assert_eq!(request["method"], "autohand.answer");
    assert_eq!(request["params"]["contractVersion"], 1);
    assert_eq!(request["params"]["artifacts"][0]["class"], "source_snippet");
    assert_eq!(
        request["params"]["policyHash"],
        "3b8f9ffb1c1962b70c60a86d3ebfe3c2422e677865057c1b8bc31e813c1db2ed"
    );
    assert!(matches!(
        sdk.request("autohand.prompt", json!({"message": "untyped"}))
            .await,
        Err(Error::ProfileViolation {
            profile: "answer_only",
            ..
        })
    ));
    let agent = autohand_sdk::Agent::from_sdk(sdk.clone());
    assert!(matches!(
        agent.send("untyped").await,
        Err(Error::ProfileViolation {
            profile: "answer_only",
            ..
        })
    ));
    let bounded_schema = answer_schema();
    let bounded_envelope = ClassifiedAnswerEnvelope::new(
        "3b8f9ffb1c1962b70c60a86d3ebfe3c2422e677865057c1b8bc31e813c1db2ed",
        vec![ClassifiedArtifact {
            id: "node-8".to_owned(),
            class: BlueprintArtifactClass::SourceSnippet,
            content: "fn bounded() {}".to_owned(),
        }],
        &bounded_schema,
    )
    .expect("bounded envelope");
    assert!(matches!(
        sdk.run_answer::<Answer>(
            bounded_envelope,
            bounded_schema,
            StructuredRunLimits {
                max_input_bytes: 8 * 1024,
                max_output_bytes: 4,
            },
        )
        .await,
        Err(Error::OutputLimitExceeded {
            stream: "structured answer",
            limit: 4
        })
    ));
    sdk.stop().await.expect("stop fixture");
    std::env::remove_var("AUTOHAND_SDK_SECRET_SHOULD_NOT_LEAK");
}

#[tokio::test]
async fn answer_only_rejects_an_incompatible_runtime_before_answering() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let request_log = directory.path().join("answer-request.json");
    let leaked = directory.path().join("leaked-env.txt");
    write_cli(&cli, &request_log, &leaked, None, 2, "blueprint-local");
    let config = Config::default()
        .with_cli_path(cli)
        .with_answer_only_profile(AnswerOnlyProfile::blueprint());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    assert!(matches!(
        sdk.start().await,
        Err(Error::UnsupportedContractVersion {
            expected: 1,
            observed: 2
        })
    ));
    assert!(
        !request_log.exists(),
        "an incompatible runtime must not receive an answer request"
    );
}

#[tokio::test]
async fn answer_only_rejects_response_provenance_that_changed_after_inspection() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let request_log = directory.path().join("answer-request.json");
    let leaked = directory.path().join("leaked-env.txt");
    write_cli(&cli, &request_log, &leaked, None, 1, "changed-provider");
    let config = Config::default()
        .with_cli_path(cli)
        .with_answer_only_profile(AnswerOnlyProfile::blueprint());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start().await.expect("start answer-only fixture");
    let schema = answer_schema();
    let envelope = ClassifiedAnswerEnvelope::new(
        "3b8f9ffb1c1962b70c60a86d3ebfe3c2422e677865057c1b8bc31e813c1db2ed",
        vec![ClassifiedArtifact {
            id: "evidence".to_owned(),
            class: BlueprintArtifactClass::SourceSnippet,
            content: "fn grounded() {}".to_owned(),
        }],
        &schema,
    )
    .expect("classified envelope");
    assert!(matches!(
        sdk.run_answer::<Answer>(envelope, schema, StructuredRunLimits::default())
            .await,
        Err(Error::Protocol(message)) if message.contains("provenance")
    ));
    sdk.stop().await.expect("stop fixture");
}

#[tokio::test]
async fn answer_only_denies_loopback_egress_for_the_real_child_process() {
    let directory = tempdir().expect("fixture directory");
    let cli = directory.path().join("fake-autohand");
    let request_log = directory.path().join("answer-request.json");
    let leaked = directory.path().join("leaked-env.txt");
    let network_marker = directory.path().join("network.txt");
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback probe");
    listener
        .set_nonblocking(true)
        .expect("make loopback probe nonblocking");
    let port = listener
        .local_addr()
        .expect("loopback probe address")
        .port();
    write_cli(
        &cli,
        &request_log,
        &leaked,
        Some((&network_marker, port)),
        1,
        "blueprint-local",
    );

    let config = Config::default()
        .with_cli_path(cli)
        .with_answer_only_profile(AnswerOnlyProfile::blueprint());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    sdk.start()
        .await
        .expect("start answer-only network probe fixture");
    assert_eq!(
        fs::read_to_string(network_marker).expect("read network probe result"),
        "denied"
    );
    assert!(
        matches!(
            listener.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ),
        "the sandbox must prevent even loopback connections"
    );
    sdk.stop().await.expect("stop network probe fixture");
}

#[tokio::test]
async fn answer_only_rejects_user_extra_arguments_before_spawn() {
    let mut config = Config::default().with_answer_only_profile(AnswerOnlyProfile::blueprint());
    config.cli_path = Some("/does/not/matter".into());
    config.extra_args.push("--unsafe-user-value".to_owned());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    assert!(matches!(
        sdk.start().await,
        Err(Error::InvalidInput(message)) if message.contains("extra_args")
    ));
}

#[tokio::test]
async fn answer_only_rejects_direct_environment_injection_before_spawn() {
    let mut config = Config::default().with_answer_only_profile(AnswerOnlyProfile::blueprint());
    config.cli_path = Some("/does/not/matter".into());
    config
        .env
        .insert("UNREVIEWED_VALUE".into(), "secret".into());
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    assert!(matches!(
        sdk.start().await,
        Err(Error::InvalidInput(message)) if message.contains("environment allowlist")
    ));
}

#[tokio::test]
async fn answer_only_rejects_interactive_model_and_debug_controls_before_spawn() {
    let mut config = Config::default().with_answer_only_profile(AnswerOnlyProfile::blueprint());
    config.cli_path = Some("/does/not/matter".into());
    config.model = Some("unreviewed-model".into());
    config.debug = true;
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    assert!(matches!(
        sdk.start().await,
        Err(Error::InvalidInput(message)) if message.contains("closed profile")
    ));
}
