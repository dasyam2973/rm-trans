//! RPG Maker data 폴더의 JSON에서 번역 가능한 문자열을 뽑아낸다.

mod database;
mod events;
mod map;
mod system;

use std::collections::HashMap;
use std::fs;

use crate::error::{Error, Result};
use crate::jsonspan::{self, Node};
use crate::model::{Entry, Kind};

use super::detect::GameLayout;

/// 데이터베이스 파일 표시 순서
const DB_ORDER: &[&str] = &[
    "System", "Actors", "Classes", "Skills", "Items", "Weapons", "Armors", "Enemies", "States", "Troops",
    "CommonEvents",
];

/// 추출 결과를 모으는 대상. 파일 하나 단위로 만든다.
pub(crate) struct Sink<'a> {
    file: &'a str,
    out: &'a mut Vec<Entry>,
}

impl Sink<'_> {
    /// 그룹 ID는 그룹의 기준이 되는 노드 경로로 만든다.
    pub fn group_id(&self, anchor: &str) -> String {
        format!("{}#{}", self.file, anchor)
    }

    /// 노드가 비어 있지 않은 문자열이면 아이템으로 추가한다.
    pub fn push(
        &mut self,
        node: Option<&Node>,
        path: String,
        kind: Kind,
        group: &str,
        group_label: &str,
        context: Option<&str>,
    ) {
        let Some(text) = node.and_then(Node::as_str) else { return };
        if text.trim().is_empty() {
            return;
        }
        self.out.push(Entry {
            id: format!("{}#{}", self.file, path),
            file: self.file.to_string(),
            path,
            kind,
            group: group.to_string(),
            group_label: group_label.to_string(),
            context: context.filter(|c| !c.is_empty()).map(str::to_string),
            original: text.to_string(),
        });
    }
}

pub(crate) fn read_json(layout: &GameLayout, name: &str) -> Result<Node> {
    let path = layout.data_dir().join(name);
    let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    jsonspan::parse(&text).map_err(|e| Error::Parse { path: layout.rel_file(name), msg: e.to_string() })
}

pub fn extract_all(layout: &GameLayout) -> Result<Vec<Entry>> {
    let data_dir = layout.data_dir();
    let mut map_files: Vec<String> = fs::read_dir(&data_dir)
        .map_err(|e| Error::io(&data_dir, e))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| map::is_map_file(n))
        .collect();
    map_files.sort();

    // 맵 표시 이름용 (에디터상의 맵 이름)
    let map_names: HashMap<i64, String> = match read_json(layout, "MapInfos.json") {
        Ok(infos) => infos
            .as_array()
            .unwrap_or_default()
            .iter()
            .filter_map(|info| Some((info.get("id")?.as_i64()?, info.get("name")?.as_str()?.to_string())))
            .collect(),
        Err(_) => HashMap::new(),
    };

    let mut out = Vec::new();
    for stem in DB_ORDER {
        let name = format!("{stem}.json");
        if !data_dir.join(&name).is_file() {
            continue;
        }
        let root = read_json(layout, &name)?;
        let file = layout.rel_file(&name);
        let mut sink = Sink { file: &file, out: &mut out };
        match *stem {
            "System" => system::extract(&root, &mut sink),
            "Troops" => database::extract_troops(&root, &mut sink),
            "CommonEvents" => database::extract_common_events(&root, &mut sink),
            _ => database::extract_records(stem, &root, &mut sink),
        }
    }

    for name in &map_files {
        let root = read_json(layout, name)?;
        let file = layout.rel_file(name);
        let mut sink = Sink { file: &file, out: &mut out };
        map::extract(name, &root, &map_names, &mut sink);
    }

    Ok(out)
}
