//! OpenAI 호환 Chat Completions 클라이언트

use serde_json::{json, Value};

use crate::error::{Error, Result};

use super::settings::{AiSettings, ResponseMode};

pub async fn chat(
    http: &reqwest::Client,
    s: &AiSettings,
    system: &str,
    user: &str,
) -> Result<String> {
    let url = format!("{}/chat/completions", s.base_url.trim_end_matches('/'));
    let mut body = json!({
        "model": s.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    });
    if let Some(t) = s.temperature {
        body["temperature"] = json!(t);
    }
    match s.response_mode {
        ResponseMode::None => {}
        ResponseMode::JsonObject => {
            body["response_format"] = json!({ "type": "json_object" });
        }
        ResponseMode::Structured => {
            body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": {
                "name": "translation_list_response",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "translations": {
                            "type": "array",
                            "description": "번역된 항목들의 목록",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": {
                                        "type": "number",
                                        "description": "원본 텍스트의 고유 ID"
                                    },
                                    "text": {
                                        "type": "string",
                                        "description": "번역된 결과 텍스트"
                                    }
                                },
                                "required": ["id", "text"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["translations"],
                    "additionalProperties": false
                }
            }
            });
        }
    }

    let mut req = http.post(&url).json(&body);
    // 로컬 서버 등 키가 필요 없는 API도 있으므로 비어 있으면 헤더를 생략
    if !s.api_key.is_empty() {
        req = req.bearer_auth(&s.api_key);
    }
    let res = req.send().await?;
    let status = res.status();
    let text = res.text().await?;
    if !status.is_success() {
        let detail: String = text.chars().take(500).collect();
        return Err(Error::msg(format!("AI 요청 실패 ({status}): {detail}")));
    }

    let v: Value = serde_json::from_str(&text)?;
    v.pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::msg("AI 응답에 choices[0].message.content가 없습니다."))
}
