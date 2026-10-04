use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::rpgm::apply;
use crate::rpgm::assets::{self, join_rel};
use crate::store::{LocalePair, WORK_DIR};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub files_copied: usize,
    pub files_patched: usize,
    pub strings_applied: usize,
    /// 번역 이미지를 쓴 파일 수
    pub images_applied: usize,
    /// 적용하지 못한 아이템 ID, 번역 이미지 오류
    pub skipped: Vec<String>,
}

/// 게임 폴더 전체를 dest로 복사한 뒤 번역문을 적용한다. 원본 폴더는 건드리지 않는다.
/// `translations`: 아이템 ID("{file}#{pointer}") → 번역문 (원문과 다른 것만 보내면 된다)
/// `translated_only`: true면 전체 복사 없이 번역이 실제로 적용된 파일만 같은 상대 경로로 dest에 쓴다.
/// `locale_pairs`: 대상 언어 파일은 원본 언어 파일에 번역을 적용해 만들고, 번역이 없어도 항상 쓴다.
#[tauri::command]
pub async fn export_project(
    root: String,
    dest: String,
    translations: HashMap<String, String>,
    translated_only: bool,
    locale_pairs: Vec<LocalePair>,
) -> Result<ExportReport> {
    tauri::async_runtime::spawn_blocking(move || {
        export(Path::new(&root), Path::new(&dest), translations, translated_only, &locale_pairs)
    })
    .await
    .expect("export_project 작업 패닉")
}

fn export(
    root: &Path,
    dest: &Path,
    translations: HashMap<String, String>,
    translated_only: bool,
    locale_pairs: &[LocalePair],
) -> Result<ExportReport> {
    let (root, dest) = prepare_dest(root, dest)?;
    let files_copied = if translated_only { 0 } else { copy_tree(&root, &dest)? };

    // 파일별로 묶어서 한 번씩만 읽고 쓴다
    let mut by_file: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (id, text) in translations {
        if let Some((file, pointer)) = id.split_once('#') {
            by_file.entry(file.to_string()).or_default().push((pointer.to_string(), text));
        }
    }
    // 언어 파일 쌍: 대상 파일 → 원본 언어 파일. 번역이 없어도 누락된 항목을 채우도록 항상 쓴다
    let locale_sources: HashMap<&str, &str> = locale_pairs.iter().map(|p| (p.target.as_str(), p.source.as_str())).collect();
    for target in locale_sources.keys() {
        by_file.entry(target.to_string()).or_default();
    }

    let mut report =
        ExportReport { files_copied, files_patched: 0, strings_applied: 0, images_applied: 0, skipped: Vec::new() };
    for (file, patches) in by_file {
        let in_root = |rel: &str| join_rel(&root, rel);
        let path = join_rel(&dest, &file);
        let locale_source = locale_sources.get(file.as_str());
        // 번역 파일만 내보낼 때는 복사본이 없으므로 원본에서 읽는다
        let src_path = match locale_source {
            Some(source) => in_root(source),
            None if translated_only => in_root(&file),
            None => path.clone(),
        };
        let src = fs::read_to_string(&src_path).map_err(|e| Error::io(&src_path, e))?;
        let result = apply::patch_file(&file, &src, &patches).map_err(|msg| Error::Parse { path: file.clone(), msg })?;
        report.skipped.extend(result.skipped.into_iter().map(|p| format!("{file}#{p}")));
        if result.applied > 0 || locale_source.is_some() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
            }
            fs::write(&path, result.text).map_err(|e| Error::io(&path, e))?;
            report.files_patched += 1;
            report.strings_applied += result.applied;
        }
    }

    // 번역 이미지: 원본 리소스 경로에 (암호화됐다면 같은 방식으로 재암호화해) 덮어쓴다
    for plain in assets::list_replacements(&root) {
        match assets::build_replacement(&root, &plain) {
            Ok(targets) => {
                for (rel, bytes) in targets {
                    let path = join_rel(&dest, &rel);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
                    }
                    fs::write(&path, bytes).map_err(|e| Error::io(&path, e))?;
                    report.images_applied += 1;
                }
            }
            Err(e) => report.skipped.push(e.to_string()),
        }
    }
    Ok(report)
}

