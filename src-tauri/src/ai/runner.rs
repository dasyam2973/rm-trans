//! 번역 요청을 배치로 나눠 동시에 보내고, 결과를 이벤트로 프론트엔드에 흘려보낸다.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

use super::codes::{self, Masked};
use super::glossary::Glossary;
use super::{client, prompt, settings::AiSettings};
use crate::store::GlossaryTerm;

pub const EVENT_RESULT: &str = "ai://result";
pub const EVENT_PROGRESS: &str = "ai://progress";
const MAX_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Deserialize)]
pub struct AiItem {
    pub id: String,
    pub text: String,
    pub group: String,
    pub context: Option<String>,
}

/// 제어 문자를 마스킹한 번역 대상
pub struct Job {
    pub id: String,
    pub group: String,
    pub context: Option<String>,
    pub masked: Masked,
}

#[derive(Serialize, Clone)]
pub struct TranslatedItem {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSummary {
    pub translated: usize,
    pub failed: usize,
    /// 제어 문자 외에 번역할 텍스트가 없어 보내지 않은 항목 수
    pub skipped: usize,
    pub errors: Vec<String>,
    pub cancelled: bool,
}

/// 실행 중인 번역 작업 상태 (Tauri managed state)
#[derive(Default)]
pub struct AiState {
    pub running: AtomicBool,
    pub cancel: AtomicBool,
    pub cancel_notify: Notify,
}

impl AiState {
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        self.cancel_notify.notify_waiters();
    }

    async fn cancelled(&self) {
        loop {
            let notified = self.cancel_notify.notified();
            if self.cancel.load(Ordering::SeqCst) {
                return;
            }
            notified.await;
        }
    }
}

/// 같은 그룹(대사 블록 등)은 가능한 한 같은 배치에 넣는다.
fn make_batches(items: Vec<Job>, size: usize) -> Vec<Vec<Job>> {
    let size = size.max(1);
    let mut groups: Vec<Vec<Job>> = Vec::new();
    for item in items {
        match groups.last_mut() {
            Some(g) if g[0].group == item.group => g.push(item),
            _ => groups.push(vec![item]),
        }
    }

    let mut batches = Vec::new();
    let mut cur: Vec<Job> = Vec::new();
    for group in groups {
        if !cur.is_empty() && cur.len() + group.len() > size {
            batches.push(std::mem::take(&mut cur));
        }
        if group.len() > size {
            let mut rest = group;
            while rest.len() > size {
                let tail = rest.split_off(size);
                batches.push(rest);
                rest = tail;
            }
            cur = rest;
        } else {
            cur.extend(group);
        }
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    batches
}

/// 성공하면 (복원까지 마친 번역문, 제어 문자가 맞지 않아 버린 항목 수)
async fn translate_batch(
    http: &reqwest::Client,
    s: &AiSettings,
    system: &str,
    glossary: &Glossary,
    batch: &[Job],
) -> Result<(Vec<TranslatedItem>, usize), String> {
    let user = prompt::user_message(batch, glossary);
    let mut last_err = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let content = match client::chat(http, s, system, &user).await {
            Ok(c) => c,
            Err(e) => {
                last_err = e.to_string();
                continue;
            }
        };
        let Some(map) = prompt::parse_response(&content) else {
            last_err = format!("응답을 JSON으로 해석할 수 없습니다: {}", content.chars().take(200).collect::<String>());
            continue;
        };
        let mut translated = Vec::new();
        let mut mismatched = 0;
        for (i, job) in batch.iter().enumerate() {
            let Some(text) = map.get(&i) else { continue };
            match job.masked.unmask(text, s.require_codes) {
                Some(text) => translated.push(TranslatedItem { id: job.id.clone(), text }),
                None => mismatched += 1,
            }
        }
        return Ok((translated, mismatched));
    }
    Err(last_err)
}

pub async fn run(
    app: &AppHandle,
    state: &AiState,
    settings: AiSettings,
    items: Vec<AiItem>,
    glossary: Vec<GlossaryTerm>,
) -> AiSummary {
    let glossary = Glossary::new(glossary);
    let total = items.len();
    let system = prompt::system_prompt(&settings.system_prompt, &settings.target_language);
    let http = reqwest::Client::builder().timeout(Duration::from_secs(300)).build().expect("HTTP 클라이언트 생성");
    let jobs: Vec<Job> = items
        .into_iter()
        .map(|item| Job { masked: codes::mask(&item.text), id: item.id, group: item.group, context: item.context })
        .filter(|job| job.masked.has_text())
        .collect();
    let skipped = total - jobs.len();
    let batches = make_batches(jobs, settings.batch_size);

    let mut summary = AiSummary { skipped, ..Default::default() };
    let mut mismatched = 0;
    let mut done = skipped;
    let _ = app.emit(EVENT_PROGRESS, Progress { done, total });

    let mut results = stream::iter(batches)
        .map(|batch| {
            let (http, settings, system, glossary) = (&http, &settings, &system, &glossary);
            async move {
                if state.cancel.load(Ordering::Relaxed) {
                    return (batch.len(), None);
                }
                (batch.len(), Some(translate_batch(http, settings, system, glossary, &batch).await))
            }
        })
        .buffer_unordered(settings.concurrency.max(1));

    loop {
        // 취소되면 스트림을 버려 진행 중인 요청까지 즉시 중단한다.
        let next = tokio::select! {
            next = results.next() => next,
            _ = state.cancelled() => {
                summary.cancelled = true;
                break;
            }
        };
        let Some((len, result)) = next else { break };
        done += len;
        match result {
            None => summary.cancelled = true,
            Some(Ok((translated, bad))) => {
                summary.translated += translated.len();
                summary.failed += len - translated.len();
                mismatched += bad;
                let _ = app.emit(EVENT_RESULT, translated);
            }
            Some(Err(e)) => {
                summary.failed += len;
                summary.errors.push(e);
            }
        }
        let _ = app.emit(EVENT_PROGRESS, Progress { done, total });
    }
    if mismatched > 0 {
        let reason = if settings.require_codes { "빠졌거나 중복되었거나 잘못된" } else { "잘못된" };
        summary.errors.push(format!("제어 문자 자리표시자가 {reason} 번역 {mismatched}개를 실패 처리했습니다."));
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(group: &str) -> Job {
        Job { id: String::new(), group: group.into(), context: None, masked: codes::mask("") }
    }

    #[test]
    fn keeps_groups_together() {
        let items = vec![item("a"), item("a"), item("b"), item("b"), item("b"), item("c")];
        let sizes: Vec<_> = make_batches(items, 4).iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![2, 4]);
        let items = (0..5).map(|_| item("a")).collect();
        let sizes: Vec<_> = make_batches(items, 2).iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![2, 2, 1]);
    }
}
