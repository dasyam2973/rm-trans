//! 이벤트 커맨드 (맵 이벤트 페이지, 커먼 이벤트 공통)
//!
//! 커맨드 하나의 구조:
//! `[정수 인자 수 + 1: u8][코드: u32][정수 인자…: u32][들여쓰기: u8][문자열 인자 수: u8][문자열…][종료: u8]`
//! 종료 바이트가 0x01이면 이동 루트가 이어진다. 3.5 이후 형식은 끝에 `[n: u8][n바이트]`가 더 붙는다.

use super::reader::{Reader, WStr, R};

#[derive(Debug)]
pub struct Command {
    pub code: u32,
    pub ints: Vec<u32>,
    pub strings: Vec<WStr>,
}

pub mod code {
    pub const MESSAGE: u32 = 101;
    pub const CHOICES: u32 = 102;
    pub const STRING_CONDITION: u32 = 112;
    pub const SET_STRING: u32 = 122;
    pub const PICTURE: u32 = 150;
    pub const DATABASE: u32 = 250;
    pub const COMMON_EVENT_BY_NAME: u32 = 300;
}

/// `v35`: 3.5 이후 형식 (커맨드 끝에 추가 바이트가 있음)
pub fn read_list(r: &mut Reader, v35: bool) -> R<Vec<Command>> {
    let count = r.count(8)?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        out.push(read(r, v35).map_err(|e| format!("커맨드 #{i}: {e}"))?);
    }
    Ok(out)
}

fn read(r: &mut Reader, v35: bool) -> R<Command> {
    let int_count = r.u8()?.wrapping_sub(1) as usize;
    let code = r.u32()?;
    let ints = (0..int_count).map(|_| r.u32()).collect::<R<Vec<_>>>()?;
    let _indent = r.u8()?;
    let str_count = r.u8()? as usize;
    let strings = (0..str_count).map(|_| r.string()).collect::<R<Vec<_>>>()?;
    match r.u8()? {
        0x00 => {}
        0x01 => {
            // 이동 루트: 알 수 없는 5바이트, 플래그, 루트 커맨드 목록
            r.skip(6)?;
            read_route(r)?;
        }
        t => return Err(format!("알 수 없는 커맨드 종료 바이트 {t:#04x} (코드 {code})")),
    }
    if v35 {
        let n = r.u8()? as usize;
        r.skip(n)?;
    }
    Ok(Command { code, ints, strings })
}

/// 이동 루트 커맨드 목록: `[개수: u32]` 뒤에 `[id: u8][인자 수: u8][인자…: u32][01 00]`
pub fn read_route(r: &mut Reader) -> R<()> {
    let count = r.count(4)?;
    for _ in 0..count {
        r.u8()?;
        let args = r.u8()? as usize;
        r.skip(args * 4)?;
        r.expect(&[0x01, 0x00], "이동 루트 종료 표시")?;
    }
    Ok(())
}

impl Command {
    /// 그림 표시 커맨드가 문자열을 표시하는지 (파일 이름이 아니라)
    pub fn is_picture_text(&self) -> bool {
        self.code == code::PICTURE && self.ints.first().is_some_and(|a| (a >> 4) & 0x07 == 2)
    }
}

#[cfg(test)]
pub mod tests {
    use super::super::reader::tests::Writer;
    use super::*;

    /// 테스트용 커맨드 쓰기
    pub fn write(w: &mut Writer, code: u32, ints: &[u32], strings: &[&str], v35: bool) {
        w.u8(ints.len() as u8 + 1).u32(code);
        for &i in ints {
            w.u32(i);
        }
        w.u8(0).u8(strings.len() as u8);
        for s in strings {
            w.str(s);
        }
        w.u8(0);
        if v35 {
            w.u8(0);
        }
    }

    #[test]
    fn reads_commands_and_routes() {
        let mut w = Writer::default();
        w.u32(3);
        write(&mut w, code::MESSAGE, &[], &["こんにちは"], true);
        // 이동 커맨드 (루트 2개)
        w.u8(1).u32(201).u8(0).u8(0).u8(1);
        w.raw(&[0; 5]).u8(0).u32(2);
        w.u8(1).u8(1).u32(5).raw(&[1, 0]);
        w.u8(2).u8(0).raw(&[1, 0]);
        w.u8(2).raw(&[9, 9]);
        write(&mut w, code::PICTURE, &[0x20, 1], &["文字"], true);

        let mut r = Reader::new(&w.0, 0, true);
        let list = read_list(&mut r, true).unwrap();
        assert!(r.is_eof());
        assert_eq!(list.iter().map(|c| c.code).collect::<Vec<_>>(), [101, 201, 150]);
        assert_eq!(list[0].strings[0].text, "こんにちは");
        assert!(list[2].is_picture_text());
    }
}
