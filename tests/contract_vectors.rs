use autohand_sdk::{
    AnswerOnlyProfile, BlueprintArtifactClass, ClassifiedAnswerEnvelope, ClassifiedArtifact,
    Config, Error, SetupOnlyProfile, StrictJsonSchema, StructuredRunLimits,
    BLUEPRINT_ANSWER_MAX_INPUT_BYTES, BLUEPRINT_ANSWER_MAX_OUTPUT_BYTES,
};
use serde_json::json;

const VALID: &str = include_str!("../schema/blueprint-answer-contract-v1.valid.json");
const INVALID: &str = include_str!("../schema/blueprint-answer-contract-v1.invalid.json");

#[test]
fn canonical_cli_contract_vectors_are_consumed_without_field_redefinition() {
    let valid: ClassifiedAnswerEnvelope =
        serde_json::from_str(VALID).expect("canonical valid envelope");
    let schema =
        StrictJsonSchema::new(valid.output_schema.clone()).expect("canonical strict output schema");
    ClassifiedAnswerEnvelope::new(valid.policy_hash, valid.artifacts, &schema)
        .expect("canonical valid vector should satisfy the SDK contract");

    assert!(
        serde_json::from_str::<ClassifiedAnswerEnvelope>(INVALID).is_err(),
        "the canonical invalid vector contains a closed-enum artifact violation"
    );
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

#[test]
fn strict_schema_rejects_unenforced_keywords_and_duplicate_artifact_ids() {
    assert!(matches!(
        StrictJsonSchema::new(json!({
            "type": "object",
            "properties": {
                "summary": { "type": "string", "pattern": "^claimed-but-unenforced$" }
            },
            "required": ["summary"],
            "additionalProperties": false
        })),
        Err(Error::InvalidInput(message)) if message.contains("unsupported")
    ));

    let schema = answer_schema();
    let duplicate = ClassifiedArtifact {
        id: "same-id".to_owned(),
        class: BlueprintArtifactClass::SourceSnippet,
        content: "fn grounded() {}".to_owned(),
    };
    assert!(matches!(
        ClassifiedAnswerEnvelope::new(
            "3b8f9ffb1c1962b70c60a86d3ebfe3c2422e677865057c1b8bc31e813c1db2ed",
            vec![duplicate.clone(), duplicate],
            &schema,
        ),
        Err(Error::InvalidInput(message)) if message.contains("unique")
    ));
}

#[tokio::test]
async fn answer_only_limits_can_be_tightened_but_not_relaxed() {
    let schema = answer_schema();
    let envelope = ClassifiedAnswerEnvelope::new(
        "3b8f9ffb1c1962b70c60a86d3ebfe3c2422e677865057c1b8bc31e813c1db2ed",
        vec![ClassifiedArtifact {
            id: "evidence".to_owned(),
            class: BlueprintArtifactClass::SourceSnippet,
            content: "fn bounded() {}".to_owned(),
        }],
        &schema,
    )
    .expect("valid classified envelope");
    let sdk = autohand_sdk::AutohandSdk::new(
        Config::default().with_answer_only_profile(AnswerOnlyProfile::blueprint()),
    );
    assert!(matches!(
        sdk.run_answer::<serde_json::Value>(
            envelope,
            schema,
            StructuredRunLimits {
                max_input_bytes: BLUEPRINT_ANSWER_MAX_INPUT_BYTES + 1,
                max_output_bytes: BLUEPRINT_ANSWER_MAX_OUTPUT_BYTES,
            },
        )
        .await,
        Err(Error::InvalidInput(message)) if message.contains("not relax")
    ));

    let mut config = Config::default().with_answer_only_profile(AnswerOnlyProfile::blueprint());
    config.max_stdout_bytes += 1;
    let mut sdk = autohand_sdk::AutohandSdk::new(config);
    assert!(matches!(
        sdk.start().await,
        Err(Error::InvalidInput(message)) if message.contains("not relaxed")
    ));
}

#[tokio::test]
async fn setup_only_rejects_endpoint_and_process_injection_environment() {
    for name in [
        "AUTOHAND_AUTH_API_URL",
        "https_proxy",
        "NODE_OPTIONS",
        "NODE_EXTRA_CA_CERTS",
        "DYLD_INSERT_LIBRARIES",
    ] {
        let config = Config::default()
            .with_setup_only_profile(SetupOnlyProfile::autohand_device_authorization())
            .allow_environment(name);
        let mut sdk = autohand_sdk::AutohandSdk::new(config);
        assert!(matches!(
            sdk.start().await,
            Err(Error::InvalidInput(message))
                if message.contains("behavior-changing variable")
        ));
    }
}
