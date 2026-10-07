//! WOLF RPG 에디터 게임 지원 (추출, 번역 적용).
//!
//! 압축을 푼 Data 폴더(BasicData, MapData …)를 대상으로 한다. 배포용 .wolf 아카이브는 아직 풀지 못한다.
//! 바이너리 구조는 WolfTL(https://github.com/Sinflower/WolfTL, MIT)의 파서를 참고했다.

pub mod apply;
pub mod assets;
mod command;
mod common;
mod database;
mod extract;
mod game_dat;
mod map;
mod reader;

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub use extract::extract_all;
pub(crate) use extract::has_text;

pub const BASIC_DATA: &str = "BasicData";
pub const MAP_DATA: &str = "MapData";

#[derive(Debug, Clone)]
pub struct WolfLayout {
    pub root: PathBuf,
    /// 루트 기준 Data 폴더 상대 경로 ("Data", 또는 Data 폴더 자체를 열었으면 "")
    pub data_rel: String,
    /// 3.x(UTF-8) 형식인지. 아니면 2.x(Shift-JIS)
    pub utf8: bool,
}

impl WolfLayout {
    pub fn data_dir(&self) -> PathBuf {
        if self.data_rel.is_empty() {
            self.root.clone()
        } else {
            self.root.join(&self.data_rel)
        }
    }

    /// Data 폴더 안의 상대 경로('/' 구분)를 루트 기준으로 변환
    pub fn rel_file(&self, name: &str) -> String {
        if self.data_rel.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.data_rel, name)
        }
    }
}

/// WOLF RPG 게임 폴더(게임 루트 또는 Data 폴더)인지 확인한다.
/// WOLF RPG가 아니면 None, WOLF RPG지만 열 수 없으면 (아카이브만 있음, 암호화) 에러.
pub fn detect(root: &Path) -> Option<Result<WolfLayout>> {
    let found = ["Data", ""].into_iter().find(|rel| root.join(rel).join(BASIC_DATA).is_dir());
    let Some(data_rel) = found else {
        let has_archive = [root.to_path_buf(), root.join("Data")].iter().any(|dir| has_wolf_archive(dir));
        return has_archive.then(|| {
            Err(Error::msg(
                "WOLF RPG 아카이브(.wolf)만 있는 게임입니다. UberWolf 등으로 아카이브를 풀어 Data 폴더(BasicData, MapData)를 만든 뒤 열어 주세요.",
            ))
        });
    };
    let basic = root.join(data_rel).join(BASIC_DATA);

    // 문자 인코딩은 헤더로 판별한다. 읽을 수 있는 파일이 하나도 없으면 그 이유를 알린다
    let mut first_err = None;
    for name in ["Game.dat", "CommonEvent.dat", "DataBase.dat"] {
        let path = basic.join(name);
        let Ok(raw) = fs::read(&path) else { continue };
        match header_utf8(name, &raw) {
            Ok(utf8) => {
                return Some(Ok(WolfLayout { root: root.to_path_buf(), data_rel: data_rel.to_string(), utf8 }));
            }
            Err(e) => {
                first_err.get_or_insert_with(|| Error::Parse { path: format!("{BASIC_DATA}/{name}"), msg: e });
            }
        }
    }
    Some(Err(first_err.unwrap_or_else(|| Error::msg("BasicData 폴더에 WOLF RPG 데이터 파일이 없습니다."))))
}

fn header_utf8(name: &str, raw: &[u8]) -> reader::R<bool> {
    match name {
        "Game.dat" => game_dat::parse(raw).map(|g| g.buf.utf8),
        "CommonEvent.dat" => common::check(raw),
        _ => database::check_dat(raw),
    }
}

fn has_wolf_archive(dir: &Path) -> bool {
    fs::read_dir(dir).map_or(false, |it| {
        it.filter_map(|e| e.ok()).any(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("wolf")))
    })
}

#[cfg(test)]
pub mod tests {
    pub use super::map::tests::sample as sample_map;
}
