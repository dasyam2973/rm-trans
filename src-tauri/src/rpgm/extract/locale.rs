//! 번역 플러그인의 언어 파일 쌍 (원본_언어.json → 다른_언어.json).
//!
//! 항목은 원본 언어 파일의 구조로 만들되 ID는 대상 파일 기준으로 붙이고, 원문은 원본 언어의 값이다.
//! 대상 파일에 같은 경로의 값이 이미 있으면 `Entry::initial`로 넘겨 기존 번역으로 쓴다.
//! 내보낼 때는 원본 언어 파일에 번역을 적용해 대상 파일 경로에 쓰므로(commands/export.rs),
//! 대상 파일에서 빠져 있던 항목은 원본 언어로 채워지고 대상 파일에만 있던 항목은 빠진다.

use std::collections::HashSet;

use crate::jsonspan;
use crate::model::{Entry, Kind};
use crate::store::{LocalePair, ProjectOptions};

use super::{json_files, GameLayout, Sink};

pub fn extract_all(layout: &GameLayout, out: &mut Vec<Entry>, warnings: &mut Vec<String>, options: &ProjectOptions) {
    for pair in &options.locale_pairs {
        if let Err(msg) = extract_pair(layout, pair, out, warnings, options) {
            warnings.push(msg);
        }
    }
}

fn extract_pair(
    layout: &GameLayout,
    pair: &LocalePair,
    out: &mut Vec<Entry>,
    warnings: &mut Vec<String>,
    options: &ProjectOptions,
) -> Result<(), String> {
    let LocalePair { source, target } = pair;
    if source == target {
        return Err(format!("{source}: 원본과 대상 언어 파일이 같습니다."));
    }
    let src_root = json_files::read(layout, source).map_err(|e| format!("{source}: {e}"))?;
    // 대상 파일은 아직 없을 수 있다 (새 언어 추가)
    let target_exists = target.split('/').fold(layout.root.clone(), |p, part| p.join(part)).is_file();
    let tgt_root = match target_exists.then(|| json_files::read(layout, target)) {
        Some(Ok(root)) => Some(root),
        Some(Err(e)) => {
            warnings.push(format!("{target}: {e} (기존 번역을 가져오지 못했습니다)"));
            None
        }
        None => None,
    };

    let start = out.len();
    json_files::extract(&src_root, Kind::Locale, &mut Sink::new(target, out, options));
    let Some(tgt_root) = tgt_root else { return Ok(()) };

    for e in &mut out[start..] {
        // 비어 있는 값은 누락된 것으로 본다
        e.initial = jsonspan::resolve_str(&tgt_root, &e.path).filter(|t| !t.trim().is_empty() && *t != e.original);
    }
    let known: HashSet<&str> = out[start..].iter().map(|e| e.path.as_str()).collect();
    let mut target_only = Vec::new();
    json_files::extract(&tgt_root, Kind::Locale, &mut Sink::new(target, &mut target_only, options));
    let lost = target_only.iter().filter(|e| !known.contains(e.path.as_str())).count();
    if lost > 0 {
        warnings.push(format!("{target}: 원본 언어 파일({source})에 없는 항목 {lost}개는 내보낼 때 빠집니다."));
    }
    Ok(())
}
