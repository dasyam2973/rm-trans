use std::collections::HashMap;

use crate::jsonspan::Node;
use crate::model::Kind;

use super::events;
use super::Sink;

/// "Map001.json" 형태인지 (MapInfos.json 제외)
pub fn is_map_file(name: &str) -> bool {
    name.strip_prefix("Map")
        .and_then(|s| s.strip_suffix(".json"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

pub fn extract(name: &str, root: &Node, map_names: &HashMap<i64, String>, sink: &mut Sink) {
    let stem = name.trim_end_matches(".json");
    let id: i64 = stem.trim_start_matches("Map").parse().unwrap_or(0);
    let map_label = match map_names.get(&id) {
        Some(n) if !n.is_empty() => format!("{stem} {n}"),
        _ => stem.to_string(),
    };

    let group = sink.group_id("/displayName");
    sink.push(root.get("displayName"), "/displayName".into(), Kind::DisplayName, &group, &map_label, None);

    for (e, event) in root.get("events").and_then(Node::as_array).unwrap_or_default().iter().enumerate() {
        let Some(pages) = event.get("pages").and_then(Node::as_array) else { continue };
        let ev_id = event.get("id").and_then(Node::as_i64).unwrap_or(e as i64);
        let ev_name = event.get("name").and_then(Node::as_str).unwrap_or("");
        for (p, page) in pages.iter().enumerate() {
            if let Some(list) = page.get("list") {
                let label = format!("{map_label} · EV{ev_id:03} {ev_name} · P{}", p + 1);
                events::extract_list(list, &format!("/events/{e}/pages/{p}/list"), &label, sink);
            }
        }
    }
}
