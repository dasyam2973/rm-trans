use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::model::Engine;

#[derive(Debug, Clone)]
pub struct GameLayout {
    pub root: PathBuf,
    /// 루트 기준 data 폴더 상대 경로 ("www/data", "data", 또는 루트 자체면 "")
    pub data_rel: String,
    pub engine: Engine,
}

impl GameLayout {
    pub fn data_dir(&self) -> PathBuf {
        if self.data_rel.is_empty() {
            self.root.clone()
        } else {
            self.root.join(&self.data_rel)
        }
    }

    /// data 폴더 안의 파일명을 루트 기준 상대 경로로 변환
    pub fn rel_file(&self, name: &str) -> String {
        if self.data_rel.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.data_rel, name)
        }
    }
}

/// 선택한 폴더에서 RPG Maker data 폴더 위치와 엔진 종류를 찾는다.
/// 배포본(www/data), 에디터 프로젝트(data), data 폴더 자체를 고른 경우를 모두 허용한다.
pub fn detect(root: &Path) -> Result<GameLayout> {
    let data_rel = ["www/data", "data", ""]
        .into_iter()
        .find(|rel| root.join(rel).join("System.json").is_file())
        .ok_or_else(|| Error::msg("RPG Maker 데이터 폴더(System.json)를 찾을 수 없습니다."))?;

    let has = |p: &str| root.join(p).is_file();
    let engine = if has("js/rmmz_core.js") || has("www/js/rmmz_core.js") {
        Engine::Mz
    } else if has("js/rpg_core.js") || has("www/js/rpg_core.js") {
        Engine::Mv
    } else {
        Engine::Unknown
    };

    Ok(GameLayout { root: root.to_path_buf(), data_rel: data_rel.to_string(), engine })
}
