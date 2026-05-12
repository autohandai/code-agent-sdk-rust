mod common;

use autohand_sdk::{Agent, Config};
use serde::Serialize;

#[derive(Debug)]
struct GitHubCredentials {
    token_env_name: String,
    remote: String,
    base_branch: String,
    repository: Option<String>,
}

#[derive(Debug, Serialize)]
struct IncidentPacket {
    id: &'static str,
    severity: &'static str,
    service: &'static str,
    first_seen: &'static str,
    release: &'static str,
    error_signature: &'static str,
    user_impact: &'static str,
    stack_trace: String,
    logs: Vec<&'static str>,
    suspected_files: Vec<&'static str>,
    reproduction_command: &'static str,
    validation_commands: Vec<&'static str>,
}

fn github_credentials_from_env() -> Result<GitHubCredentials, String> {
    let token_env_name = if std::env::var("GITHUB_TOKEN").is_ok_and(|value| !value.is_empty()) {
        "GITHUB_TOKEN"
    } else if std::env::var("GH_TOKEN").is_ok_and(|value| !value.is_empty()) {
        "GH_TOKEN"
    } else {
        return Err("Set GITHUB_TOKEN or GH_TOKEN before running this example.".to_string());
    };

    Ok(GitHubCredentials {
        token_env_name: token_env_name.to_string(),
        remote: std::env::var("AUTOHAND_GITHUB_REMOTE").unwrap_or_else(|_| "origin".to_string()),
        base_branch: std::env::var("AUTOHAND_GITHUB_BASE_BRANCH")
            .unwrap_or_else(|_| "main".to_string()),
        repository: std::env::var("GITHUB_REPOSITORY").ok(),
    })
}

fn capture_incident_packet() -> IncidentPacket {
    IncidentPacket {
        id: "INC-2026-05-12-0417",
        severity: "sev2",
        service: "checkout-api",
        first_seen: "2026-05-12T09:14:22Z",
        release: "checkout-api@2026.05.12.3",
        error_signature:
            "RuntimeError: checkout discount failed while replaying coupon idempotency key",
        user_impact:
            "Checkout returns HTTP 500 for guest customers using coupon replay from mobile clients.",
        stack_trace: [
            "RuntimeError: checkout discount failed while replaying coupon idempotency key",
            "    at checkout::discounts::calculate_discount (src/checkout/discounts.rs:42)",
            "    at checkout::payments::build_payment_intent (src/checkout/payment_intent.rs:118)",
            "    at checkout::session::create_checkout_session (src/checkout/session.rs:88)",
        ]
        .join("\n"),
        logs: vec![
            "level=error trace=trk_94 request_id=req_7f2 route=POST /checkout status=500 duration_ms=184",
            "level=warn trace=trk_94 idempotency_key=checkout:cart_live_9834:attempt_2 cache_status=miss",
            "level=info trace=trk_94 feature_flags=discount-v2,coupon-replay",
        ],
        suspected_files: vec![
            "src/checkout/discounts.rs",
            "src/checkout/payment_intent.rs",
            "src/checkout/session.rs",
            "tests/checkout_session.rs",
        ],
        reproduction_command:
            "cargo test guest_coupon_replay --package checkout-api -- --exact",
        validation_commands: vec![
            "cargo test guest_coupon_replay --package checkout-api -- --exact",
            "cargo test",
            "cargo clippy --all-targets -- -D warnings",
        ],
    }
}

fn build_prompt(
    incident: &IncidentPacket,
    github: &GitHubCredentials,
) -> Result<String, serde_json::Error> {
    let repo_hint = github
        .repository
        .as_ref()
        .map(|repo| format!("- GitHub repository hint: {repo}."))
        .unwrap_or_else(|| "- Discover the GitHub repository from git remote output.".to_string());
    let incident_json = serde_json::to_string_pretty(incident)?;

    Ok([
        "You are a senior QA engineering agent responsible for converting production incidents into verified repair pull requests.".to_string(),
        String::new(),
        "GitHub credentials:".to_string(),
        format!("- A GitHub token is available in the {} environment variable. Do not print or commit the token.", github.token_env_name),
        format!("- Use git remote {}.", github.remote),
        format!("- Open the pull request against {}.", github.base_branch),
        repo_hint,
        "- Before pushing, run gh auth status or an equivalent non-secret auth check.".to_string(),
        String::new(),
        "Incident packet:".to_string(),
        "```json".to_string(),
        incident_json,
        "```".to_string(),
        String::new(),
        "Required workflow:".to_string(),
        "1. Inspect the target repository and confirm the likely failing path.".to_string(),
        "2. Reproduce the incident using the provided payload or nearest existing test harness.".to_string(),
        "3. Fix the root cause, not just the thrown exception.".to_string(),
        "4. Add a regression test covering guest checkout, coupon replay, and idempotency behavior.".to_string(),
        "5. Run the focused test first, then the relevant validation commands.".to_string(),
        "6. Create a branch named autohand/fix-checkout-incident-inc-2026-05-12-0417.".to_string(),
        "7. Commit the fix with a clear message.".to_string(),
        "8. Push the branch and open a pull request.".to_string(),
        "9. In the PR body, include the incident id, error signature, files changed, tests run, and any residual risk.".to_string(),
    ]
    .join("\n"))
}

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    let github = github_credentials_from_env()
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    let prompt = build_prompt(&capture_incident_packet(), &github)?;
    let target_repo = std::env::var("AUTOHAND_TARGET_REPO").unwrap_or_else(|_| ".".to_string());

    let config = Config::from_env().with_cwd(target_repo).with_instructions(
        "Work like a careful senior QA engineer. Keep secrets out of logs and pull request text.",
    );
    let mut agent = Agent::create(config).await?;
    let mut run = agent.send(prompt).await?;

    while let Some(event) = run.next().await {
        common::handle_plain_event(event?).await;
    }

    let result = run.wait().await?;
    println!("\n\nRun {} {}.", result.id, result.status);
    agent.close().await?;
    Ok(())
}
