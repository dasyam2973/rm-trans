use crate::jsonspan::{escape_token, Node};
use crate::model::Kind;

use super::Sink;

/// 문자열 배열 하나를 그룹 하나로 추출
fn string_array(root: &Node, path: &str, label: &str, sink: &mut Sink) {
    let Some(arr) = root.pointer(path).and_then(Node::as_array) else { return };
    let group = sink.group_id(path);
    for (i, item) in arr.iter().enumerate() {
        sink.push(Some(item), format!("{path}/{i}"), Kind::Term, &group, label, None);
    }
}

pub fn extract(root: &Node, sink: &mut Sink) {
    let group = sink.group_id("");
    for key in ["gameTitle", "currencyUnit"] {
        sink.push(root.get(key), format!("/{key}"), Kind::Term, &group, "System", Some(key));
    }

    for (path, label) in [
        ("/elements", "System · 속성"),
        ("/skillTypes", "System · 스킬 타입"),
        ("/weaponTypes", "System · 무기 타입"),
        ("/armorTypes", "System · 방어구 타입"),
        ("/equipTypes", "System · 장비 타입"),
        ("/terms/basic", "System · 용어 (기본 상태)"),
        ("/terms/commands", "System · 용어 (커맨드)"),
        ("/terms/params", "System · 용어 (능력치)"),
    ] {
        string_array(root, path, label, sink);
    }

    if let Some(messages) = root.pointer("/terms/messages").and_then(Node::as_object) {
        let group = sink.group_id("/terms/messages");
        for (key, node) in messages {
            let path = format!("/terms/messages/{}", escape_token(key));
            sink.push(Some(node), path, Kind::Message, &group, "System · 용어 (메시지)", Some(key));
        }
    }
}
