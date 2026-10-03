//! 세부 수정 모드: 게임 폴더 아래의 그 밖의 JSON 파일(플러그인 전용 데이터 등)에서 문자열을 추출한다.
//!
//! RPG Maker 기본 data 파일은 전용 추출기가 처리하므로 제외한다.
//! package.json은 NW.js 설정 파일이라 제외한다 (name을 바꾸면 세이브 위치가 달라짐).
//! 무엇이 표시 문구인지 알 수 없으므로 비어 있지 않은 문자열은 모두 추출하고,
//! 문자열 안에 인코딩된 JSON은 플러그인 파라미터와 같은 방식으로 안쪽까지 따라 들어간다.

use std::collections::HashSet;
use std::fs;

use walkdir::{DirEntry, WalkDir};

use crate::jsonspan::{self, escape_token, Node, Value};
use crate::model::{Entry, Kind};
use crate::store::{ProjectOptions, WORK_DIR};

use super::plugins::{walk, Target};
use super::{is_standard_data_file, GameLayout, Sink};

/// 대상 파일을 찾아 추출한다. 읽거나 파싱할 수 없는 파일은 경고만 남기고 건너뛴다.
/// 언어 파일 쌍에 쓰인 파일은 locale 모듈이 처리하므로 제외한다.
pub fn extract_all(layout: &GameLayout, out: &mut Vec<Entry>, warnings: &mut Vec<String>, options: &ProjectOptions) {
    let paired: HashSet<&str> = options.locale_pairs.iter().flat_map(|p| [p.source.as_str(), p.target.as_str()]).collect();
    for file in find_files(layout) {
        if paired.contains(file.as_str()) {
            continue;
        }
        match read(layout, &file) {
            Ok(root) => extract(&root, Kind::JsonData, &mut Sink::new(&file, out, options)),
            Err(msg) => warnings.push(format!("{file}: {msg}")),
        }
    }
}

/// 루트 기준 상대 경로의 JSON 파일을 읽어 파싱한다.
pub(super) fn read(layout: &GameLayout, file: &str) -> Result<Node, String> {
    let path = file.split('/').fold(layout.root.clone(), |p, part| p.join(part));
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    jsonspan::parse(&text).map_err(|e| e.to_string())
}

/// 루트 기준 상대 경로('/' 구분) 목록. 정렬해서 돌려준다.
fn find_files(layout: &GameLayout) -> Vec<String> {
    // 작업 폴더(.rmtrans)를 포함한 숨김 폴더와 node_modules는 들어가지 않는다
    let skip_dir = |e: &DirEntry| {
        let name = e.file_name().to_string_lossy();
        e.depth() > 0 && e.file_type().is_dir() && (name.starts_with('.') || name == WORK_DIR || name == "node_modules")
    };
    let mut files: Vec<String> = WalkDir::new(&layout.root)
        .into_iter()
        .filter_entry(|e| !skip_dir(e))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(&layout.root).ok()?;
            let parts: Vec<_> = rel.iter().map(|p| p.to_str()).collect::<Option<_>>()?;
            Some(parts.join("/"))
        })
        .filter(|rel| {
            let (dir, name) = rel.rsplit_once('/').unwrap_or(("", rel));
            name.to_ascii_lowercase().ends_with(".json")
                && name != "package.json"
                && !(dir == layout.data_rel && is_standard_data_file(name))
        })
        .collect();
    files.sort();
    files
}

/// 비어 있지 않은 문자열을 모두 추출한다. 최상위 키(배열이면 인덱스)마다 그룹을 하나씩 만든다.
pub(super) fn extract(root: &Node, kind: Kind, sink: &mut Sink) {
    let name = sink.file.rsplit('/').next().unwrap_or(sink.file).to_string();
    let top = |path: String, key: &str, node: &Node, sink: &mut Sink| {
        let group = sink.group_id(&path);
        let label = format!("{name} · {key}");
        let target = Target { kind, group: &group, label: &label, all: true };
        walk(node, &path, "", &target, sink, 0);
    };
    match &root.value {
        Value::Object(fields) => {
            for (key, value) in fields {
                top(format!("/{}", escape_token(key)), key, value, sink);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                top(format!("/{i}"), &format!("#{i}"), item, sink);
            }
        }
        _ => top(String::new(), "", root, sink),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Engine;

    #[test]
    fn finds_external_json() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-jsonfiles-{}", std::process::id()));
        let root = base.join("game");
        for dir in ["www/data", "www/data/extra", "www/js/plugins", ".rmtrans", "www/node_modules/x"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        for file in [
            "www/data/System.json",
            "www/data/Map001.json",
            "www/data/MapInfos.json",
            "www/data/Quests.json",
            "www/data/extra/Map002.json",
            "www/js/plugins/Text.JSON",
            "www/js/plugins.js",
            "package.json",
            ".rmtrans/project.json",
            "www/node_modules/x/a.json",
        ] {
            fs::write(root.join(file), "{}").unwrap();
        }
        let layout = GameLayout { root: root.clone(), data_rel: "www/data".into(), engine: Engine::Mv };
        // data 폴더 밖의 Map002.json은 기본 데이터가 아니므로 포함
        assert_eq!(find_files(&layout), ["www/data/Quests.json", "www/data/extra/Map002.json", "www/js/plugins/Text.JSON"]);
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn extracts_all_strings() {
        let src = r#"{"quests":[{"title":"薬草集め","reward":100,"done":false,"tag":"q1"}],"version":"1.0","empty":""}"#;
        let root = jsonspan::parse(src).unwrap();
        let mut out = Vec::new();
        extract(&root, Kind::JsonData, &mut Sink { file: "www/data/Quests.json", out: &mut out, plugins: false, detailed: true });
        let got: Vec<_> =
            out.iter().map(|e| (e.path.as_str(), e.original.as_str(), e.group_label.as_str(), e.context.as_deref())).collect();
        assert_eq!(
            got,
            [
                ("/quests/0/title", "薬草集め", "Quests.json · quests", Some("1 › title")),
                ("/quests/0/tag", "q1", "Quests.json · quests", Some("1 › tag")),
                ("/version", "1.0", "Quests.json · version", None),
            ]
        );
        assert!(out.iter().all(|e| e.kind == Kind::JsonData));
    }
}
