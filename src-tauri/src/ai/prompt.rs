use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::runner::AiItem;

/// 사용자 프롬프트 뒤에 항상 붙는 입출력 형식 지시. 응답을 기계적으로 파싱하기 위해 고정한다.
const FORMAT_INSTRUCTIONS: &str = r#"
## Input / output format
The user message is a JSON object: {"items":[{"id":number,"text":string,"group"?:number,"context"?:string}]}.
`context` is extra information such as the speaker name; do not translate it into the output.
Respond with ONLY a JSON object, without markdown code fences, of the form:
{"translations":[{"id":number,"text":string}]}
Include every id exactly once."#;

#[derive(Serialize)]
struct InputItem<'a> {
    id: usize,
    text: &'a str,
    group: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<&'a str>,
}

#[derive(Serialize)]
struct Input<'a> {
    items: Vec<InputItem<'a>>,
}

pub fn system_prompt(user_prompt: &str, language: &str) -> String {
    let language = if language.trim().is_empty() { "Korean" } else { language.trim() };
    let prompt = user_prompt.trim_end().replace("{{language}}", language);
    format!("{prompt}\n\nTarget language: {language}. Write every translated text in {language}.\n{FORMAT_INSTRUCTIONS}")
}

/// 토큰을 아끼기 위해 아이템 ID 대신 배치 내 순번을, 그룹 ID 대신 그룹 순번을 보낸다.
pub fn user_message(batch: &[AiItem]) -> String {
    let mut group_index: HashMap<&str, usize> = HashMap::new();
    let items = batch
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let next = group_index.len();
            InputItem {
                id: i,
                text: &item.text,
                group: *group_index.entry(&item.group).or_insert(next),
                context: item.context.as_deref(),
            }
        })
        .collect();
    serde_json::to_string(&Input { items }).expect("직렬화 실패 없음")
}

#[derive(Deserialize)]
struct Output {
    translations: Vec<OutputItem>,
}

#[derive(Deserialize)]
struct OutputItem {
    id: Value,
    text: String,
}

/// 응답에서 {배치 내 순번 → 번역문}을 뽑는다. 코드 펜스나 앞뒤 설명문이 섞여 있어도 허용한다.
pub fn parse_response(content: &str) -> Option<HashMap<usize, String>> {
    let start = content.find('{')?;
    let end = content.rfind('}')?;
    let out: Output = serde_json::from_str(content.get(start..=end)?).ok()?;
    Some(
        out.translations
            .into_iter()
            .filter_map(|t| {
                let id = match &t.id {
                    Value::Number(n) => n.as_u64()? as usize,
                    Value::String(s) => s.trim().parse().ok()?,
                    _ => return None,
                };
                Some((id, t.text))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fenced_response() {
        let r = parse_response("```json\n{\"translations\":[{\"id\":0,\"text\":\"안녕\"},{\"id\":\"1\",\"text\":\"잘가\"}]}\n```").unwrap();
        assert_eq!(r[&0], "안녕");
        assert_eq!(r[&1], "잘가");
    }
}
