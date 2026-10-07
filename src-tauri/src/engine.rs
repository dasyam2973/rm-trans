//! 게임 엔진 판별과 엔진별 처리 분기.
//!
//! 엔진마다 폴더 구조·데이터 형식·추출 규칙이 다르므로 열기/추출은 여기서 엔진별 모듈로 나눈다.
//! 아이템 ID("{file}#{path}"), 저장 파일, AI 번역 등 그 위의 계층은 엔진과 무관하다.

use std::path::Path;

use crate::error::{Error, Result};
use crate::model::{Engine, Extracted};
use crate::rpgm::detect::GameLayout;
use crate::store::ProjectOptions;
use crate::wolf::WolfLayout;
use crate::{rpgm, textfile, wolf};

pub enum Layout {
    Rpgm(GameLayout),
    Wolf(WolfLayout),
}

impl Layout {
    pub fn engine(&self) -> Engine {
        match self {
            Layout::Rpgm(l) => l.engine,
            Layout::Wolf(l) if l.utf8 => Engine::Wolf3,
            Layout::Wolf(_) => Engine::Wolf2,
        }
    }

    pub fn root(&self) -> &Path {
        match self {
            Layout::Rpgm(l) => &l.root,
            Layout::Wolf(l) => &l.root,
        }
    }

    /// 루트 기준 데이터 폴더 상대 경로 (파일 목록 표시용)
    pub fn data_rel(&self) -> &str {
        match self {
            Layout::Rpgm(l) => &l.data_rel,
            Layout::Wolf(l) => &l.data_rel,
        }
    }
}

/// 선택한 폴더의 엔진과 데이터 위치를 찾는다.
pub fn detect(root: &Path) -> Result<Layout> {
    if let Ok(layout) = rpgm::detect::detect(root) {
        return Ok(Layout::Rpgm(layout));
    }
    match wolf::detect(root) {
        Some(result) => result.map(Layout::Wolf),
        None => Err(Error::msg(
            "게임 데이터를 찾을 수 없습니다. RPG Maker MV/MZ(System.json) 또는 WOLF RPG(Data/BasicData) 게임 폴더를 선택해 주세요.",
        )),
    }
}

/// `options`의 플러그인/세부 수정/언어 파일 설정은 RPG Maker에만 적용된다.
/// 텍스트 파일 규칙은 엔진과 무관하게 엔진 데이터 뒤에 추출한다.
pub fn extract_all(layout: &Layout, options: &ProjectOptions) -> Result<Extracted> {
    let mut ex = match layout {
        Layout::Rpgm(l) => rpgm::extract::extract_all(l, options)?,
        Layout::Wolf(l) => wolf::extract_all(l)?,
    };
    textfile::extract_all(layout.root(), &options.text_rules, &mut ex.entries, &mut ex.warnings);
    Ok(ex)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn detects_engines() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-engine-{}", std::process::id()));
        let rm = base.join("rm");
        fs::create_dir_all(rm.join("data")).unwrap();
        fs::write(rm.join("data/System.json"), "{}").unwrap();
        assert!(matches!(detect(&rm), Ok(Layout::Rpgm(_))));

        // 3.x 커먼 이벤트 헤더만 있어도 WOLF RPG로 판별
        let wolf = base.join("wolf");
        fs::create_dir_all(wolf.join("Data/BasicData")).unwrap();
        fs::write(wolf.join("Data/BasicData/CommonEvent.dat"), [0, 0x57, 0, 0, 0x4F, 0x4C, 0x55, 0x46, 0x43, 0, 0x93]).unwrap();
        let layout = detect(&wolf).unwrap();
        assert_eq!(layout.engine(), Engine::Wolf3);
        assert_eq!(layout.data_rel(), "Data");
        // Data 폴더를 직접 고른 경우
        assert_eq!(detect(&wolf.join("Data")).unwrap().data_rel(), "");

        // 암호화된 파일만 있으면 이유를 알린다
        let enc = base.join("enc");
        fs::create_dir_all(enc.join("Data/BasicData")).unwrap();
        fs::write(enc.join("Data/BasicData/CommonEvent.dat"), [0x2A, 1, 2, 3]).unwrap();
        assert!(detect(&enc).err().unwrap().to_string().contains("암호화"));

        // 아카이브만 있는 배포본
        let packed = base.join("packed");
        fs::create_dir_all(&packed).unwrap();
        fs::write(packed.join("Data.wolf"), "x").unwrap();
        assert!(detect(&packed).err().unwrap().to_string().contains(".wolf"));

        assert!(detect(&base).is_err());
        fs::remove_dir_all(&base).ok();
    }
}
