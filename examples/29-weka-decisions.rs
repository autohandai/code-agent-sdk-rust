use std::collections::BTreeMap;

use autohand_sdk::{Result, WekaAnswer, WekaClient, WekaDecisionRequest, WekaQuestion};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    let questions = BTreeMap::from([
        (
            "release_lane".to_owned(),
            WekaQuestion::choice(
                "Choose the safest release lane.",
                BTreeMap::from([
                    (
                        "stable".to_owned(),
                        json!("Healthy checks and low expected impact."),
                    ),
                    (
                        "canary".to_owned(),
                        json!("Healthy checks with elevated impact."),
                    ),
                    ("blocked".to_owned(), json!("A required check failed.")),
                ]),
            )?,
        ),
        (
            "risk".to_owned(),
            WekaQuestion::score(
                "Score release risk against the ordered anchors.",
                vec![
                    json!("Low risk"),
                    json!("Material risk"),
                    json!("Severe risk"),
                ],
            )?,
        ),
    ]);
    let request = WekaDecisionRequest::new(
        json!({"tests": "passed", "changed_systems": ["checkout"]}),
        questions,
    )?;
    let response = WekaClient::from_env()?.decide(&request).await?;

    if let WekaAnswer::Choice { choice, .. } = &response.answers["release_lane"] {
        println!("release lane: {choice}");
    }
    if let WekaAnswer::Score { score, .. } = response.answers["risk"] {
        println!("risk score: {score}");
    }

    Ok(())
}
