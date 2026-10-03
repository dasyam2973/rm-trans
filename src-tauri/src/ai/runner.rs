//! 번역 요청을 배치로 나눠 동시에 보내고, 결과를 이벤트로 프론트엔드에 흘려보낸다.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

use super::codes::{self, Masked};
use super::glossary::Glossary;
use super::{client, lines, prompt, settings::AiSettings};
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
    /// 대사 줄이고 그 블록의 대사 줄이 모두 이번 요청에 들어 있는지 (병합 번역 대상)
    #[serde(default)]
    pub merge: bool,
}

/// 제어 문자를 마스킹한 번역 대상
pub struct Job {
    /// 대상 아이템 ID. 병합된 대사 블록이면 줄 순서대로 여러 개
    pub ids: Vec<String>,
    pub group: String,
    pub context: Option<String>,
    pub masked: Masked,
    /// 병합된 블록의 번역문을 나눌 때 쓰는 한 줄 최대 폭
    line_limit: f32,
}

impl Job {
    pub fn new(item: AiItem) -> Self {
        Job { masked: codes::mask(&item.text), ids: vec![item.id], group: item.group, context: item.context, line_limit: 0.0 }
    }

    /// 대사 블록의 줄들을 이어 작업 하나로 만든다.
    fn merged(lines: Vec<AiItem>, max_width: f32) -> Self {
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        let masked = codes::mask(&lines::join(&texts));
        // 원문 줄이 이미 그만큼 넓었다면 창이 그 폭을 담을 수 있다고 본다
        let widest = texts.iter().map(|t| codes::display_width(t.trim())).fold(0.0, f32::max);
        let (group, context) = (lines[0].group.clone(), lines[0].context.clone());
        Job { ids: lines.into_iter().map(|l| l.id).collect(), group, context, masked, line_limit: max_width.max(widest) }
    }

    /// 복원까지 마친 번역문을 아이템별 결과로 바꾼다. 병합된 블록은 원래 줄 수에 맞춰 나눈다.
    fn results(&self, text: String) -> Vec<TranslatedItem> {
        if let [id] = self.ids.as_slice() {
            return vec![TranslatedItem { id: id.clone(), text }];
        }
        let parts = lines::split(&text, self.ids.len(), self.line_limit);
        self.ids.iter().zip(parts).map(|(id, text)| TranslatedItem { id: id.clone(), text }).collect()
    }
}

enum Slot {
    Single(AiItem),
    /// 병합할 그룹. 그 그룹 첫 줄의 자리에 둔다
    Merged(String),
}

/// 아이템을 번역 작업으로 바꾼다. 병합 대상 줄은 그룹별로 이어 작업 하나로 만든다.
fn build_jobs(items: Vec<AiItem>, settings: &AiSettings) -> Vec<Job> {
    let enabled = settings.merge_lines && lines::splits_by_space(&settings.target_language);
    let mut merged: HashMap<String, Vec<AiItem>> = HashMap::new();
    let mut slots = Vec::new();
    for item in items {
        if enabled && item.merge {
            let lines = merged.entry(item.group.clone()).or_default();
            if lines.is_empty() {
                slots.push(Slot::Merged(item.group.clone()));
            }
            lines.push(item);
        } else {
            slots.push(Slot::Single(item));
        }
    }
    slots
        .into_iter()
        .flat_map(|slot| match slot {
            Slot::Single(item) => vec![Job::new(item)],
            Slot::Merged(group) => {
                let lines = merged.remove(&group).expect("슬롯을 만들 때 넣었다");
                if lines.len() == 1 {
                    lines.into_iter().map(Job::new).collect()
                } else {
                    vec![Job::merged(lines, settings.max_line_width)]
                }
            }
        })
        .collect()
}

/// 작업들이 담고 있는 아이템 수
fn item_count(jobs: &[Job]) -> usize {
    jobs.iter().map(|j| j.ids.len()).sum()
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
                Some(text) => translated.extend(job.results(text)),
                None => mismatched += job.ids.len(),
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
    let jobs: Vec<Job> = build_jobs(items, &settings).into_iter().filter(|job| job.masked.has_text()).collect();
    let skipped = total - item_count(&jobs);
    let batches = make_batches(jobs, settings.batch_size);

    let mut summary = AiSummary { skipped, ..Default::default() };
    let mut mismatched = 0;
    let mut done = skipped;
    let _ = app.emit(EVENT_PROGRESS, Progress { done, total });

    let mut results = stream::iter(batches)
        .map(|batch| {
            let (http, settings, system, glossary) = (&http, &settings, &system, &glossary);
            async move {
                let len = item_count(&batch);
                if state.cancel.load(Ordering::Relaxed) {
                    return (len, None);
                }
                (len, Some(translate_batch(http, settings, system, glossary, &batch).await))
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

    fn ai_item(id: &str, text: &str, group: &str, merge: bool) -> AiItem {
        AiItem { id: id.into(), text: text.into(), group: group.into(), context: None, merge }
    }

    fn item(group: &str) -> Job {
        Job::new(ai_item("", "", group, false))
    }

    #[test]
    fn merges_whole_blocks() {
        let items = vec![
            ai_item("s", "ハロルド", "a", false),
            ai_item("1", "昨日、村の外れで", "a", true),
            ai_item("2", "光を見た。", "a", true),
            ai_item("3", "一行だけ", "b", true),
            ai_item("4", "部分", "c", false),
        ];
        let jobs = build_jobs(items.clone(), &AiSettings::default());
        let ids: Vec<_> = jobs.iter().map(|j| j.ids.join(",")).collect();
        assert_eq!(ids, ["s", "1,2", "3", "4"]);
        assert_eq!(jobs[1].masked.text, "昨日、村の外れで光を見た。");

        // 띄어쓰기를 쓰지 않는 대상 언어나 설정이 꺼져 있으면 병합하지 않는다
        let settings = AiSettings { target_language: "Japanese".into(), ..AiSettings::default() };
        assert_eq!(build_jobs(items.clone(), &settings).len(), 5);
        let settings = AiSettings { merge_lines: false, ..AiSettings::default() };
        assert_eq!(build_jobs(items, &settings).len(), 5);
    }

    #[test]
    fn splits_merged_result() {
        let job = Job::merged(vec![ai_item("1", "昨日、村の外れで", "a", true), ai_item("2", "光を見た。", "a", true)], 22.0);
        let results = job.results("어제 마을 밖에서 빛을 봤다.".into());
        let texts: Vec<_> = results.iter().map(|r| (r.id.as_str(), r.text.as_str())).collect();
        assert_eq!(texts, [("1", "어제 마을 밖에서 빛을 봤다."), ("2", "")]);
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
