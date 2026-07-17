mod common;

use autohand_sdk::{
    Agent, AutoresearchCompareParams, AutoresearchEvaluatorMode, AutoresearchEvent,
    AutoresearchOptimizationDirection, AutoresearchPinParams, AutoresearchPruneParams,
    AutoresearchReplayParams, AutoresearchRescoreParams, AutoresearchSamplingOptions,
    AutoresearchStartParams, Config,
};

#[tokio::main]
async fn main() -> autohand_sdk::Result<()> {
    let target_repo = std::env::var("AUTOHAND_TARGET_REPO").unwrap_or_else(|_| ".".to_string());
    let mut agent = Agent::create(Config::from_env().with_cwd(target_repo)).await?;

    let started = agent
        .start_autoresearch(AutoresearchStartParams {
            objective: "Improve the benchmark while preserving correctness".into(),
            max_iterations: Some(10),
            metric_name: Some("duration_ms".into()),
            metric_unit: Some("ms".into()),
            direction: Some(AutoresearchOptimizationDirection::Lower),
            measure_command: Some("cargo test".into()),
            checks_command: Some("cargo test".into()),
            files_in_scope: vec!["src/".into()],
            sampling: Some(AutoresearchSamplingOptions {
                min_samples: Some(2),
                max_samples: Some(5),
                confidence_threshold: Some(0.9),
            }),
            ..AutoresearchStartParams::default()
        })
        .await?;

    let instruction = started.instruction.ok_or_else(|| {
        autohand_sdk::Error::StructuredOutput(
            started
                .error
                .unwrap_or_else(|| "autoresearch returned no loop instruction".into()),
        )
    })?;
    let mut run = agent.send(instruction).await?;
    while let Some(event) = run.next().await {
        let event = event?;
        if let Some(autoresearch) = event.autoresearch() {
            match autoresearch? {
                AutoresearchEvent::Lifecycle(event) => {
                    println!("[autoresearch:{:?}] {}", event.phase, event.status_text);
                }
                AutoresearchEvent::Operation(event) => {
                    println!(
                        "[autoresearch:{:?}:{:?}] success={}",
                        event.operation, event.phase, event.success
                    );
                }
            }
        } else {
            common::handle_plain_event(event).await;
        }
    }
    let result = run.wait().await?;
    println!("\nRun {} {}.", result.id, result.status);

    let history = agent.get_autoresearch_history().await?;
    println!("{} persisted attempts", history.attempts.len());
    let status = agent.get_autoresearch_status().await?;
    println!(
        "Autoresearch active={} runs={}: {}",
        status.active, status.runs_logged, status.status_text
    );
    let replayable: Vec<_> = history
        .attempts
        .iter()
        .filter(|attempt| attempt.replayable)
        .collect();
    if let Some(attempt) = replayable.first() {
        agent
            .replay_autoresearch(AutoresearchReplayParams {
                attempt_id: attempt.attempt_id.clone(),
                evaluator: Some(AutoresearchEvaluatorMode::Original),
            })
            .await?;
        agent
            .rescore_autoresearch(AutoresearchRescoreParams::attempt(&attempt.attempt_id))
            .await?;
        agent
            .replay_autoresearch(AutoresearchReplayParams {
                attempt_id: attempt.attempt_id.clone(),
                evaluator: Some(AutoresearchEvaluatorMode::Current),
            })
            .await?;
        agent
            .pin_autoresearch(AutoresearchPinParams {
                attempt_id: attempt.attempt_id.clone(),
                pinned: true,
            })
            .await?;
    }
    if replayable.len() > 1 {
        agent
            .compare_autoresearch(AutoresearchCompareParams {
                left_attempt_id: replayable[0].attempt_id.clone(),
                right_attempt_id: replayable[1].attempt_id.clone(),
            })
            .await?;
    }

    let pareto = agent.get_autoresearch_pareto().await?;
    println!("Pareto attempts: {:?}", pareto.attempt_ids);
    let preview = agent
        .prune_autoresearch(AutoresearchPruneParams {
            dry_run: Some(true),
            yes: None,
        })
        .await?;
    println!(
        "Prune preview: {} candidates, {} bytes",
        preview.candidates.len(),
        preview.bytes_freed
    );
    if std::env::var("AUTOHAND_APPLY_PRUNE").as_deref() == Ok("1") {
        agent
            .prune_autoresearch(AutoresearchPruneParams {
                dry_run: Some(false),
                yes: Some(true),
            })
            .await?;
    }

    agent.stop_autoresearch().await?;
    agent.close().await?;
    Ok(())
}
