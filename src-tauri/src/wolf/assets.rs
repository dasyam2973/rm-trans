//! WOLF RPG 리소스 목록 (리소스 탭).
//!
//! UberWolf 등으로 푼 Data 폴더의 평문 파일만 다룬다 (암호화 방식·키가 없다). 결과는 RPG Maker와 같은 `AssetScan`이라
//! 미리보기·추출·번역 이미지 등록과 내보내기(`assets::build_replacement`)를 그대로 쓴다.

use std::fs;

use walkdir::WalkDir;

use crate::error::Result;
use crate::rpgm::assets::{ext_of, list_replacements, rel_path, AssetFile, AssetKind, AssetScan};
use crate::store::WORK_DIR;

use super::{WolfLayout, BASIC_DATA, MAP_DATA};

pub fn scan(layout: &WolfLayout) -> Result<AssetScan> {
    let data = layout.data_dir();
    // 게임 데이터 폴더와 작업 폴더(Data 폴더 자체를 연 경우 그 안에 있다)는 리소스가 아니다
    let skip = |name: &str| [BASIC_DATA, MAP_DATA, WORK_DIR].iter().any(|d| name.eq_ignore_ascii_case(d)) || name.starts_with('.');
    let walker = WalkDir::new(&data).min_depth(1).into_iter().filter_entry(|e| !(e.depth() == 1 && skip(&e.file_name().to_string_lossy())));

    let mut files = Vec::new();
    for entry in walker.filter_map(|e| e.ok()).filter(|e| e.file_type().is_file()) {
        let Some(rel) = rel_path(&layout.root, entry.path()) else { continue };
        let Some(kind) = AssetKind::of(&ext_of(&rel)) else { continue };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        files.push(AssetFile { plain_path: rel.clone(), path: rel, kind, encrypted: false, scheme: None, size });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let mut warnings = Vec::new();
    let packed = packed_archives(layout);
    if !packed.is_empty() {
        warnings.push(format!(
            "풀리지 않은 아카이브가 있어 그 안의 리소스는 목록에 없습니다: {}. UberWolf 등으로 풀어 주세요.",
            packed.join(", ")
        ));
    }

    Ok(AssetScan {
        web_dir: layout.data_rel.clone(),
        system_key: None,
        schemes: Vec::new(),
        crypto_plugins: Vec::new(),
        files,
        replacements: list_replacements(&layout.root),
        warnings,
    })
}

/// Data 폴더의 `X.wolf` 중 옆에 풀린 폴더 `X`가 없는 것
fn packed_archives(layout: &WolfLayout) -> Vec<String> {
    let data = layout.data_dir();
    let Ok(dir) = fs::read_dir(&data) else { return Vec::new() };
    let mut names: Vec<String> = dir
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wolf")))
        .filter(|p| !p.with_extension("").is_dir())
        .filter_map(|p| p.file_name()?.to_str().map(str::to_string))
        // BasicData·MapData는 게임 데이터라 리소스 목록과 관계없다
        .filter(|n| ![BASIC_DATA, MAP_DATA].iter().any(|d| n.eq_ignore_ascii_case(&format!("{d}.wolf"))))
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::super::detect;
    use super::*;

    #[test]
    fn scans_unpacked_resources() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-wolf-assets-{}", std::process::id()));
        let data = base.join("Data");
        for dir in ["BasicData", "MapData", "Picture/ui", "SE", "BGM"] {
            fs::create_dir_all(data.join(dir)).unwrap();
        }
        fs::write(data.join("BasicData/CommonEvent.dat"), [0, 0x57, 0, 0, 0x4F, 0x4C, 0x55, 0x46, 0x43, 0, 0x93]).unwrap();
        fs::write(data.join("BasicData/icon000.png"), "png").unwrap();
        fs::write(data.join("Picture/ui/title.png"), "png").unwrap();
        fs::write(data.join("SE/ok.ogg"), "ogg").unwrap();
        fs::write(data.join("BGM/song.mid"), "mid").unwrap();
        // 풀린 폴더가 있는 아카이브는 경고하지 않고, 없는 것만 알린다
        fs::write(data.join("Picture.wolf"), "x").unwrap();
        fs::write(data.join("CharaChip.wolf"), "x").unwrap();
        fs::write(data.join("MapData.wolf"), "x").unwrap();

        let layout = detect(&base).unwrap().unwrap();
        let s = scan(&layout).unwrap();
        let paths: Vec<_> = s.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["Data/Picture/ui/title.png", "Data/SE/ok.ogg"]);
        assert!(s.files.iter().all(|f| !f.encrypted && f.plain_path == f.path));
        assert_eq!(s.web_dir, "Data");
        assert_eq!(s.warnings.len(), 1);
        assert!(s.warnings[0].contains("CharaChip.wolf") && !s.warnings[0].contains("Picture.wolf"));
        fs::remove_dir_all(&base).ok();
    }
}

#[cfg(test)]
mod real_game {
    use super::super::detect;
    use super::*;

    /// `WOLF_GAME=<경로> cargo test --lib wolf_assets_scan -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn wolf_assets_scan() {
        let root = std::env::var("WOLF_GAME").expect("WOLF_GAME 환경 변수가 필요합니다");
        let s = scan(&detect(std::path::Path::new(&root)).unwrap().unwrap()).unwrap();
        let mut by_kind = std::collections::BTreeMap::new();
        for f in &s.files {
            *by_kind.entry(format!("{:?}", f.kind)).or_insert(0) += 1;
        }
        println!("{} files {by_kind:?}\nwarnings: {:?}", s.files.len(), s.warnings);
        for f in s.files.iter().step_by(1500) {
            println!("  {}", f.path);
        }
    }
}
