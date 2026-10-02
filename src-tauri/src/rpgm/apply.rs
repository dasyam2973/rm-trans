//! 번역문을 JSON 원문에 적용한다.
//!
//! 파일을 다시 직렬화하지 않고 대상 문자열 값의 바이트 구간만 교체하므로
//! 번역하지 않은 부분은 원본과 바이트 단위로 동일하게 유지된다.

use crate::jsonspan;

pub struct PatchResult {
    pub text: String,
    pub applied: usize,
    /// 경로를 찾지 못했거나 문자열이 아니어서 건너뛴 포인터
    pub skipped: Vec<String>,
}

/// RPG Maker(JSON.stringify)와 같은 방식으로 문자열을 JSON 리터럴로 만든다.
/// 비ASCII 문자는 이스케이프하지 않는다.
fn to_json_string(s: &str) -> String {
    serde_json::to_string(s).expect("문자열 직렬화는 실패하지 않음")
}

pub fn patch(src: &str, patches: &[(String, String)]) -> Result<PatchResult, jsonspan::ParseError> {
    let root = jsonspan::parse(src)?;
    let mut spans = Vec::with_capacity(patches.len());
    let mut skipped = Vec::new();
    for (pointer, text) in patches {
        match root.pointer(pointer) {
            Some(node) if node.as_str().is_some() => spans.push((node.start, node.end, to_json_string(text))),
            _ => skipped.push(pointer.clone()),
        }
    }

    // 뒤에서부터 교체해야 앞쪽 오프셋이 유지된다
    spans.sort_by(|a, b| b.0.cmp(&a.0));
    spans.dedup_by_key(|s| s.0);
    let mut text = src.to_string();
    for (start, end, replacement) in &spans {
        text.replace_range(start..end, replacement);
    }
    Ok(PatchResult { text, applied: spans.len(), skipped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_untouched_bytes() {
        let src = "[\nnull,\n{\"id\":1,\"name\":\"ハロルド\",\"note\":\"<x:1.50>\",\"v\":1.50}\n]";
        let r = patch(src, &[("/1/name".into(), "하롤드 \"\\".into())]).unwrap();
        assert_eq!(r.applied, 1);
        assert_eq!(r.text, "[\nnull,\n{\"id\":1,\"name\":\"하롤드 \\\"\\\\\",\"note\":\"<x:1.50>\",\"v\":1.50}\n]");
    }
}
