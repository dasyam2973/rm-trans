use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::rpgm::apply;
use crate::store::WORK_DIR;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub files_copied: usize,
    pub files_patched: usize,
    pub strings_applied: usize,
    /// 적용하지 못한 아이템 ID
    pub skipped: Vec<String>,
}

/// 게임 폴더 전체를 dest로 복사한 뒤 번역문을 적용한다. 원본 폴더는 건드리지 않는다.
/// `translations`: 아이템 ID("{file}#{pointer}") → 번역문 (원문과 다른 것만 보내면 된다)
#[tauri::command]
pub async fn export_project(root: String, dest: String, translations: HashMap<String, String>) -> Result<ExportReport> {
    tauri::async_runtime::spawn_blocking(move || export(Path::new(&root), Path::new(&dest), translations))
        .await
        .expect("export_project 작업 패닉")
}

fn export(root: &Path, dest: &Path, translations: HashMap<String, String>) -> Result<ExportReport> {
    let root = root.canonicalize().map_err(|e| Error::io(root, e))?;
    if dest.exists() {
        let mut it = fs::read_dir(dest).map_err(|e| Error::io(dest, e))?;
        if it.next().is_some() {
            return Err(Error::msg(format!("대상 폴더가 비어 있지 않습니다: {}", dest.display())));
        }
    }
    fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;
    let dest = dest.canonicalize().map_err(|e| Error::io(dest, e))?;
    if dest.starts_with(&root) {
        fs::remove_dir(&dest).ok();
        return Err(Error::msg("원본 폴더 안에는 저장할 수 없습니다."));
    }

    let files_copied = copy_tree(&root, &dest)?;

    // 파일별로 묶어서 한 번씩만 읽고 쓴다
    let mut by_file: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (id, text) in translations {
        if let Some((file, pointer)) = id.split_once('#') {
            by_file.entry(file.to_string()).or_default().push((pointer.to_string(), text));
        }
    }

    let mut report = ExportReport { files_copied, files_patched: 0, strings_applied: 0, skipped: Vec::new() };
    for (file, patches) in by_file {
        // canonicalize된 Windows 경로(\\?\)는 '/'를 구분자로 인식하지 않으므로 구성 요소별로 붙인다
        let path = file.split('/').fold(dest.clone(), |p, part| p.join(part));
        let src = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let result = apply::patch_file(&file, &src, &patches).map_err(|msg| Error::Parse { path: file.clone(), msg })?;
        report.skipped.extend(result.skipped.into_iter().map(|p| format!("{file}#{p}")));
        if result.applied > 0 {
            fs::write(&path, result.text).map_err(|e| Error::io(&path, e))?;
            report.files_patched += 1;
            report.strings_applied += result.applied;
        }
    }
    Ok(report)
}

