use autohand_sdk::{
    AnswerOnlyProfile, BlueprintArtifactClass, ClassifiedAnswerEnvelope, ClassifiedArtifact,
    Config, StrictJsonSchema,
};

fn main() -> autohand_sdk::Result<()> {
    let schema = StrictJsonSchema::new(serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" }
        },
        "required": ["answer"],
        "additionalProperties": false
    }))?;
    let _envelope = ClassifiedAnswerEnvelope::new(
        "3b8f9ffb1c1962b70c60a86d3ebfe3c2422e677865057c1b8bc31e813c1db2ed",
        vec![ClassifiedArtifact {
            id: "evidence-1".to_owned(),
            class: BlueprintArtifactClass::SourceSnippet,
            content: "fn main() {}".to_owned(),
        }],
        &schema,
    )?;
    let _config = Config::default().with_answer_only_profile(AnswerOnlyProfile::blueprint());
    Ok(())
}
