use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{Error, Result};

/// 언어 지원 이전 버전의 기본 프롬프트 (한국어 고정). 저장된 값이 이것이면 새 기본값으로 교체한다.
const LEGACY_SYSTEM_PROMPT: &str = "\
You are a professional video game translator. Translate the given RPG game text into natural Korean.
- Keep the tone and personality of each speaker.
- Preserve RPG Maker control codes exactly as they appear (e.g. \\V[1], \\N[2], \\C[3], \\I[64], \\G, \\{, \\}, \\., \\|, \\!, \\>, \\<, \\^, \\\\).
- Keep line breaks as they are.
- Items in the same `group` are consecutive lines of the same message; translate them so they read naturally together.";

/// `{{language}}`는 요청 시 대상 언어 이름으로 치환된다.
pub const DEFAULT_SYSTEM_PROMPT: &str = "\
You are a professional video game translator. Translate the given RPG game text into natural {{language}}.
- Keep the tone and personality of each speaker.
- Preserve RPG Maker control codes exactly as they appear (e.g. \\V[1], \\N[2], \\C[3], \\I[64], \\G, \\{, \\}, \\., \\|, \\!, \\>, \\<, \\^, \\\\).
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
    // 구버전 설정 마이그레이션: jsonMode(bool) → responseMode, 한국어 고정 프롬프트 → 언어 치환 프롬프트
    if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
        if raw.get("responseMode").is_none() && raw.get("jsonMode").and_then(|v| v.as_bool()) == Some(true) {
            settings.response_mode = ResponseMode::JsonObject;
        }
    }
    if settings.system_prompt.trim() == LEGACY_SYSTEM_PROMPT.trim() {
        settings.system_prompt = DEFAULT_SYSTEM_PROMPT.into();
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
