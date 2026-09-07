use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use autohand_sdk::{is_step_count, Agent, Config, PromptOptions, ProviderName, RunStatus};
use serde_json::{json, Value};
use tempfile::tempdir;

struct ProviderMock {
    address: SocketAddr,
    calls: Arc<Mutex<Vec<Value>>>,
    stopped: Arc<AtomicBool>,
    task: Option<thread::JoinHandle<()>>,
}

impl ProviderMock {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local HTTP mock");
        let address = listener.local_addr().unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));
        let task = thread::spawn({
            let calls = calls.clone();
            let stopped = stopped.clone();
            move || {
                for stream in listener.incoming() {
                    if stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    let stream = stream.expect("accept mock request");
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut reader = BufReader::new(stream);
                    let mut first = String::new();
                    reader.read_line(&mut first).unwrap();
                    let path = first.split_whitespace().nth(1).expect("HTTP path");
                    let mut content_length = 0;
                    let mut authorized = false;
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        if line.trim().is_empty() {
                            break;
                        }
                        if let Some((key, value)) = line.split_once(':') {
                            if key.eq_ignore_ascii_case("content-length") {
                                content_length = value.trim().parse().unwrap();
                            }
                            if key.eq_ignore_ascii_case("authorization") {
                                authorized = value.trim() == "Bearer sdk-fixture-key";
                            }
                        }
                    }
                    let body = if path == "/auth/me" {
                        json!({"authenticated":true,"user":{"id":"fixture","email":"sdk@example.test","name":"SDK Fixture"}})
                    } else {
                        assert_eq!(path, "/chat/completions");
                        assert!(authorized, "mock inference credential");
                        let mut body = vec![0; content_length];
                        reader.read_exact(&mut body).unwrap();
                        let mut calls = calls.lock().unwrap();
                        calls.push(serde_json::from_slice(&body).unwrap());
                        let message = if calls.len() == 1 {
                            json!({"role":"assistant","content":"Inspect the evidence file.","tool_calls":[{
                                "id":"call-read","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"evidence.txt\"}"}
                            }]})
                        } else {
                            json!({"role":"assistant","content":"continued from persisted evidence"})
                        };
                        json!({"id":"fixture","choices":[{"message":message,"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":10,"total_tokens":20}})
                    }.to_string();
                    write!(reader.get_mut(), "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
                }
            }
        });
        Self {
            address,
            calls,
            stopped,
            task: Some(task),
        }
    }
}

impl Drop for ProviderMock {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.address);
        if let Some(task) = self.task.take() {
            task.join().expect("HTTP mock worker");
        }
    }
}

/// Set AUTOHAND_TEST_CLI_PATH to an executable current CLI; provider traffic stays local.
#[tokio::test]
async fn current_harness_persists_tool_results_across_stop_and_resume() {
    let Some(cli) = std::env::var_os("AUTOHAND_TEST_CLI_PATH") else {
        eprintln!("Set AUTOHAND_TEST_CLI_PATH for actual CLI integration");
        return;
    };
    let provider = ProviderMock::start();
    let workspace = tempdir().unwrap();
    fs::write(workspace.path().join("evidence.txt"), "sdk-parity-evidence").unwrap();
    let config_file = workspace.path().join("config.json");
    let base_url = format!("http://{}", provider.address);
    fs::write(&config_file, json!({
        "auth":{"token":"sdk-fixture-key"},"provider":"openrouter",
        "openrouter":{"baseUrl":format!("{base_url}/unused"),"apiKey":"saved-provider-key"},
        "autohandai":{"model":"fantail","plan":"cloud","authMode":"api-key","contextWindow":200000},
        "features":{"autohand_inference":true,"automaticSpecialists":false},
        "telemetry":{"enabled":false}
    }).to_string()).unwrap();
    let mut config = Config::default().with_cli_path(cli);
    config.cwd = Some(workspace.path().to_path_buf());
    config.provider = Some(ProviderName::AutohandAi);
    config.model = Some("fantail".into());
    config.api_key = Some("sdk-fixture-key".into());
    config.base_url = Some(base_url.clone());
    config.bare = true;
    config.unrestricted = true;
    config.extra_args = vec!["--config".into(), config_file.display().to_string()];
    config.env.extend([
        (
            "AUTOHAND_HOME".into(),
            workspace.path().join("home").display().to_string(),
        ),
        ("AUTOHAND_API_KEY".into(), "sdk-fixture-key".into()),
        ("AUTOHAND_API_URL".into(), base_url.clone()),
        ("AUTOHAND_AUTH_API_URL".into(), format!("{base_url}/auth")),
        ("AUTOHAND_SKIP_PING".into(), "1".into()),
        ("AUTOHAND_SKIP_UPDATE_CHECK".into(), "1".into()),
        ("AUTOHAND_NO_IDLE_LOGOUT".into(), "1".into()),
        ("AUTOHAND_DISABLE_AUTO_REPORT".into(), "1".into()),
    ]);
    let mut agent = Agent::create(config).await.expect("start actual CLI");
    let result = tokio::time::timeout(
        Duration::from_secs(45),
        agent.run_with_options(
            "Read evidence.txt with read_file.",
            PromptOptions {
                stop_when: vec![is_step_count(1).unwrap()],
                ..PromptOptions::default()
            },
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.status, RunStatus::Stopped);
    assert_eq!(result.steps.len(), 1);
    assert_eq!(provider.calls.lock().unwrap().len(), 1);
    let tool_result = &result.steps[0].tool_results[0];
    assert!(tool_result.success);
    assert!(tool_result
        .output
        .as_deref()
        .unwrap_or_default()
        .contains("sdk-parity-evidence"));
    let result = tokio::time::timeout(
        Duration::from_secs(45),
        agent.run("Continue using the saved tool result."),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.status, RunStatus::Completed);
    {
        let calls = provider.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert!(calls[1]["messages"]
            .to_string()
            .contains("sdk-parity-evidence"));
    }
    agent.close().await.unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_file).unwrap()).unwrap();
    assert_eq!(saved["provider"], "openrouter");
    assert_eq!(saved["openrouter"]["apiKey"], "saved-provider-key");
    assert!(saved["autohandai"].get("apiKey").is_none());
    assert!(saved["autohandai"].get("baseUrl").is_none());
}
