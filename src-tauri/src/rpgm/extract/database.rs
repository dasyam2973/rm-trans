use crate::jsonspan::Node;
use crate::model::Kind;

use super::events;
use super::Sink;

/// 데이터베이스 파일별 번역 대상 필드
fn fields_of(stem: &str) -> &'static [(&'static str, Kind)] {
    use Kind::*;
    match stem {
        "Actors" => &[("name", Name), ("nickname", Nickname), ("profile", Profile)],
        "Classes" | "Enemies" => &[("name", Name)],
        "Skills" => &[("name", Name), ("description", Description), ("message1", Message), ("message2", Message)],
        "Items" | "Weapons" | "Armors" => &[("name", Name), ("description", Description)],
        "States" => &[
            ("name", Name),
            ("message1", Message),
            ("message2", Message),
            ("message3", Message),
            ("message4", Message),
        ],
        _ => &[],
    }
}

fn record_label(stem: &str, record: &Node) -> String {
    let id = record.get("id").and_then(Node::as_i64).unwrap_or(0);
    let name = record.get("name").and_then(Node::as_str).unwrap_or("");
    format!("{stem} #{id} {name}").trim_end().to_string()
}

/// [null, {id, name, ...}, ...] 형태의 레코드 배열. 레코드 하나가 그룹 하나.
pub fn extract_records(stem: &str, root: &Node, sink: &mut Sink) {
    let fields = fields_of(stem);
    for (i, record) in root.as_array().unwrap_or_default().iter().enumerate() {
        if record.as_object().is_none() {
            continue;
        }
        let anchor = format!("/{i}");
        let group = sink.group_id(&anchor);
        let label = record_label(stem, record);
        for &(key, kind) in fields {
            sink.push(record.get(key), format!("{anchor}/{key}"), kind, &group, &label, Some(key));
        }
    }
}

pub fn extract_troops(root: &Node, sink: &mut Sink) {
    for (i, troop) in root.as_array().unwrap_or_default().iter().enumerate() {
        let Some(pages) = troop.get("pages").and_then(Node::as_array) else { continue };
        let label = record_label("Troops", troop);
        for (p, page) in pages.iter().enumerate() {
            if let Some(list) = page.get("list") {
                events::extract_list(list, &format!("/{i}/pages/{p}/list"), &format!("{label} · P{}", p + 1), sink);
            }
        }
    }
}

pub fn extract_common_events(root: &Node, sink: &mut Sink) {
    for (i, ev) in root.as_array().unwrap_or_default().iter().enumerate() {
        if let Some(list) = ev.get("list") {
            events::extract_list(list, &format!("/{i}/list"), &record_label("CommonEvents", ev), sink);
        }
    }
}
