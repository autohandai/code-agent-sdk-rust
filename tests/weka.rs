use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
    time::Duration,
};

use autohand_sdk::{
    Error, WekaAnswer, WekaClient, WekaDecisionRequest, WekaNoulCriteria, WekaQuestion,
};
use serde_json::{json, Value};

#[derive(Debug)]
struct CapturedRequest {
    target: String,
    authorization: Option<String>,
    body: Value,
}

fn decision_request() -> WekaDecisionRequest {
    WekaDecisionRequest::new(
        json!({"tests": "passed", "changed_systems": ["checkout"]}),
        BTreeMap::from([
            (
                "needs_review".to_owned(),
                WekaQuestion::noul(
                    "Does this release need a person?",
                    Some(WekaNoulCriteria {
                        r#true: Some(json!("A person must review the evidence.")),
                        r#false: Some(json!("The automated checks are sufficient.")),
                    }),
                )
                .expect("noul question should be valid"),
            ),
            (
                "release_lane".to_owned(),
                WekaQuestion::choice(
                    "Choose the safest release lane.",
                    BTreeMap::from([
                        ("continue".to_owned(), json!("All required checks passed.")),
                        ("review".to_owned(), json!("The evidence needs a person.")),
                    ]),
                )
                .expect("choice question should be valid"),
            ),
            (
                "risk".to_owned(),
                WekaQuestion::score(
                    "Score the release risk.",
                    vec![json!("Low risk"), json!("Medium risk"), json!("High risk")],
                )
                .expect("score question should be valid"),
            ),
        ]),
    )
    .expect("decision request should be valid")
}

fn spawn_server(
    status: u16,
    response_body: Value,
    response_headers: &[(&str, &str)],
) -> (
    String,
    mpsc::Receiver<CapturedRequest>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("test listener should bind");
    let address = listener
        .local_addr()
        .expect("test listener should have an address");
    let (sender, receiver) = mpsc::channel();
    let headers = response_headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect::<Vec<_>>();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client should connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("read timeout should be set");
        let raw = read_http_request(&mut stream);
        let header_end = raw
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("request should contain headers");
        let header_text =
            String::from_utf8(raw[..header_end].to_vec()).expect("request headers should be UTF-8");
        let mut lines = header_text.split("\r\n");
        let target = lines
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .expect("request target should be present")
            .to_owned();
        let authorization = lines.find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("authorization")
                .then(|| value.trim().to_owned())
        });
        let body =
            serde_json::from_slice(&raw[header_end + 4..]).expect("request body should be JSON");
        sender
            .send(CapturedRequest {
                target,
                authorization,
                body,
            })
            .expect("capture receiver should remain open");

        let response_body = serde_json::to_vec(&response_body).expect("response should serialize");
        let reason = if status == 200 { "OK" } else { "Error" };
        let mut response = format!(
            "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n",
            response_body.len()
        );
        for (name, value) in headers {
            response.push_str(&format!("{name}: {value}\r\n"));
        }
        response.push_str("\r\n");
        stream
            .write_all(response.as_bytes())
            .expect("response headers should write");
        stream
            .write_all(&response_body)
            .expect("response body should write");
    });
    (format!("http://{address}"), receiver, handle)
}

fn read_http_request(stream: &mut impl Read) -> Vec<u8> {
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_length = None;
    loop {
        let read = stream
            .read(&mut buffer)
            .expect("request should be readable");
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buffer[..read]);
        if expected_length.is_none() {
            if let Some(header_end) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&raw[..header_end]);
                let content_length = headers.lines().find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| {
                        value
                            .trim()
                            .parse::<usize>()
                            .expect("content length should parse")
                    })
                });
                expected_length = Some(header_end + 4 + content_length.unwrap_or(0));
            }
        }
        if expected_length.is_some_and(|length| raw.len() >= length) {
            raw.truncate(expected_length.expect("length was checked"));
            break;
        }
    }
    raw
}

