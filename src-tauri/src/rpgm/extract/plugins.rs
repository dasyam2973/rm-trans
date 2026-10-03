//! 플러그인 파라미터(js/plugins.js)와 플러그인 커맨드 인자에서 문자열을 추출한다.
//!
//! 플러그인 파라미터는 모두 문자열로 저장되며, 구조체/배열/노트 타입은
//! JSON 텍스트가 문자열 안에 한 번 더(때로는 여러 번) 인코딩되어 있다.
//! 이런 값은 안쪽까지 파싱해서 실제 문자열만 아이템으로 만들고,
//! 경로에는 jsonspan::NESTED 토큰을 끼워 넣어 내보내기 시 다시 인코딩할 수 있게 한다.
//!
//! 플러그인 값에는 파일명, 스위치 이름, 스크립트 같은 번역하면 안 되는 문자열도 섞여 있으므로
//! 숫자/불리언처럼 확실히 텍스트가 아닌 것만 걸러낸다. 세부 수정 모드에서는 이것도 거르지 않는다.

use crate::jsonspan::{self, escape_token, Node, Value, NESTED};
use crate::model::Kind;

use super::Sink;

/// 중첩 인코딩을 따라 들어가는 최대 깊이
const MAX_DEPTH: u8 = 8;

pub fn extract(root: &Node, sink: &mut Sink) {
    for (i, plugin) in root.as_array().unwrap_or_default().iter().enumerate() {
        // 꺼져 있는 플러그인은 게임에 영향이 없으므로 건너뛴다
        if plugin.get("status").and_then(Node::as_bool) == Some(false) {
            continue;
        }
        let Some(params) = plugin.get("parameters").and_then(Node::as_object) else { continue };
        let name = plugin.get("name").and_then(Node::as_str).unwrap_or("");
        let group = sink.group_id(&format!("/{i}"));
        let label = format!("플러그인 · {name}");
        let target = Target { kind: Kind::PluginParam, group: &group, label: &label, all: sink.detailed };
        for (key, value) in params {
            walk(value, &format!("/{i}/parameters/{}", escape_token(key)), key, &target, sink, 0);
        }
    }
}

/// MZ 플러그인 커맨드(357)의 인자 객체. parameters = [플러그인, 커맨드, 표시 이름, 인자]
pub fn extract_command_args(args: Option<&Node>, path: &str, group: &str, label: &str, sink: &mut Sink) {
    let Some(args) = args else { return };
    let target = Target { kind: Kind::PluginCommand, group, label, all: sink.detailed };
    walk(args, path, "", &target, sink, 0);
}

pub(super) struct Target<'a> {
    pub kind: Kind,
    pub group: &'a str,
    pub label: &'a str,
    /// 텍스트로 보이지 않는 값(숫자, 불리언 등)도 추출할지
    pub all: bool,
}

/// 노드 아래의 문자열 값을 모두 찾아 추출한다. 문자열 안에 인코딩된 JSON은 안쪽까지 따라 들어간다.
pub(super) fn walk(node: &Node, path: &str, ctx: &str, target: &Target, sink: &mut Sink, depth: u8) {
    let child_ctx = |key: &str| if ctx.is_empty() { key.to_string() } else { format!("{ctx} › {key}") };
    match &node.value {
        Value::Str(s) => {
            if depth < MAX_DEPTH {
                if let Some(inner) = parse_nested(s) {
                    let before = sink.out.len();
                    walk(&inner, &format!("{path}/{NESTED}"), ctx, target, sink, depth + 1);
                    // 안쪽에 문자열이 없으면("[1,2]", "[]" 등) 모두 추출할 때는 바깥 문자열을 통째로 수정할 수 있게 한다
                    if !(target.all && sink.out.len() == before) {
                        return;
                    }
                }
            }
            if target.all || is_text(s) {
                sink.push(Some(node), path.to_string(), target.kind, target.group, target.label, Some(ctx));
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk(item, &format!("{path}/{i}"), &child_ctx(&format!("{}", i + 1)), target, sink, depth);
            }
        }
        Value::Object(fields) => {
            for (key, value) in fields {
                walk(value, &format!("{path}/{}", escape_token(key)), &child_ctx(key), target, sink, depth);
            }
        }
        _ => {}
    }
}

