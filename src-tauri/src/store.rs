//! 작업 상태 파일 (<게임 루트>/.rmtrans/project.json)

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::rpgm::assets::SchemeKey;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectOptions {
    /// 플러그인 파라미터/커맨드도 추출할지
    pub include_plugins: bool,
    /// 세부 수정: 게임 폴더의 다른 JSON 파일도 추출하고, 플러그인 데이터의 숫자/불리언 같은 값도 걸러내지 않는다
    pub detailed: bool,
    /// 번역 플러그인의 언어 파일 쌍
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locale_pairs: Vec<LocalePair>,
    /// 게임 자체 스크립트 등 텍스트 파일 추출 규칙 (엔진과 무관)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub text_rules: Vec<TextRule>,
}

/// 텍스트 파일 추출 규칙 (`textfile.rs`).
/// 지정한 접두로 시작하지 않는 줄이 이어진 묶음을 문장 하나로, 지정한 명령의 인자를 문장 하나로 추출한다.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextRule {
    /// 게임 루트 기준 파일 패턴 ('/' 구분, `*` `**` `?`). 예: `Data/Text_Script/**/*.txt`
    pub pattern: String,
    /// 이 문자열로 시작하는 줄은 명령·주석이라 번역하지 않는다. 예: `@`, `#`
    pub skip_prefixes: Vec<String>,
    /// 인자에 표시 문장이 들어 있는 명령
    pub arg_commands: Vec<ArgCommand>,
}

/// `command`로 시작하는 줄을 공백으로 나눴을 때 `arg`번째(명령 자신이 0) 토큰이 표시 문장이다
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgCommand {
    pub command: String,
    pub arg: usize,
}

/// 번역 플러그인의 언어별 JSON 파일 쌍. 경로는 게임 루트 기준 ('/' 구분).
/// 원본 언어 파일의 구조로 항목을 만들고, 내보낼 때 원본 파일에 번역을 적용해 대상 파일로 쓴다.
/// 대상 파일에서 빠져 있던 항목은 원본 언어로 채워진다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalePair {
    pub source: String,
    pub target: String,
}

/// 단어장 항목. 인명/고유명사 등을 일관되게 번역하도록 AI 요청에 함께 보낸다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryTerm {
    pub source: String,
    pub target: String,
    /// 성별, 말투 등 참고 메모 (AI에도 전달)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub version: u32,
    /// 아이템 ID → 저장된 번역 정보. BTreeMap으로 저장 순서를 고정해 diff를 안정적으로 유지
    pub entries: BTreeMap<String, SavedEntry>,
    #[serde(default)]
    pub options: ProjectOptions,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glossary: Vec<GlossaryTerm>,
    /// 사용자가 직접 입력한 리소스 암호화 키 (감지한 키보다 우선)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_keys: Vec<SchemeKey>,
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
