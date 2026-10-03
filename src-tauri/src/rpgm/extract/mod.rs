//! RPG Maker data 폴더의 JSON에서 번역 가능한 문자열을 뽑아낸다.

mod database;
mod events;
mod json_files;
mod locale;
mod map;
mod plugins;
mod system;

use std::collections::HashMap;
use std::fs;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::jsonspan::{self, Node};
use crate::model::{Entry, Kind};
use crate::store::ProjectOptions;

use super::detect::GameLayout;
use super::plugins_js;

/// 데이터베이스 파일 표시 순서
const DB_ORDER: &[&str] = &[
    "System", "Actors", "Classes", "Skills", "Items", "Weapons", "Armors", "Enemies", "States", "Troops",
    "CommonEvents",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extracted {
    pub entries: Vec<Entry>,
    /// 추출은 계속했지만 사용자에게 알려야 하는 문제 (플러그인 파일을 읽지 못함 등)
    pub warnings: Vec<String>,
}

/// 추출 결과를 모으는 대상. 파일 하나 단위로 만든다.
pub(crate) struct Sink<'a> {
    file: &'a str,
    out: &'a mut Vec<Entry>,
    /// 플러그인 커맨드 인자도 추출할지
    plugins: bool,
    /// 세부 수정: 플러그인 데이터에서 숫자/불리언 같은 값도 걸러내지 않는다
    detailed: bool,
}

impl<'a> Sink<'a> {
    fn new(file: &'a str, out: &'a mut Vec<Entry>, options: &ProjectOptions) -> Self {
        Sink { file, out, plugins: options.include_plugins, detailed: options.detailed }
    }

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
            initial: None,
        });
    }
}

pub(crate) fn read_json(layout: &GameLayout, name: &str) -> Result<Node> {
    let path = layout.data_dir().join(name);
    let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    jsonspan::parse(&text).map_err(|e| Error::Parse { path: layout.rel_file(name), msg: e.to_string() })
}

/// `options.include_plugins`: 플러그인 파라미터(js/plugins.js)와 플러그인 커맨드 인자도 추출할지
/// `options.detailed`: 그 밖의 JSON 파일도 추출하고, 플러그인 값도 거르지 않는다
pub fn extract_all(layout: &GameLayout, options: &ProjectOptions) -> Result<Extracted> {
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
    let mut warnings = Vec::new();
    for stem in DB_ORDER {
        let name = format!("{stem}.json");
        if !data_dir.join(&name).is_file() {
            continue;
        }
        let root = read_json(layout, &name)?;
        let file = layout.rel_file(&name);
        let mut sink = Sink::new(&file, &mut out, options);
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
        let mut sink = Sink::new(&file, &mut out, options);
        map::extract(name, &root, &map_names, &mut sink);
    }

    if options.include_plugins {
        if let Err(msg) = extract_plugins_js(layout, &mut out, options) {
            warnings.push(msg);
        }
    }

    locale::extract_all(layout, &mut out, &mut warnings, options);

    if options.detailed {
        json_files::extract_all(layout, &mut out, &mut warnings, options);
    }

    Ok(Extracted { entries: out, warnings })
}

/// RPG Maker가 기본으로 쓰는 data 폴더 파일인지 (세부 수정의 외부 JSON 추출에서 제외)
pub(crate) fn is_standard_data_file(name: &str) -> bool {
    map::is_map_file(name)
        || name.strip_suffix(".json").is_some_and(|stem| {
            DB_ORDER.contains(&stem) || matches!(stem, "MapInfos" | "Animations" | "Tilesets")
        })
}

/// js/plugins.js의 플러그인 파라미터. 파일이 없으면 조용히 넘어가고, 읽을 수 없으면 경고 메시지를 돌려준다.
fn extract_plugins_js(layout: &GameLayout, out: &mut Vec<Entry>, options: &ProjectOptions) -> std::result::Result<(), String> {
    let Some(file) = layout.plugins_rel() else { return Ok(()) };
    let path = file.split('/').fold(layout.root.clone(), |p, part| p.join(part));
    if !path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("{file}: {e}"))?;
    let range = plugins_js::json_range(&text).ok_or_else(|| format!("{file}: $plugins 배열을 찾을 수 없습니다."))?;
    let root = jsonspan::parse(&text[range]).map_err(|e| format!("{file}: {e}"))?;
    plugins::extract(&root, &mut Sink::new(&file, out, options));
    Ok(())
}
