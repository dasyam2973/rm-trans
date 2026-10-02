//! 작업 상태 파일 (<게임 루트>/.rmtrans/project.json)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const WORK_DIR: &str = ".rmtrans";
const PROJECT_FILE: &str = "project.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Translated,
    Untranslated,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SavedEntry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    /// 사용자가 직접 지정한 상태 (없으면 원문/번역문 비교로 판정)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    pub version: u32,
    /// 아이템 ID → 저장된 번역 정보. BTreeMap으로 저장 순서를 고정해 diff를 안정적으로 유지
    pub entries: BTreeMap<String, SavedEntry>,
}

fn project_path(root: &Path) -> PathBuf {
    root.join(WORK_DIR).join(PROJECT_FILE)
}

pub fn load(root: &Path) -> Result<Option<ProjectFile>> {
    let path = project_path(root);
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let file = serde_json::from_str(&text).map_err(|e| Error::Parse { path: path.display().to_string(), msg: e.to_string() })?;
    Ok(Some(file))
}

pub fn save(root: &Path, file: &ProjectFile) -> Result<()> {
    let path = project_path(root);
    let dir = root.join(WORK_DIR);
    fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let text = serde_json::to_string_pretty(file)?;
    // 임시 파일에 쓴 뒤 교체해서 저장 도중 실패해도 기존 파일이 깨지지 않게 한다
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|e| Error::io(&tmp, e))?;
    fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))?;
    Ok(())
}
