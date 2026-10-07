use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter};

use crate::engine::{self, Layout};
use crate::error::{Error, Result};
use crate::rpgm::assets::{self, AssetScan, KeyRing, SchemeKey};
use crate::rpgm::crypto::Scheme;
use crate::wolf;

use super::export::prepare_dest;

const EVENT_PROGRESS: &str = "assets://progress";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetExportReport {
    /// 쓴 파일 수 (복호화한 것 포함)
    pub written: usize,
    pub decrypted: usize,
    /// 실패한 파일과 이유
    pub failed: Vec<String>,
}

#[derive(Clone, Serialize)]
struct Progress {
    done: usize,
    total: usize,
}

#[tauri::command]
pub async fn scan_assets(root: String) -> Result<AssetScan> {
    tauri::async_runtime::spawn_blocking(move || match engine::detect(&PathBuf::from(root))? {
        Layout::Rpgm(l) => assets::scan(&l),
        Layout::Wolf(l) => wolf::assets::scan(&l),
    })
        .await
        .expect("scan_assets 작업 패닉")
}

/// System.json 형식의 키 문자열을 방식별 실제 키(hex)로 바꾼다 (사용자 입력용)
#[tauri::command]
pub fn derive_asset_key(scheme: Scheme, source: String) -> Result<String> {
    Ok(scheme.derive_key(source.trim())?.to_hex())
}

/// 미리보기용: 복호화한 파일 내용을 그대로 보낸다
#[tauri::command]
pub async fn read_asset(root: String, path: String, keys: Vec<SchemeKey>) -> Result<Response> {
    tauri::async_runtime::spawn_blocking(move || {
        let keys = KeyRing::new(&keys)?;
        assets::read_plain(Path::new(&root), &path, &keys).map(Response::new)
    })
    .await
    .expect("read_asset 작업 패닉")
}

/// 리소스 하나를 복호화해 dest 파일로 저장한다 (번역 이미지 편집용)
#[tauri::command]
pub async fn save_asset(root: String, path: String, keys: Vec<SchemeKey>, dest: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let keys = KeyRing::new(&keys)?;
        let bytes = assets::read_plain(Path::new(&root), &path, &keys)?;
        fs::write(&dest, bytes).map_err(|e| Error::io(&dest, e))
    })
    .await
    .expect("save_asset 작업 패닉")
}

/// source PNG를 리소스(path)의 번역 이미지로 등록한다 (.rmtrans/assets에 복사). 등록된 plain_path를 돌려준다
#[tauri::command]
pub async fn set_asset_replacement(root: String, path: String, source: String) -> Result<String> {
    tauri::async_runtime::spawn_blocking(move || assets::set_replacement(Path::new(&root), &path, Path::new(&source)))
        .await
        .expect("set_asset_replacement 작업 패닉")
}

#[tauri::command]
pub fn remove_asset_replacement(root: String, path: String) -> Result<()> {
    assets::remove_replacement(Path::new(&root), &path)
}

#[tauri::command]
pub async fn read_asset_replacement(root: String, path: String) -> Result<Response> {
    tauri::async_runtime::spawn_blocking(move || assets::read_replacement(Path::new(&root), &path).map(Response::new))
        .await
        .expect("read_asset_replacement 작업 패닉")
}

/// 리소스를 복호화해 dest에 같은 상대 경로(원래 확장자)로 쓴다. 원본 폴더는 건드리지 않는다.
/// 진행률은 `assets://progress` 이벤트로 전달된다.
#[tauri::command]
pub async fn export_assets(
    app: AppHandle,
    root: String,
    dest: String,
    paths: Vec<String>,
    keys: Vec<SchemeKey>,
) -> Result<AssetExportReport> {
    tauri::async_runtime::spawn_blocking(move || {
        let keys = KeyRing::new(&keys)?;
        export(Path::new(&root), Path::new(&dest), &paths, &keys, |done, total| {
            // 파일이 많을 수 있어 일정 간격으로만 알린다
            if done % 20 == 0 || done == total {
                let _ = app.emit(EVENT_PROGRESS, Progress { done, total });
            }
        })
    })
    .await
    .expect("export_assets 작업 패닉")
}

fn export(
    root: &Path,
    dest: &Path,
    paths: &[String],
    keys: &KeyRing,
    mut on_progress: impl FnMut(usize, usize),
) -> Result<AssetExportReport> {
    let (root, dest) = prepare_dest(root, dest)?;
    let mut report = AssetExportReport { written: 0, decrypted: 0, failed: Vec::new() };
    for (i, rel) in paths.iter().enumerate() {
        let out_rel = assets::plain_path(rel);
        let result = assets::read_plain(&root, rel, keys).and_then(|bytes| {
            let out = assets::safe_join(&dest, &out_rel)?;
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
            }
            fs::write(&out, bytes).map_err(|e| Error::io(&out, e))
        });
        match result {
            Ok(()) => {
                report.written += 1;
                if out_rel != *rel {
                    report.decrypted += 1;
                }
            }
            Err(e) => report.failed.push(e.to_string()),
        }
        on_progress(i + 1, paths.len());
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpgm::assets::tests::{keys_of, make_game, PNG};
    use crate::rpgm::detect;

    #[test]
    fn exports_decrypted_copies() {
        let (base, game) = make_game("export");
        let scan = assets::scan(&detect::detect(&game).unwrap()).unwrap();
        let keys = KeyRing::new(&keys_of(&scan)).unwrap();
        let paths: Vec<String> = scan.files.iter().map(|f| f.path.clone()).collect();
        let dest = base.join("out");

        let mut last = (0, 0);
        let report = export(&game, &dest, &paths, &keys, |d, t| last = (d, t)).unwrap();
        assert_eq!(last, (5, 5));
        assert_eq!((report.written, report.decrypted, report.failed.len()), (4, 3, 1)); // odd.rpgmvp 실패
        assert_eq!(fs::read(dest.join("www/img/pictures/a.png")).unwrap(), PNG);
        assert_eq!(fs::read(dest.join("www/img/pictures/b.png")).unwrap(), PNG);
        assert_eq!(fs::read(dest.join("www/img/pictures/plain.png")).unwrap(), PNG);
        assert_eq!(fs::read(dest.join("www/audio/bgm/song.ogg")).unwrap(), b"OggS audio");
        assert!(!dest.join("www/img/pictures/a.rpgmvp").exists());

        // 원본 폴더 안이나 비어 있지 않은 폴더에는 쓰지 않는다
        assert!(export(&game, &game.join("out"), &paths, &keys, |_, _| {}).is_err());
        assert!(export(&game, &dest, &paths, &keys, |_, _| {}).is_err());
        fs::remove_dir_all(&base).ok();
    }
}