#[tokio::test]
async fn decide_sends_contract_and_returns_typed_answers() {
    let response = json!({
        "model": "weka",
        "answers": {
            "needs_review": {"type": "noul", "noul": 0.73},
            "release_lane": {
                "type": "choice",
                "choice": "review",
                "confidence": 0.82,
                "probabilities": {"continue": 0.18, "review": 0.82}
            },
            "risk": {
                "type": "score",
                "score": 1.3,
                "confidence": 0.7,
                "legend": {"0": "Low risk", "1": "Medium risk", "2": "High risk"},
                "probabilities": {"0": 0.1, "1": 0.5, "2": 0.4}
            }
        },
        "usage": {"input_tokens": 426, "output_tokens": 73}
    });
    let (base_url, captured, server) =
        spawn_server(200, response, &[("x-request-id", "req-success")]);
    let request = decision_request();
    let client = WekaClient::new("test-key")
        .expect("client config should be valid")
        .with_base_url(base_url)
        .expect("base URL should be valid");

    let result = client
        .decide(&request)
        .await
        .expect("decision should succeed");
    let WekaAnswer::Choice { choice, .. } = &result.answers["release_lane"] else {
        panic!("release lane should be a choice answer");
    };
    assert_eq!(choice, "review");
    let WekaAnswer::Noul { noul } = result.answers["needs_review"] else {
        panic!("needs review should be a noul answer");
    };
    assert_eq!(noul, 0.73);
    let WekaAnswer::Score { score, .. } = result.answers["risk"] else {
        panic!("risk should be a score answer");
    };
    assert_eq!(score, 1.3);
    assert_eq!(result.usage.input_tokens, 426);

    let captured = captured.recv().expect("request should be captured");
    assert_eq!(captured.target, "/v1/decisions");
    assert_eq!(captured.authorization.as_deref(), Some("Bearer test-key"));
    assert_eq!(captured.body, serde_json::to_value(request).unwrap());
    server.join().expect("server should finish");
}

#[tokio::test]
async fn decide_rejects_invalid_request_before_network() {
    let mut request = decision_request();
    request.questions.insert(
        "invalid".to_owned(),
        WekaQuestion::Score {
            instructions: json!("Score it."),
            criteria: vec![json!("Only one anchor")],
        },
    );
    let client = WekaClient::new("test-key")
        .expect("client config should be valid")
        .with_base_url("http://127.0.0.1:1")
        .expect("base URL should be valid");

    let error = client
        .decide(&request)
        .await
        .expect_err("request should be invalid");
    assert!(matches!(error, Error::InvalidInput(_)));
}

#[tokio::test]
async fn decide_rejects_unrequested_choice() {
    let response = json!({
        "model": "weka",
        "answers": {
            "needs_review": {"type": "noul", "noul": 0.73},
            "release_lane": {
                "type": "choice",
                "choice": "blocked",
                "confidence": 0.99,
                "probabilities": {"continue": 0.01, "review": 0.0, "blocked": 0.99}
            },
            "risk": {
                "type": "score",
                "score": 0.0,
                "confidence": 1.0,
                "legend": {"0": "Low risk", "1": "Medium risk", "2": "High risk"},
                "probabilities": {"0": 1.0, "1": 0.0, "2": 0.0}
            }
        },
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });
    let (base_url, _captured, server) = spawn_server(200, response, &[]);
    let client = WekaClient::new("test-key")
        .expect("client config should be valid")
        .with_base_url(base_url)
        .expect("base URL should be valid");

    let error = client
        .decide(&decision_request())
        .await
        .expect_err("unrequested choice should fail");
    assert!(matches!(error, Error::Protocol(_)));
    assert!(error.to_string().contains("unexpected response shape"));
    server.join().expect("server should finish");
}

#[tokio::test]
async fn decide_reports_status_without_exposing_response_body() {
    let (base_url, _captured, server) = spawn_server(
        429,
        json!({"secret": "must-not-appear"}),
        &[("x-request-id", "req-rate-limit")],
    );
    let client = WekaClient::new("test-key")
        .expect("client config should be valid")
        .with_base_url(base_url)
        .expect("base URL should be valid");

    let error = client
        .decide(&decision_request())
        .await
        .expect_err("HTTP error should fail");
    let Error::WekaRequest { status, request_id } = &error else {
        panic!("expected a Weka request error");
    };
    assert_eq!(*status, 429);
    assert_eq!(request_id.as_deref(), Some("req-rate-limit"));
    assert!(!error.to_string().contains("must-not-appear"));
    server.join().expect("server should finish");
}
