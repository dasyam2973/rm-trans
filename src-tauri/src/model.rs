use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Name,
    Nickname,
    Profile,
    Description,
    Message,
    Term,
    Speaker,
    Dialogue,
    Scroll,
    Choice,
    DisplayName,
    /// js/plugins.js의 플러그인 파라미터
    PluginParam,
    /// 이벤트의 플러그인 커맨드 인자 (MZ 357)
    PluginCommand,
    /// 세부 수정 모드에서 추출하는 그 밖의 JSON 파일(플러그인 전용 데이터 등)의 값
    JsonData,
    /// 번역 플러그인의 언어 파일 값
    Locale,
    /// WOLF RPG: 그림으로 표시하는 문자열
    PictureText,
    /// WOLF RPG: 문자열 변수 대입, 문자열 비교, DB 쓰기, 이름으로 커먼 이벤트 호출 등의 문자열 인자.
    /// 다른 곳에서 비교하거나 키로 쓰는 값일 수 있어 위험 항목으로 다룬다
    StringArg,
    /// WOLF RPG: 데이터베이스의 문자열 값
    DbValue,
    /// 텍스트 파일 규칙: 명령이 아닌 줄이 이어진 묶음 (게임 자체 스크립트의 대사 등)
    ScriptText,
    /// 텍스트 파일 규칙: 명령 줄의 인자 (선택지 등)
    ScriptArg,
}

/// 번역 가능한 문자열 하나. 원본 JSON의 문자열 값 하나와 1:1로 대응한다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// "{file}#{pointer}" 형태의 고유 ID
    pub id: String,
    /// 게임 루트 기준 상대 경로 (구분자는 '/')
    pub file: String,
    /// 파일 내 JSON Pointer
    pub path: String,
    pub kind: Kind,
    /// 같이 보여줄 아이템 묶음의 ID (대사 블록, 선택지, DB 레코드 등)
    pub group: String,
    pub group_label: String,
    /// 화자 이름 등 번역에 참고할 부가 정보
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    pub original: String,
    /// 언어 파일 쌍의 대상 파일에 이미 있던 값 (원문과 다를 때만). 저장된 번역이 없으면 번역으로 채운다
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial: Option<String>,
}

impl Entry {
    /// 비어 있지 않은 문자열이면 아이템을 만든다. ID는 "{file}#{path}"
    pub fn text(
        file: &str,
        path: String,
        kind: Kind,
        group: &str,
        group_label: &str,
        context: Option<&str>,
        text: &str,
    ) -> Option<Entry> {
        if text.trim().is_empty() {
            return None;
        }
        Some(Entry {
            id: format!("{file}#{path}"),
            file: file.to_string(),
            path,
            kind,
            group: group.to_string(),
            group_label: group_label.to_string(),
            context: context.filter(|c| !c.is_empty()).map(str::to_string),
            original: text.to_string(),
            initial: None,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    Mv,
    Mz,
    Unknown,
    /// WOLF RPG 2.x (Shift-JIS)
    Wolf2,
    /// WOLF RPG 3.x (UTF-8)
    Wolf3,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extracted {
    pub entries: Vec<Entry>,
    /// 추출은 계속했지만 사용자에게 알려야 하는 문제 (플러그인 파일을 읽지 못함 등)
    pub warnings: Vec<String>,
}