/// 작업 폴더(.rmtrans)를 제외하고 디렉토리 트리를 복사한다.
fn copy_tree(src: &Path, dest: &Path) -> Result<usize> {
    let mut count = 0;
    let walker = WalkDir::new(src).min_depth(1).into_iter().filter_entry(|e| !(e.depth() == 1 && e.file_name() == WORK_DIR));
    for entry in walker {
        let entry = entry.map_err(|e| Error::msg(e.to_string()))?;
        let rel = entry.path().strip_prefix(src).expect("walkdir 경로는 src 하위");
        let target: PathBuf = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(|e| Error::io(&target, e))?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| Error::io(entry.path(), e))?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpgm::{detect, extract};

    #[test]
    fn extract_then_export_roundtrip() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-{}", std::process::id()));
        let game = base.join("game");
        let data = game.join("www/data");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(game.join(WORK_DIR)).unwrap();
        fs::write(game.join(WORK_DIR).join("project.json"), "{}").unwrap();
        fs::write(data.join("System.json"), r#"{"gameTitle":"勇者","terms":{"basic":["レベル",""],"messages":{"victory":"%1の勝利！"}}}"#).unwrap();
        fs::write(data.join("Actors.json"), "[\nnull,\n{\"id\":1,\"name\":\"ハロルド\",\"nickname\":\"\",\"profile\":\"説明\"}\n]").unwrap();
        fs::write(data.join("MapInfos.json"), r#"[null,{"id":1,"name":"村"}]"#).unwrap();
        let map = r#"{"displayName":"始まりの村","events":[null,{"id":1,"name":"村人","pages":[{"list":[
{"code":101,"indent":0,"parameters":["",0,0,2,"村人"]},
{"code":401,"indent":0,"parameters":["こんにちは"]},
{"code":401,"indent":0,"parameters":["\\N[1]さん！"]},
{"code":102,"indent":0,"parameters":[["はい","いいえ"],1,0,2,0]},
{"code":0,"indent":0,"parameters":[]}]}]}]}"#;
        fs::write(data.join("Map001.json"), map).unwrap();

        let layout = detect::detect(&game).unwrap();
        assert_eq!(layout.data_rel, "www/data");
        let entries = extract::extract_all(&layout, false).unwrap().entries;
        let originals: Vec<_> = entries.iter().map(|e| e.original.as_str()).collect();
        assert_eq!(
            originals,
            ["勇者", "レベル", "%1の勝利！", "ハロルド", "説明", "始まりの村", "村人", "こんにちは", "\\N[1]さん！", "はい", "いいえ"]
        );
        // 화자 + 대사 두 줄은 같은 그룹, 선택지는 별도 그룹
        let dlg: Vec<_> = entries.iter().filter(|e| e.file.ends_with("Map001.json")).map(|e| &e.group).collect();
        assert_eq!(dlg[1], dlg[2]);
        assert_eq!(dlg[2], dlg[3]);
        assert_ne!(dlg[3], dlg[4]);
        assert_eq!(dlg[4], dlg[5]);

        let id = |o: &str| entries.iter().find(|e| e.original == o).unwrap().id.clone();
        let translations = HashMap::from([
            (id("こんにちは"), "안녕하세요".to_string()),
            (id("\\N[1]さん！"), "\\N[1]님!".to_string()),
            (id("ハロルド"), "해롤드".to_string()),
        ]);
        let dest = base.join("out");
        let report = export(&game, &dest, translations).unwrap();
        assert_eq!(report.strings_applied, 3);
        assert!(report.skipped.is_empty());
        assert!(!dest.join(WORK_DIR).exists());

        let out_map = fs::read_to_string(dest.join("www/data/Map001.json")).unwrap();
        assert_eq!(out_map, map.replace("こんにちは", "안녕하세요").replace("\\\\N[1]さん！", "\\\\N[1]님!"));
        let out_actors = fs::read_to_string(dest.join("www/data/Actors.json")).unwrap();
        assert_eq!(out_actors, "[\nnull,\n{\"id\":1,\"name\":\"해롤드\",\"nickname\":\"\",\"profile\":\"説明\"}\n]");

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn plugins_roundtrip() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-plugins-{}", std::process::id()));
        let game = base.join("game");
        fs::create_dir_all(game.join("data")).unwrap();
        fs::create_dir_all(game.join("js")).unwrap();
        fs::write(game.join("data/System.json"), r#"{"gameTitle":"勇者"}"#).unwrap();
        let cmd = r#"{"code":357,"indent":0,"parameters":["Pop","show","表示",{"text":"ようこそ","id":"5"}]}"#;
        fs::write(game.join("data/CommonEvents.json"), format!(r#"[null,{{"id":1,"name":"","list":[{cmd},{{"code":0,"indent":0,"parameters":[]}}]}}]"#)).unwrap();
        let plugins = "// Generated by RPG Maker.\n// Do not edit this file directly.\nvar $plugins =\n[\n{\"name\":\"Menu\",\"status\":true,\"description\":\"\",\"parameters\":{\"Title\":\"メニュー\",\"List\":\"[\\\"{\\\\\\\"Name\\\\\\\":\\\\\\\"回復\\\\\\\"}\\\"]\"}}\n];\n";
        fs::write(game.join("js/plugins.js"), plugins).unwrap();

        let layout = detect::detect(&game).unwrap();
        assert_eq!(layout.plugins_rel().as_deref(), Some("js/plugins.js"));
        assert_eq!(extract::extract_all(&layout, false).unwrap().entries.len(), 1);
        let ex = extract::extract_all(&layout, true).unwrap();
        assert!(ex.warnings.is_empty());
        let originals: Vec<_> = ex.entries.iter().map(|e| e.original.as_str()).collect();
        assert_eq!(originals, ["勇者", "ようこそ", "メニュー", "回復"]);

        let id = |o: &str| ex.entries.iter().find(|e| e.original == o).unwrap().id.clone();
        let translations = HashMap::from([
            (id("ようこそ"), "어서 오세요".to_string()),
            (id("メニュー"), "메뉴".to_string()),
            (id("回復"), "회복".to_string()),
        ]);
        let dest = base.join("out");
        let report = export(&game, &dest, translations).unwrap();
        assert_eq!(report.strings_applied, 3);
        assert!(report.skipped.is_empty());

        let out = fs::read_to_string(dest.join("js/plugins.js")).unwrap();
        assert_eq!(out, plugins.replace("メニュー", "메뉴").replace("回復", "회복"));
        let out_ce = fs::read_to_string(dest.join("data/CommonEvents.json")).unwrap();
        assert!(out_ce.contains(r#"{"text":"어서 오세요","id":"5"}"#));

        fs::remove_dir_all(&base).ok();
    }
}
