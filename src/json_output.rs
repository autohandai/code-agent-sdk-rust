use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct StructuredOutputError {
    pub message: String,
    pub raw_response: String,
}

pub fn json_instruction(
    schema_name: Option<&str>,
    schema: Option<&Value>,
    output_instructions: Option<&str>,
) -> String {
    let mut parts = vec![
        "Return only valid JSON.".to_string(),
        "Do not wrap the response in Markdown.".to_string(),
        "Do not include commentary outside the JSON value.".to_string(),
    ];
    if let Some(schema_name) = schema_name {
        parts.push(format!("The JSON value should satisfy: {schema_name}."));
    }
    if let Some(schema) = schema {
        parts.push(format!(
            "Use this JSON schema or example shape:\n{}",
            serde_json::to_string_pretty(schema).unwrap_or_else(|_| schema.to_string())
        ));
    }
    if let Some(output_instructions) = output_instructions {
        parts.push(output_instructions.to_string());
    }
    parts.join("\n")
}

pub fn parse_json_text(text: &str) -> Result<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(Error::StructuredOutput(
            "Expected JSON output, received an empty response.".to_string(),
        ));
    }
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }
    for candidate in fenced_json_candidates(trimmed) {
        if let Ok(value) = serde_json::from_str(&candidate) {
            return Ok(value);
        }
    }
    for candidate in embedded_json_candidates(trimmed) {
        if let Ok(value) = serde_json::from_str(&candidate) {
            return Ok(value);
        }
    }
    Err(Error::StructuredOutput(
        "Expected valid JSON output from the agent.".to_string(),
    ))
}

pub fn parse_json_as<T: DeserializeOwned>(text: &str) -> Result<T> {
    let value = parse_json_text(text)?;
    serde_json::from_value(value).map_err(Error::from)
}

fn fenced_json_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        rest = &rest[start + 3..];
        if let Some(after_newline) = rest.strip_prefix("json\n") {
            rest = after_newline;
        } else if let Some(after_newline) = rest.strip_prefix('\n') {
            rest = after_newline;
        }
        if let Some(end) = rest.find("```") {
            candidates.push(rest[..end].trim().to_string());
            rest = &rest[end + 3..];
        } else {
            break;
        }
    }
    candidates
}

fn embedded_json_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut stack = Vec::new();
    let mut start = None;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            continue;
        }
        if ch == '{' || ch == '[' {
            if stack.is_empty() {
                start = Some(index);
            }
            stack.push(ch);
            continue;
        }
        if (ch == '}' || ch == ']') && !stack.is_empty() {
            let opener = stack.pop().expect("stack checked");
            let matches = (opener == '{' && ch == '}') || (opener == '[' && ch == ']');
            if !matches {
                stack.clear();
                start = None;
                continue;
            }
            if stack.is_empty() {
                if let Some(start) = start.take() {
                    candidates.push(text[start..=index].to_string());
                }
            }
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::parse_json_text;

    #[test]
    fn parses_direct_fenced_and_embedded_json() {
        assert_eq!(parse_json_text(r#"{"ok":true}"#).unwrap()["ok"], true);
        assert_eq!(
            parse_json_text("```json\n{\"ok\":true}\n```").unwrap()["ok"],
            true
        );
        assert_eq!(
            parse_json_text("Result: {\"ok\":true} done.").unwrap()["ok"],
            true
        );
    }

    #[test]
    fn rejects_non_json() {
        assert!(parse_json_text("not json").is_err());
    }
}
