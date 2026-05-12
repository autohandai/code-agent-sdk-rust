mod common;

use autohand_sdk::{Agent, Config};

fn checkout_discount(subtotal: f64, loyalty_tier: Option<&str>) -> Result<f64, String> {
    match loyalty_tier {
        Some("gold") => Ok(subtotal * 0.15),
        Some(_) => Ok(subtotal * 0.05),
        None => Err("checkout discount failed: missing customer loyalty tier".to_string()),
    }
}

fn capture_runtime_error() -> String {
    match checkout_discount(129.0, None) {
        Ok(_) => String::new(),
        Err(error) => [
            format!("RuntimeError: {error}"),
            "    at checkout::discounts::checkout_discount (src/checkout/discounts.rs:42)"
                .to_string(),
            "    at checkout::session::create_checkout_session (src/checkout/session.rs:88)"
                .to_string(),
            "Request: POST /checkout".to_string(),
            r#"Payload: {"subtotal":129,"customer":null}"#.to_string(),
        ]
        .join("\n"),
    }
}

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    let target_repo = std::env::var("AUTOHAND_TARGET_REPO").unwrap_or_else(|_| ".".to_string());
    let captured_error = capture_runtime_error();
    let prompt = format!(
        "{}\n\n{}\n\n{}\n```text\n{}\n```\n\n{}\n\n{}",
        "You are a QA engineering agent that turns production error reports into small repair pull requests.",
        "Reproduce the failure when the repository makes that possible. Fix the root cause, add or update a focused regression test, run the relevant validation command, commit the fix, push a branch, and create a pull request.",
        "A runtime error was captured by the application error boundary.",
        captured_error,
        "Expected user impact: A checkout session should still calculate a safe default discount when the customer object is missing.",
        "Please create a pull request with the fix."
    );

    println!("=== 26 Runtime Error to Pull Request ===\n");

    let instructions = [
        "You are a QA engineering agent that turns production error reports into small repair pull requests.",
        "Reproduce the failure when the repository makes that possible.",
        "Fix the root cause, add or update a focused regression test, run the relevant validation command, commit the fix, push a branch, and create a pull request.",
        "Keep the pull request description concise and include the error signature, the fix summary, and the validation result.",
    ]
    .join("\n");

    let config = Config::from_env()
        .with_cwd(target_repo)
        .with_instructions(instructions);
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
