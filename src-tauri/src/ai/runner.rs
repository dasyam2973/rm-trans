//! 번역 요청을 배치로 나눠 동시에 보내고, 결과를 이벤트로 프론트엔드에 흘려보낸다.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

use super::{client, prompt, settings::AiSettings};

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
fn make_batches(items: Vec<AiItem>, size: usize) -> Vec<Vec<AiItem>> {
    let size = size.max(1);
    let mut groups: Vec<Vec<AiItem>> = Vec::new();
    for item in items {
        match groups.last_mut() {
            Some(g) if g[0].group == item.group => g.push(item),
            _ => groups.push(vec![item]),
        }
    }

    let mut batches = Vec::new();
    let mut cur: Vec<AiItem> = Vec::new();
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

async fn translate_batch(http: &reqwest::Client, s: &AiSettings, system: &str, batch: &[AiItem]) -> Result<Vec<TranslatedItem>, String> {
    let user = prompt::user_message(batch);
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
        return Ok(batch
            .iter()
            .enumerate()
            .filter_map(|(i, item)| map.get(&i).map(|t| TranslatedItem { id: item.id.clone(), text: t.clone() }))
            .collect());
    }
    Err(last_err)
}

pub async fn run(app: &AppHandle, state: &AiState, settings: AiSettings, items: Vec<AiItem>) -> AiSummary {
    let total = items.len();
    let system = prompt::system_prompt(&settings.system_prompt, &settings.target_language);
    let http = reqwest::Client::builder().timeout(Duration::from_secs(300)).build().expect("HTTP 클라이언트 생성");
    let batches = make_batches(items, settings.batch_size);

    let mut summary = AiSummary::default();
    let mut done = 0;
    let _ = app.emit(EVENT_PROGRESS, Progress { done, total });

    let mut results = stream::iter(batches)
        .map(|batch| {
            let (http, settings, system) = (&http, &settings, &system);
            async move {
                if state.cancel.load(Ordering::Relaxed) {
                    return (batch.len(), None);
                }
                (batch.len(), Some(translate_batch(http, settings, system, &batch).await))
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
            Some(Ok(translated)) => {
                summary.translated += translated.len();
                summary.failed += len - translated.len();
                let _ = app.emit(EVENT_RESULT, translated);
            }
            Some(Err(e)) => {
                summary.failed += len;
                summary.errors.push(e);
            }
        }
        let _ = app.emit(EVENT_PROGRESS, Progress { done, total });
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(group: &str) -> AiItem {
        AiItem { id: String::new(), text: String::new(), group: group.into(), context: None }
    }

    #[test]
    fn keeps_groups_together() {
        let items = vec![item("a"), item("a"), item("b"), item("b"), item("b"), item("c")];
        let sizes: Vec<_> = make_batches(items, 4).iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![2, 4]);
        let items = vec![item("a"); 5];
        let sizes: Vec<_> = make_batches(items, 2).iter().map(Vec::len).collect();
        assert_eq!(sizes, vec![2, 2, 1]);
    }
}