/// 문자열 안에 인코딩된 JSON 배열/객체/문자열이면 파싱해서 돌려준다.
fn parse_nested(s: &str) -> Option<Node> {
    if !matches!(s.trim_start().as_bytes().first(), Some(b'[' | b'{' | b'"')) {
        return None;
    }
    let node = jsonspan::parse(s).ok()?;
    matches!(node.value, Value::Array(_) | Value::Object(_) | Value::Str(_)).then_some(node)
}

/// 번역 대상이 될 수 있는 텍스트인지 (숫자, 불리언, 기호만 있는 값 등은 제외)
fn is_text(s: &str) -> bool {
    let t = s.trim();
    !matches!(t, "" | "true" | "false" | "null") && t.parse::<f64>().is_err() && t.chars().any(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Entry;

    fn run(src: &str) -> Vec<Entry> {
        run_with(src, false)
    }

    fn run_with(src: &str, detailed: bool) -> Vec<Entry> {
        let root = jsonspan::parse(src).unwrap();
        let mut out = Vec::new();
        let mut sink = Sink { file: "js/plugins.js", out: &mut out, plugins: true, detailed };
        extract(&root, &mut sink);
        out
    }

    #[test]
    fn nested_params() {
        // 구조체 배열 파라미터: 문자열 안에 JSON 배열, 그 안에 다시 JSON 객체 문자열
        let list = serde_json::to_string(&[serde_json::to_string(&serde_json::json!({"Name":"回復","Id":"3"})).unwrap()]).unwrap();
        // 노트 타입 파라미터: JSON 문자열 리터럴이 한 번 더 인코딩됨
        let note = serde_json::to_string("一行目\n二行目").unwrap();
        let plugins = serde_json::json!([
            {"name":"Menu","status":true,"description":"","parameters":{"Title":"メニュー","Size":"28","Show":"true","List":list,"Help":note}},
            {"name":"Off","status":false,"description":"","parameters":{"Text":"無効"}}
        ]);
        let entries = run(&plugins.to_string());
        let got: Vec<_> = entries.iter().map(|e| (e.path.as_str(), e.original.as_str(), e.context.as_deref())).collect();
        // json!은 키를 정렬해서 직렬화한다
        assert_eq!(
            got,
            [
                ("/0/parameters/Help/~j", "一行目\n二行目", Some("Help")),
                ("/0/parameters/List/~j/0/~j/Name", "回復", Some("List › 1 › Name")),
                ("/0/parameters/Title", "メニュー", Some("Title")),
            ]
        );
    }

    #[test]
    fn detailed_keeps_values() {
        let list = serde_json::to_string(&[serde_json::to_string(&serde_json::json!({"Name":"回復","Id":"3"})).unwrap()]).unwrap();
        let plugins = serde_json::json!([
            {"name":"Menu","status":true,"description":"","parameters":{"Title":"メニュー","Size":"28","Show":"true","Ids":"[1,2]","Empty":"","List":list}},
            {"name":"Off","status":false,"description":"","parameters":{"Text":"無効"}}
        ]);
        let entries = run_with(&plugins.to_string(), true);
        let got: Vec<_> = entries.iter().map(|e| (e.path.as_str(), e.original.as_str())).collect();
        // 숫자/불리언 값과 문자열이 없는 중첩 JSON도 추출하고, 빈 값과 꺼진 플러그인은 여전히 제외
        assert_eq!(
            got,
            [
                ("/0/parameters/Ids", "[1,2]"),
                ("/0/parameters/List/~j/0/~j/Id", "3"),
                ("/0/parameters/List/~j/0/~j/Name", "回復"),
                ("/0/parameters/Show", "true"),
                ("/0/parameters/Size", "28"),
                ("/0/parameters/Title", "メニュー"),
            ]
        );
    }
}
