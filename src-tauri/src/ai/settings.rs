use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};

/// `{{language}}`는 요청 시 대상 언어 이름으로 치환된다.
pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are a professional video game translator. Translate the given RPG game text into natural {{language}}.
- Keep the tone and personality of each speaker.
- Keep line breaks as they are.
- Items in the same `group` are consecutive lines of the same message; translate them so they read naturally together.";

/// 사용자가 직접 입력하는 AI 설정. OpenAI 호환 API라면 무엇이든 쓸 수 있도록 엔드포인트와 모델을 자유롭게 지정한다.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiSettings {
    /// 예: https://api.openai.com/v1, http://localhost:11434/v1
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub temperature: Option<f32>,
    /// 요청 하나에 담을 아이템 수
    pub batch_size: usize,
    /// 동시에 보낼 요청 수
    pub concurrency: usize,
    /// 번역 대상 언어 (영문 이름, 프롬프트에 그대로 들어간다)
    pub target_language: String,
    /// response_format 종류 (지원하지 않는 API도 있음)
    pub response_mode: ResponseMode,
    /// 원문의 제어 문자가 빠지거나 중복된 번역을 실패로 처리할지
    pub require_codes: bool,
    /// 대사 블록이 통째로 번역 대상이면 줄을 이어 번역한 뒤 다시 나눌지 (띄어쓰기를 쓰는 대상 언어에서만)
    pub merge_lines: bool,
    /// 병합 번역 결과를 나눌 때 한 줄의 최대 폭 (전각 글자 수 기준, 반각은 0.5)
    pub max_line_width: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResponseMode {
    /// response_format: json_object
    JsonObject,
    /// response_format: json_schema (구조화된 출력)
    #[default]
    Structured,
    /// response_format 미지정 (프롬프트 지시에만 의존)
    None,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            base_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.into(),
            temperature: None,
            batch_size: 30,
            concurrency: 2,
            target_language: "Korean".into(),
            response_mode: ResponseMode::default(),
            require_codes: true,
            merge_lines: true,
            max_line_width: 22.0,
        }
    }
}

fn settings_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| Error::msg(e.to_string()))?;
    Ok(dir.join("ai-settings.json"))
}

#[tauri::command]
pub fn get_ai_settings(app: AppHandle) -> Result<AiSettings> {
    let path = settings_path(&app)?;
    if !path.is_file() {
        return Ok(AiSettings::default());
    }
    let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let mut settings: AiSettings = serde_json::from_str(&text).unwrap_or_default();
    // 구버전 설정 마이그레이션: jsonMode(bool) → responseMode
    if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
        if raw.get("responseMode").is_none() && raw.get("jsonMode").and_then(|v| v.as_bool()) == Some(true) {
            settings.response_mode = ResponseMode::JsonObject;
        }
    }
    Ok(settings)
}

#[tauri::command]
pub fn save_ai_settings(app: AppHandle, settings: AiSettings) -> Result<()> {
    let path = settings_path(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    fs::write(&path, serde_json::to_string_pretty(&settings)?).map_err(|e| Error::io(&path, e))
}

#[tauri::command]
pub fn default_system_prompt() -> &'static str {
    DEFAULT_SYSTEM_PROMPT
}
