use std::{collections::BTreeMap, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

const DEFAULT_BASE_URL: &str = "https://api.autohand.ai";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WekaDecisionRequest {
    model: WekaModel,
    pub state: Value,
    pub questions: BTreeMap<String, WekaQuestion>,
}

impl WekaDecisionRequest {
    pub fn new(state: Value, questions: BTreeMap<String, WekaQuestion>) -> Result<Self> {
        let request = Self {
            model: WekaModel::Weka,
            state,
            questions,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn model(&self) -> &'static str {
        "weka"
    }

    fn validate(&self) -> Result<()> {
        if self.questions.is_empty() {
            return Err(Error::InvalidInput(
                "Weka questions must not be empty".to_owned(),
            ));
        }
        for (name, question) in &self.questions {
            if name.is_empty() {
                return Err(Error::InvalidInput(
                    "Weka question names must not be empty".to_owned(),
                ));
            }
            question.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum WekaModel {
    #[serde(rename = "weka")]
    Weka,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum WekaQuestion {
    Noul {
        instructions: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        criteria: Option<WekaNoulCriteria>,
    },
    Choice {
        instructions: Value,
        criteria: BTreeMap<String, Value>,
    },
    Score {
        instructions: Value,
        criteria: Vec<Value>,
    },
}

impl WekaQuestion {
    pub fn noul(
        instructions: impl Into<Value>,
        criteria: Option<WekaNoulCriteria>,
    ) -> Result<Self> {
        let question = Self::Noul {
            instructions: instructions.into(),
            criteria,
        };
        question.validate()?;
        Ok(question)
    }

    pub fn choice(
        instructions: impl Into<Value>,
        criteria: BTreeMap<String, Value>,
    ) -> Result<Self> {
        let question = Self::Choice {
            instructions: instructions.into(),
            criteria,
        };
        question.validate()?;
        Ok(question)
    }

    pub fn score(instructions: impl Into<Value>, criteria: Vec<Value>) -> Result<Self> {
        let question = Self::Score {
            instructions: instructions.into(),
            criteria,
        };
        question.validate()?;
        Ok(question)
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Noul {
                instructions,
                criteria,
            } => {
                validate_description(instructions)?;
                if let Some(criteria) = criteria {
                    if let Some(description) = &criteria.r#true {
                        validate_description(description)?;
                    }
                    if let Some(description) = &criteria.r#false {
                        validate_description(description)?;
                    }
                }
            }
            Self::Choice {
                instructions,
                criteria,
            } => {
                validate_description(instructions)?;
                if criteria.is_empty() {
                    return Err(Error::InvalidInput(
                        "Weka choice criteria must not be empty".to_owned(),
                    ));
                }
                for description in criteria.values() {
                    validate_description(description)?;
                }
            }
            Self::Score {
                instructions,
                criteria,
            } => {
                validate_description(instructions)?;
                if criteria.len() < 2 {
                    return Err(Error::InvalidInput(
                        "Weka score criteria must contain at least two anchors".to_owned(),
                    ));
                }
                for description in criteria {
                    validate_description(description)?;
                }
            }
        }
        Ok(())
    }

    fn answer_kind(&self) -> WekaAnswerKind {
        match self {
            Self::Noul { .. } => WekaAnswerKind::Noul,
            Self::Choice { .. } => WekaAnswerKind::Choice,
            Self::Score { .. } => WekaAnswerKind::Score,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WekaNoulCriteria {
    #[serde(rename = "true", skip_serializing_if = "Option::is_none")]
    pub r#true: Option<Value>,
    #[serde(rename = "false", skip_serializing_if = "Option::is_none")]
    pub r#false: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum WekaAnswer {
    Noul {
        noul: f64,
    },
    Choice {
        choice: String,
        confidence: f64,
        probabilities: BTreeMap<String, f64>,
    },
    Score {
        score: f64,
        confidence: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
    },
}

impl WekaAnswer {
    fn kind(&self) -> WekaAnswerKind {
        match self {
            Self::Noul { .. } => WekaAnswerKind::Noul,
            Self::Choice { .. } => WekaAnswerKind::Choice,
            Self::Score { .. } => WekaAnswerKind::Score,
        }
    }

    fn validate(&self) -> bool {
        match self {
            Self::Noul { noul } => probability(*noul),
            Self::Choice {
                confidence,
                probabilities,
                ..
            }
            | Self::Score {
                confidence,
                probabilities,
                ..
            } => {
                probability(*confidence)
                    && probabilities.values().copied().all(probability)
                    && match self {
                        Self::Score { score, .. } => score.is_finite(),
                        _ => true,
                    }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WekaAnswerKind {
    Noul,
    Choice,
    Score,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WekaUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WekaDecisionResponse {
    pub model: String,
    pub answers: BTreeMap<String, WekaAnswer>,
    pub usage: WekaUsage,
}

#[derive(Debug, Clone)]
pub struct WekaClient {
    api_key: String,
    base_url: String,
    timeout: Duration,
    http: reqwest::Client,
}

impl WekaClient {
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(Error::InvalidInput(
                "Set AUTOHAND_AI_API_KEY or pass an API key to WekaClient".to_owned(),
            ));
        }
        Ok(Self {
            api_key,
            base_url: DEFAULT_BASE_URL.to_owned(),
            timeout: DEFAULT_TIMEOUT,
            http: reqwest::Client::new(),
        })
    }

    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("AUTOHAND_AI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var("AUTOHAND_API_KEY")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
            .ok_or_else(|| {
                Error::InvalidInput(
                    "Set AUTOHAND_AI_API_KEY or AUTOHAND_API_KEY before creating WekaClient"
                        .to_owned(),
                )
            })?;
        let mut client = Self::new(api_key)?;
        if let Some(base_url) = std::env::var("AUTOHAND_AI_BASE_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                std::env::var("AUTOHAND_API_URL")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
            })
        {
            client = client.with_base_url(base_url)?;
        }
        Ok(client)
    }

    pub fn with_base_url(mut self, base_url: impl AsRef<str>) -> Result<Self> {
        let parsed = url::Url::parse(base_url.as_ref())
            .map_err(|_| Error::InvalidInput("Invalid Weka base URL".to_owned()))?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host().is_none() {
            return Err(Error::InvalidInput(
                "Weka base URL must be an absolute HTTP or HTTPS URL".to_owned(),
            ));
        }
        self.base_url = parsed.as_str().trim_end_matches('/').to_owned();
        Ok(self)
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self> {
        if timeout.is_zero() {
            return Err(Error::InvalidInput(
                "Weka timeout must be greater than zero".to_owned(),
            ));
        }
        self.timeout = timeout;
        Ok(self)
    }

    pub async fn decide(&self, request: &WekaDecisionRequest) -> Result<WekaDecisionResponse> {
        request.validate()?;
        let response = self
            .http
            .post(format!("{}/v1/decisions", self.base_url))
            .bearer_auth(&self.api_key)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(self.timeout)
            .json(request)
            .send()
            .await?;

        let status = response.status();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        if !status.is_success() {
            return Err(Error::WekaRequest {
                status: status.as_u16(),
                request_id,
            });
        }

        let body = response.bytes().await?;
        let result: WekaDecisionResponse = serde_json::from_slice(&body).map_err(|_| {
            Error::Protocol("Weka returned an unexpected response shape".to_owned())
        })?;
        if !response_matches_request(&result, request) {
            return Err(Error::Protocol(
                "Weka returned an unexpected response shape".to_owned(),
            ));
        }
        Ok(result)
    }
}

fn response_matches_request(
    response: &WekaDecisionResponse,
    request: &WekaDecisionRequest,
) -> bool {
    if response.model.is_empty() || response.answers.len() != request.questions.len() {
        return false;
    }
    request.questions.iter().all(|(name, question)| {
        let Some(answer) = response.answers.get(name) else {
            return false;
        };
        if question.answer_kind() != answer.kind() || !answer.validate() {
            return false;
        }
        match (question, answer) {
            (WekaQuestion::Choice { criteria, .. }, WekaAnswer::Choice { choice, .. }) => {
                criteria.contains_key(choice)
            }
            _ => true,
        }
    })
}

fn validate_description(value: &Value) -> Result<()> {
    if matches!(
        value,
        Value::Null | Value::String(_) | Value::Array(_) | Value::Object(_)
    ) {
        Ok(())
    } else {
        Err(Error::InvalidInput(
            "Weka descriptions must be strings, arrays, objects, or null".to_owned(),
        ))
    }
}

fn probability(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
