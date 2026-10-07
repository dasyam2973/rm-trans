//! 게임 기본 설정 (BasicData/Game.dat)
//!
//! `[0x00][매직 9바이트]` 뒤에 바로 본문 (압축 없음). 문자열 목록 뒤에
//! `[파일 크기 - 1: u32][?: u32][n: u32][n×2바이트][오프셋1: u32][오프셋2: u32]…`가 온다.
//! 문자열 길이가 바뀌면 파일 크기와 두 오프셋을 그만큼 고쳐야 한다.

use super::common::check_header;
use super::reader::{Buffer, Reader, WStr, R};

const MAGIC: [u8; 9] = [0x57, 0, 0, 0x4F, 0x4C, 0, 0x46, 0x4D, 0];
const MAGIC_UTF8_AT: usize = 8;
const HEADER: usize = 10;

pub struct GameDat {
    pub buf: Buffer,
    /// (경로 이름, 표시 이름, 문자열)
    pub strings: Vec<(&'static str, &'static str, WStr)>,
    /// 파일 크기 필드 위치
    pub size_at: usize,
    /// 두 오프셋 필드 위치
    pub offsets_at: usize,
}

pub fn parse(raw: &[u8]) -> R<GameDat> {
    let utf8 = check_header(raw, &MAGIC, MAGIC_UTF8_AT, "Game.dat")?;
    let mut r = Reader::new(raw, HEADER, utf8);
    let n = r.count(1)?;
    r.skip(n)?;
    let count = r.u32()?;
    let mut strings = vec![("title", "게임 제목", r.string()?)];
    if r.string()?.text != "0000-0000" {
        return Err("Game.dat 형식이 아닙니다".into());
    }
    let n = r.count(1)?;
    r.skip(n)?; // 암호 키
    for _ in 0..5 {
        r.string()?; // 폰트 4개, 기본 주인공 그래픽
    }
    if count >= 9 {
        strings.push(("titlePlus", "타이틀 추가 문구", r.string()?));
    }
    if count > 9 {
        r.string()?; // 로딩 이미지
        r.string()?; // 게이지 이미지
        strings.push(("startUpMsg", "시작 메시지", r.string()?));
        strings.push(("titleMsg", "타이틀 메시지", r.string()?));
    }
    if count > 13 {
        r.string()?;
    }
    let size_at = r.pos();
    r.skip(8)?;
    let words = r.count(2)?;
    r.skip(words * 2)?;
    let offsets_at = r.pos();
    r.skip(8)?;
    Ok(GameDat { buf: Buffer { data: raw.to_vec(), utf8, packed_at: None }, strings, size_at, offsets_at })
}

impl GameDat {
    /// 크기 필드가 실제 파일 크기와 맞는지. 맞지 않으면 구조를 잘못 읽은 것이므로 쓰지 않는다
    pub fn check_size(&self) -> R<()> {
        let stored = read_u32(&self.buf.data, self.size_at);
        if stored as usize + 1 != self.buf.data.len() {
            return Err(format!("Game.dat 크기 필드({stored})가 파일 크기와 맞지 않아 수정할 수 없습니다"));
        }
        Ok(())
    }

    /// 문자열을 교체한 데이터(`data`)의 크기·오프셋 필드를 고친다.
    /// 교체한 문자열은 모두 크기 필드보다 앞에 있으므로 필드 위치도 길이 차이만큼 밀려 있다
    pub fn fix_sizes(&self, data: &mut [u8]) {
        let diff = data.len() as i64 - self.buf.data.len() as i64;
        for at in [self.size_at, self.offsets_at, self.offsets_at + 4] {
            let at = (at as i64 + diff) as usize;
            let value = (read_u32(data, at) as i64 + diff) as u32;
            data[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
}

fn read_u32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}