/// 내보낼 폴더를 확인하고 만든다. 비어 있어야 하고 원본 폴더 밖이어야 한다.
/// canonicalize한 (원본, 대상) 경로를 돌려준다.
pub(crate) fn prepare_dest(root: &Path, dest: &Path) -> Result<(PathBuf, PathBuf)> {
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
    Ok((root, dest))
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
    use crate::store::{LocalePair, ProjectOptions};

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
        let entries = extract::extract_all(&layout, &ProjectOptions::default()).unwrap().entries;
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
        let report = export(&game, &dest, translations, false, &[]).unwrap();
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
        assert_eq!(extract::extract_all(&layout, &ProjectOptions::default()).unwrap().entries.len(), 1);
        let ex = extract::extract_all(&layout, &ProjectOptions { include_plugins: true, ..Default::default() }).unwrap();
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
        let report = export(&game, &dest, translations, false, &[]).unwrap();
        assert_eq!(report.strings_applied, 3);
        assert!(report.skipped.is_empty());

        let out = fs::read_to_string(dest.join("js/plugins.js")).unwrap();
        assert_eq!(out, plugins.replace("メニュー", "메뉴").replace("回復", "회복"));
        let out_ce = fs::read_to_string(dest.join("data/CommonEvents.json")).unwrap();
        assert!(out_ce.contains(r#"{"text":"어서 오세요","id":"5"}"#));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn detailed_json_roundtrip() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-detailed-{}", std::process::id()));
        let game = base.join("game");
        fs::create_dir_all(game.join("data")).unwrap();
        fs::write(game.join("data/System.json"), r#"{"gameTitle":"勇者"}"#).unwrap();
        let quests = "{\n  \"list\": [{\"title\": \"薬草集め\", \"reward\": 1.50}],\n  \"size\": \"28\"\n}";
        fs::write(game.join("data/Quests.json"), quests).unwrap();

        let layout = detect::detect(&game).unwrap();
        assert_eq!(extract::extract_all(&layout, &ProjectOptions::default()).unwrap().entries.len(), 1);
        let ex = extract::extract_all(&layout, &ProjectOptions { detailed: true, ..Default::default() }).unwrap();
        assert!(ex.warnings.is_empty());
        let originals: Vec<_> = ex.entries.iter().map(|e| e.original.as_str()).collect();
        assert_eq!(originals, ["勇者", "薬草集め", "28"]);

        let id = |o: &str| ex.entries.iter().find(|e| e.original == o).unwrap().id.clone();
        let translations = HashMap::from([(id("薬草集め"), "약초 모으기".to_string()), (id("28"), "24".to_string())]);
        let dest = base.join("out");
        let report = export(&game, &dest, translations, true, &[]).unwrap();
        assert_eq!(report.strings_applied, 2);
        let out = fs::read_to_string(dest.join("data/Quests.json")).unwrap();
        assert_eq!(out, quests.replace("薬草集め", "약초 모으기").replace("\"28\"", "\"24\""));

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn locale_pair_roundtrip() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-locale-{}", std::process::id()));
        let game = base.join("game");
        fs::create_dir_all(game.join("data")).unwrap();
        fs::create_dir_all(game.join("locales")).unwrap();
        fs::write(game.join("data/System.json"), r#"{"gameTitle":"勇者"}"#).unwrap();
        let ja = "{\n  \"menu\": {\"title\": \"メニュー\", \"help\": \"ヘルプ\", \"size\": 28},\n  \"items\": [\"剣\", \"盾\"]\n}";
        fs::write(game.join("locales/ja.json"), ja).unwrap();
        // help는 누락, 盾은 빈 값, old는 원본에 없는 항목
        fs::write(game.join("locales/ko.json"), r#"{"menu":{"title":"메뉴"},"items":["검",""],"old":"옛날"}"#).unwrap();

        let pairs = vec![LocalePair { source: "locales/ja.json".into(), target: "locales/ko.json".into() }];
        let options = ProjectOptions { locale_pairs: pairs.clone(), detailed: true, ..Default::default() };
        let layout = detect::detect(&game).unwrap();
        let ex = extract::extract_all(&layout, &options).unwrap();
        assert_eq!(ex.warnings.len(), 1, "{:?}", ex.warnings);
        assert!(ex.warnings[0].contains("1개"));
        // 세부 수정이 켜져 있어도 쌍에 쓰인 파일은 JSON 데이터로 따로 나오지 않는다
        let got: Vec<_> = ex.entries[1..].iter().map(|e| (e.id.as_str(), e.original.as_str(), e.initial.as_deref())).collect();
        assert_eq!(
            got,
            [
                ("locales/ko.json#/menu/title", "メニュー", Some("메뉴")),
                ("locales/ko.json#/menu/help", "ヘルプ", None),
                ("locales/ko.json#/items/0", "剣", Some("검")),
                ("locales/ko.json#/items/1", "盾", None),
            ]
        );

        // 번역이 하나도 없어도 대상 파일은 원본 언어로 채워서 만든다
        let report = export(&game, &base.join("out0"), HashMap::new(), true, &pairs).unwrap();
        assert_eq!(report.files_patched, 1);
        assert_eq!(fs::read_to_string(base.join("out0/locales/ko.json")).unwrap(), ja);

        let translations = HashMap::from([
            ("locales/ko.json#/menu/title".to_string(), "메뉴".to_string()),
            ("locales/ko.json#/items/0".to_string(), "검".to_string()),
        ]);
        let dest = base.join("out");
        let report = export(&game, &dest, translations, false, &pairs).unwrap();
        assert_eq!(report.strings_applied, 2);
        let out = fs::read_to_string(dest.join("locales/ko.json")).unwrap();
        assert_eq!(out, ja.replace("メニュー", "메뉴").replace("剣", "검"));
        // 원본 언어 파일은 그대로 복사된다
        assert_eq!(fs::read_to_string(dest.join("locales/ja.json")).unwrap(), ja);

        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn export_translated_only() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-only-{}", std::process::id()));
        let game = base.join("game");
        let data = game.join("www/data");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(game.join("www/img")).unwrap();
        fs::write(game.join("www/img/a.png"), "png").unwrap();
        fs::write(data.join("System.json"), r#"{"gameTitle":"勇者"}"#).unwrap();
        let actors = r#"[null,{"id":1,"name":"ハロルド","nickname":"","profile":""}]"#;
        fs::write(data.join("Actors.json"), actors).unwrap();

        let layout = detect::detect(&game).unwrap();
        let entries = extract::extract_all(&layout, &ProjectOptions::default()).unwrap().entries;
        let id = entries.iter().find(|e| e.original == "ハロルド").unwrap().id.clone();
        let dest = base.join("out");
        let report = export(&game, &dest, HashMap::from([(id, "해롤드".to_string())]), true, &[]).unwrap();
        assert_eq!(report.files_copied, 0);
        assert_eq!(report.files_patched, 1);
        assert_eq!(report.strings_applied, 1);

        // 번역된 파일만 같은 상대 경로로 생기고, 나머지는 복사되지 않는다
        let out = fs::read_to_string(dest.join("www/data/Actors.json")).unwrap();
        assert_eq!(out, actors.replace("ハロルド", "해롤드"));
        assert!(!dest.join("www/data/System.json").exists());
        assert!(!dest.join("www/img").exists());
        // 원본은 그대로
        assert_eq!(fs::read_to_string(data.join("Actors.json")).unwrap(), actors);

        fs::remove_dir_all(&base).ok();
    }
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use crate::rpgm::assets::tests::{keys_of, make_game};
    use crate::rpgm::assets::{read_plain, scan, set_replacement, KeyRing};
    use crate::rpgm::detect;

    #[test]
    fn exports_translated_images() {
        let (base, game) = make_game("export-images");
        let new_png: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR translated";
        let src = base.join("t.png");
        fs::write(&src, new_png).unwrap();
        set_replacement(&game, "www/img/pictures/b.png_", &src).unwrap();
        set_replacement(&game, "www/img/pictures/plain.png", &src).unwrap();
        let keys = KeyRing::new(&keys_of(&scan(&detect::detect(&game).unwrap()).unwrap())).unwrap();

        // 번역된 파일만: 번역 이미지만 원래 경로(암호화 확장자 그대로)로 쓰인다
        let only = base.join("only");
        let report = export(&game, &only, HashMap::new(), true, &[]).unwrap();
        assert_eq!((report.images_applied, report.skipped.len()), (2, 0));
        assert!(!only.join("www/img/pictures/a.rpgmvp").exists());
        assert!(!only.join(WORK_DIR).exists());
        let out = only.canonicalize().unwrap();
        assert_eq!(read_plain(&out, "www/img/pictures/b.png_", &keys).unwrap(), new_png);
        assert_eq!(fs::read(only.join("www/img/pictures/plain.png")).unwrap(), new_png);

        // 전체 복사: 복사본을 덮어쓰고, 원본은 그대로
        let full = base.join("full");
        export(&game, &full, HashMap::new(), false, &[]).unwrap();
        let out = full.canonicalize().unwrap();
        assert_eq!(read_plain(&out, "www/img/pictures/b.png_", &keys).unwrap(), new_png);
        assert_eq!(read_plain(&out, "www/img/pictures/a.rpgmvp", &keys).unwrap(), crate::rpgm::assets::tests::PNG);
        assert_eq!(read_plain(&game, "www/img/pictures/b.png_", &keys).unwrap(), crate::rpgm::assets::tests::PNG);
        fs::remove_dir_all(&base).ok();
    }
}
