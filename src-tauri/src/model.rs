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

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Engine {
    Mv,
    Mz,
    Unknown,
}
