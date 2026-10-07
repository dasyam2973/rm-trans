//! 커먼 이벤트 (BasicData/CommonEvent.dat)
//!
//! `[0x00][매직 9바이트][버전: u8]` 뒤에 본문. 버전 0x93/0xCC는 3.5 형식이고 본문이 LZ4로 압축돼 있다.
//! 첫 바이트가 0이 아니면 암호화된 파일이다.

use super::command::{self, Command};
use super::reader::{self, Buffer, Reader, WStr, R};

const MAGIC: [u8; 9] = [0x57, 0, 0, 0x4F, 0x4C, 0, 0x46, 0x43, 0];
const MAGIC_UTF8_AT: usize = 5;
const HEADER: usize = 11;

pub struct CommonEvents {
    pub buf: Buffer,
    pub events: Vec<CommonEvent>,
}

pub struct CommonEvent {
    pub id: u32,
    pub name: WStr,
    pub commands: Vec<Command>,
}

/// 헤더만 확인하고 UTF-8 여부를 돌려준다
pub fn check(raw: &[u8]) -> R<bool> {
    check_header(raw, &MAGIC, MAGIC_UTF8_AT, "커먼 이벤트 파일")
}

pub fn parse(raw: &[u8]) -> R<CommonEvents> {
    let utf8 = check(raw)?;
    let version = raw[HEADER - 1];
    let v35 = matches!(version, 0x93 | 0xCC);
    let buf = Buffer::load(raw, utf8, v35.then_some(HEADER))?;

    let mut r = Reader::new(&buf.data, HEADER, utf8);
    let count = r.count(1)?;
    let mut events = Vec::with_capacity(count);
    for i in 0..count {
        events.push(read_event(&mut r, v35).map_err(|e| format!("커먼 이벤트 #{i}: {e}"))?);
    }
    let end = r.u8()?;
    if end < 0x89 {
        return Err(format!("커먼 이벤트 종료 표시가 잘못됐습니다 ({end:#04x})"));
    }
    if !r.is_eof() {
        return Err("커먼 이벤트 끝 뒤에 데이터가 더 있습니다".into());
    }
    Ok(CommonEvents { buf, events })
}

/// `[0x00][매직]` 헤더를 확인하고 UTF-8 여부를 돌려준다 (CommonEvent.dat, DB .dat, Game.dat 공통)
pub(super) fn check_header(raw: &[u8], magic: &[u8], utf8_at: usize, what: &str) -> R<bool> {
    match raw.first() {
        None => Err(format!("{what}이(가) 비어 있습니다")),
        Some(0) => reader::match_magic(&raw[1..], magic, utf8_at)
            .ok_or_else(|| format!("{what} 형식이 아닙니다 (지원하지 않는 버전)")),
        Some(_) => Err(format!("암호화된 {what}입니다. UberWolf 등으로 먼저 복호화해 주세요")),
    }
}

fn read_event(r: &mut Reader, v35: bool) -> R<CommonEvent> {
    r.expect_u8(0x8E, "커먼 이벤트 시작 표시")?;
    let id = r.u32()?;
    r.u32()?;
    r.skip(7)?;
    let name = r.string()?;
    let commands = command::read_list(r, v35)?;
    r.string()?;
    r.string()?; // 설명 (에디터 전용)
    r.expect_u8(0x8F, "커먼 이벤트 데이터 표시")?;
    // 이하 인자 이름, 인자 설정, 셀프 변수 이름 등 에디터용 정보
    for _ in 0..r.count(4)? {
        r.string()?;
    }
    let n = r.count(1)?;
    r.skip(n)?;
    for _ in 0..r.count(4)? {
        for _ in 0..r.count(4)? {
            r.string()?;
        }
    }
    for _ in 0..r.count(4)? {
        let n = r.count(4)?;
        r.skip(n * 4)?;
    }
    r.skip(0x1D)?;
    for _ in 0..100 {
        r.string()?;
    }
    r.expect_u8(0x91, "커먼 이벤트 데이터 표시")?;
    r.string()?;
    match r.u8()? {
        0x91 => {}
        0x92 => {
            r.string()?;
            r.u32()?;
            r.expect_u8(0x92, "커먼 이벤트 종료 표시")?;
        }
        b => return Err(format!("알 수 없는 표시 {b:#04x} (오프셋 {:#x})", r.pos() - 1)),
    }
    Ok(CommonEvent { id, name, commands })
}
