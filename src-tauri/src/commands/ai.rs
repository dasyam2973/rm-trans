use std::sync::atomic::Ordering;

use tauri::{AppHandle, State};

use crate::ai::runner::{self, AiItem, AiState, AiSummary};
use crate::ai::settings::AiSettings;
use crate::error::{Error, Result};
use crate::store::GlossaryTerm;

/// 번역 결과는 완료되는 배치마다 `ai://result` 이벤트로, 진행률은 `ai://progress`로 전달된다.
#[tauri::command]
pub async fn ai_translate(
    app: AppHandle,
    state: State<'_, AiState>,
    settings: AiSettings,
    items: Vec<AiItem>,
    glossary: Vec<GlossaryTerm>,
) -> Result<AiSummary> {
    if settings.model.trim().is_empty() {
        return Err(Error::msg("AI 설정에서 모델을 지정해 주세요."));
    }
    if state.running.swap(true, Ordering::SeqCst) {
        return Err(Error::msg("이미 번역이 진행 중입니다."));
    }
    state.cancel.store(false, Ordering::SeqCst);
    let summary = runner::run(&app, &state, settings, items, glossary).await;
    state.running.store(false, Ordering::SeqCst);
    Ok(summary)
}

#[tauri::command]
pub fn ai_cancel(state: State<'_, AiState>) {
    state.request_cancel();
}
