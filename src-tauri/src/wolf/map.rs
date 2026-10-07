//! 맵 파일 (MapData/*.mps)
//!
//! `[매직 20바이트][버전: u32][? : u8]` 뒤에 본문이 온다. 버전 0x65 이상이면 본문이 LZ4로 압축돼 있고,
//! 0x67 이상이면 커맨드가 3.5 형식이다.
//! 본문: 이름(?), 타일셋, 크기, 이벤트 수, (0x67+: ?, 레이어 수), 타일 데이터, 이벤트들(0x6F로 시작), 끝(0x66)

use super::command::{self, Command};
use super::reader::{self, Buffer, Reader, WStr, R};

const MAGIC: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x57, 0x4F, 0x4C, 0x46, 0x4D, 0, 0, 0, 0, 0];
const MAGIC_UTF8_AT: usize = 16;
const HEADER: usize = 25;
const EVENT: u8 = 0x6F;
const PAGE: u8 = 0x79;
const END: u8 = 0x66;

pub struct Map {
    pub buf: Buffer,
    pub events: Vec<Event>,
}

pub struct Event {
    pub id: u32,
    pub name: WStr,
    pub pages: Vec<Vec<Command>>,
}

pub fn parse(raw: &[u8]) -> R<Map> {
    let utf8 = reader::match_magic(raw, &MAGIC, MAGIC_UTF8_AT)
        .ok_or("맵 파일 형식이 아닙니다 (암호화됐거나 지원하지 않는 버전)")?;
    let version = Reader::new(raw, MAGIC.len(), utf8).u32()?;
    let buf = Buffer::load(raw, utf8, (version >= 0x65).then_some(HEADER))?;
    let events = parse_body(&buf.data, utf8, version)?;
    Ok(Map { buf, events })
}

fn parse_body(data: &[u8], utf8: bool, version: u32) -> R<Vec<Event>> {
    let v35 = version >= 0x67;
    let mut r = Reader::new(data, HEADER, utf8);
    r.string()?;
    r.u32()?; // 타일셋
    let width = r.u32()? as usize;
    let height = r.u32()? as usize;
    let event_count = r.u32()? as usize;
    let mut layers = 3;
    if v35 {
        r.u32()?;
        layers = r.u32()? as usize;
    }

    let mut events = Vec::new();
    if r.pos() + 1 == data.len() {
        r.expect_u8(END, "맵 종료 표시")?;
        return Ok(events);
    }
    // UTF-8 형식은 타일 데이터 자리에 -1이 있으면 타일이 없다
    if utf8 && r.peek_u32()? == u32::MAX {
        r.u32()?;
    } else {
        r.skip(width * height * layers * 4)?;
    }
    loop {
        match r.u8()? {
            EVENT => {
                let i = events.len();
                events.push(read_event(&mut r, v35).map_err(|e| format!("이벤트 #{i}: {e}"))?);
            }
            END => break,
            b => return Err(format!("알 수 없는 표시 {b:#04x} (오프셋 {:#x})", r.pos() - 1)),
        }
    }
    if events.len() != event_count {
        return Err(format!("이벤트 수가 다릅니다 ({} / {event_count})", events.len()));
    }
    if !r.is_eof() {
        return Err("맵 끝 뒤에 데이터가 더 있습니다".into());
    }
    Ok(events)
}

fn read_event(r: &mut Reader, v35: bool) -> R<Event> {
    r.expect(&[0x39, 0x30, 0, 0], "이벤트 시작 표시")?;
    let id = r.u32()?;
    let name = r.string()?;
    r.skip(8)?; // x, y
    let page_count = r.u32()? as usize;
    r.expect(&[0, 0, 0, 0], "이벤트 헤더")?;
    let mut pages = Vec::new();
    loop {
        match r.u8()? {
            PAGE => {
                let p = pages.len();
                pages.push(read_page(r, v35).map_err(|e| format!("페이지 {}: {e}", p + 1))?);
            }
            0x70 => break,
            b => return Err(format!("알 수 없는 표시 {b:#04x} (오프셋 {:#x})", r.pos() - 1)),
        }
    }
    if pages.len() != page_count {
        return Err(format!("페이지 수가 다릅니다 ({} / {page_count})", pages.len()));
    }
    Ok(Event { id, name, pages })
}

fn read_page(r: &mut Reader, v35: bool) -> R<Vec<Command>> {
    r.u32()?;
    r.string()?; // 그래픽 파일 이름
    r.skip(4)?; // 방향, 프레임, 불투명도, 표시 방식
    r.skip(1 + 4 + 4 * 4 + 4 * 4)?; // 실행 조건
    r.skip(4)?; // 이동 설정
    r.skip(2)?; // 플래그, 루트 플래그
    command::read_route(r)?;
    let commands = command::read_list(r, v35)?;
    let features = r.u32()?;
    r.skip(3)?; // 그림자, 접촉 범위
    if features > 3 {
        r.u8()?;
    }
    r.expect_u8(0x7A, "페이지 종료 표시")?;
    Ok(commands)
}

#[cfg(test)]
pub mod tests {
    use super::super::command::tests::write as write_cmd;
    use super::super::reader::tests::Writer;
    use super::*;

    /// 이벤트 하나, 페이지 하나짜리 3.5 형식 맵 (압축)
    pub fn sample(messages: &[&str]) -> Vec<u8> {
        let mut b = Writer::default();
        b.str("").u32(0).u32(1).u32(1).u32(1).u32(0).u32(1);
        b.u32(u32::MAX);
        b.u8(EVENT).raw(&[0x39, 0x30, 0, 0]).u32(3).str("村人").u32(0).u32(0).u32(1).u32(0);
        b.u8(PAGE).u32(0).str("").raw(&[0; 4]).raw(&[0; 37]).raw(&[0; 4]).raw(&[0; 2]).u32(0);
        b.u32(messages.len() as u32);
        for m in messages {
            write_cmd(&mut b, command::code::MESSAGE, &[], &[m], true);
        }
        b.u32(0).raw(&[0; 3]).u8(0x7A);
        b.u8(0x70).u8(END);

        let packed = lz4_flex::block::compress(&b.0);
        let mut w = Writer::default();
        w.raw(&MAGIC);
        w.0[MAGIC_UTF8_AT] = 0x55;
        w.u32(0x67).u8(0).u32(b.0.len() as u32).u32(packed.len() as u32).raw(&packed);
        w.0
    }

    #[test]
    fn parses_compressed_map() {
        let map = parse(&sample(&["やあ", "またね"])).unwrap();
        assert!(map.buf.utf8);
        assert_eq!(map.events.len(), 1);
        assert_eq!((map.events[0].id, map.events[0].name.text.as_str()), (3, "村人"));
        let texts: Vec<_> = map.events[0].pages[0].iter().map(|c| c.strings[0].text.as_str()).collect();
        assert_eq!(texts, ["やあ", "またね"]);
        let span = map.events[0].pages[0][1].strings[0].span.clone();
        assert_eq!(&map.buf.data[span.start + 4..span.end - 1], "またね".as_bytes());
    }
}
