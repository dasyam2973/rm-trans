//! WOLF RPG 바이너리 데이터 읽기.
//!
//! 모든 파일이 리틀 엔디언 u32와 길이 접두 문자열(`[u32 길이][바이트…][NUL]`)로 이루어져 있다.
//! 3.x(UTF-8)는 본문을 LZ4 블록으로 압축하고, 2.x는 Shift-JIS 문자열을 쓴다.
//! 문자열마다 (압축을 푼) 버퍼에서의 바이트 범위를 기록해 두어 나중에 그 구간만 교체할 수 있게 한다.

use std::ops::Range;

pub type R<T> = std::result::Result<T, String>;

/// 파일 안의 문자열 하나
#[derive(Debug, Clone)]
pub struct WStr {
    pub text: String,
    /// 길이 접두부터 NUL까지를 포함한 바이트 범위
    pub span: Range<usize>,
}

/// 압축을 푼 파일 내용. 문자열 범위는 이 버퍼 기준이다.
pub struct Buffer {
    pub data: Vec<u8>,
    pub utf8: bool,
    /// 본문이 LZ4로 압축돼 있었으면 본문 시작 위치 (= 헤더 길이)
    pub packed_at: Option<usize>,
}

impl Buffer {
    pub fn load(raw: &[u8], utf8: bool, packed_at: Option<usize>) -> R<Buffer> {
        let data = match packed_at {
            Some(at) => unpack(raw, at)?,
            None => raw.to_vec(),
        };
        Ok(Buffer { data, utf8, packed_at })
    }

    /// 문자열을 파일의 인코딩으로 `[u32 길이][바이트…][NUL]` 형태로 만든다.
    /// Shift-JIS로 나타낼 수 없는 문자(한글 등)가 있으면 None
    pub fn encode(&self, text: &str) -> Option<Vec<u8>> {
        let bytes = if self.utf8 {
            std::borrow::Cow::Borrowed(text.as_bytes())
        } else {
            let (bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode(text);
            if had_errors {
                return None;
            }
            bytes
        };
        let mut out = Vec::with_capacity(bytes.len() + 5);
        out.extend_from_slice(&(bytes.len() as u32 + 1).to_le_bytes());
        out.extend_from_slice(&bytes);
        out.push(0);
        Some(out)
    }

    /// 압축을 푼 데이터를 원래 파일 형태로 되돌린다 (압축돼 있었으면 다시 압축)
    pub fn pack(&self, data: &[u8]) -> Vec<u8> {
        let Some(at) = self.packed_at else { return data.to_vec() };
        let body = &data[at..];
        let packed = lz4_flex::block::compress(body);
        let mut out = Vec::with_capacity(at + 8 + packed.len());
        out.extend_from_slice(&data[..at]);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        out.extend_from_slice(&packed);
        out
    }
}

/// 겹치지 않는 범위들을 교체한 새 데이터를 만든다
pub fn splice(data: &[u8], mut replacements: Vec<(Range<usize>, Vec<u8>)>) -> Vec<u8> {
    replacements.sort_by_key(|(r, _)| r.start);
    let mut out = Vec::with_capacity(data.len());
    let mut pos = 0;
    for (range, bytes) in replacements {
        out.extend_from_slice(&data[pos..range.start]);
        out.extend_from_slice(&bytes);
        pos = range.end;
    }
    out.extend_from_slice(&data[pos..]);
    out
}

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
    utf8: bool,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8], pos: usize, utf8: bool) -> Self {
        Reader { buf, pos, utf8 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.buf.len()
    }

    pub fn bytes(&mut self, n: usize) -> R<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len()).ok_or_else(|| {
            format!("파일이 예상보다 짧습니다 (오프셋 {:#x}에서 {n}바이트 필요)", self.pos)
        })?;
        let out = &self.buf[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    pub fn skip(&mut self, n: usize) -> R<()> {
        self.bytes(n).map(|_| ())
    }

    pub fn u8(&mut self) -> R<u8> {
        Ok(self.bytes(1)?[0])
    }

    pub fn u32(&mut self) -> R<u32> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// 다음 u32를 읽지 않고 본다
    pub fn peek_u32(&self) -> R<u32> {
        Reader { buf: self.buf, pos: self.pos, utf8: self.utf8 }.u32()
    }

    /// 개수(u32)가 앞에 붙은 목록의 개수. 남은 바이트보다 많으면 깨진 데이터로 본다
    pub fn count(&mut self, min_item_size: usize) -> R<usize> {
        let at = self.pos;
        let n = self.u32()? as usize;
        if n.saturating_mul(min_item_size) > self.buf.len() - self.pos {
            return Err(format!("잘못된 개수 {n} (오프셋 {at:#x})"));
        }
        Ok(n)
    }

    pub fn string(&mut self) -> R<WStr> {
        let start = self.pos;
        let len = self.u32()? as usize;
        let raw = self.bytes(len)?;
        let raw = raw.strip_suffix(&[0]).unwrap_or(raw);
        // 깨진 바이트가 있어도 파일 전체를 못 여는 것보다는 낫기 때문에 대체 문자로 읽는다
        let text = if self.utf8 {
            String::from_utf8_lossy(raw).into_owned()
        } else {
            encoding_rs::SHIFT_JIS.decode_without_bom_handling(raw).0.into_owned()
        };
        Ok(WStr { text, span: start..self.pos })
    }

    pub fn expect(&mut self, magic: &[u8], what: &str) -> R<()> {
        let at = self.pos;
        if self.bytes(magic.len())? != magic {
            return Err(format!("{what}을(를) 찾을 수 없습니다 (오프셋 {at:#x})"));
        }
        Ok(())
    }

    pub fn expect_u8(&mut self, value: u8, what: &str) -> R<()> {
        let at = self.pos;
        let b = self.u8()?;
        if b != value {
            return Err(format!("{what}: {value:#04x}이어야 하는데 {b:#04x}입니다 (오프셋 {at:#x})"));
        }
        Ok(())
    }
}

/// 매직 넘버를 비교한다. `utf8_at` 위치의 바이트는 2.x에서 0x00, 3.x(UTF-8)에서 0x55('U')다.
/// 일치하면 UTF-8 여부를 돌려준다.
pub fn match_magic(data: &[u8], magic: &[u8], utf8_at: usize) -> Option<bool> {
    let got = data.get(..magic.len())?;
    let same_except = got.iter().zip(magic).enumerate().all(|(i, (a, b))| i == utf8_at || a == b);
    if !same_except {
        return None;
    }
    match got[utf8_at] {
        0x00 => Some(false),
        0x55 => Some(true),
        _ => None,
    }
}

/// `at` 위치의 `[원래 크기 u32][압축 크기 u32][LZ4 블록]`을 풀어서
/// 앞부분(헤더)은 그대로 두고 그 뒤에 압축을 푼 본문을 이어 붙인 버퍼를 만든다.
/// 본문은 `at`부터 시작한다.
pub fn unpack(data: &[u8], at: usize) -> R<Vec<u8>> {
    let mut r = Reader::new(data, at, true);
    let size = r.u32()? as usize;
    let packed = r.u32()? as usize;
    let block = r.bytes(packed)?;
    let body = lz4_flex::block::decompress(block, size).map_err(|e| format!("LZ4 압축 해제 실패: {e}"))?;
    if body.len() != size {
        return Err(format!("LZ4 압축 해제 크기가 다릅니다 ({} / {size})", body.len()));
    }
    let mut out = Vec::with_capacity(at + size);
    out.extend_from_slice(&data[..at]);
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// 테스트 데이터 작성용
    #[derive(Default)]
    pub struct Writer(pub Vec<u8>);

    impl Writer {
        pub fn u8(&mut self, v: u8) -> &mut Self {
            self.0.push(v);
            self
        }
        pub fn u32(&mut self, v: u32) -> &mut Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn raw(&mut self, b: &[u8]) -> &mut Self {
            self.0.extend_from_slice(b);
            self
        }
        pub fn str(&mut self, s: &str) -> &mut Self {
            self.u32(s.len() as u32 + 1).raw(s.as_bytes()).u8(0)
        }
    }

    #[test]
    fn reads_strings_with_spans() {
        let mut w = Writer::default();
        w.u32(7).str("勇者").str("");
        let mut r = Reader::new(&w.0, 4, true);
        let a = r.string().unwrap();
        assert_eq!((a.text.as_str(), a.span.clone()), ("勇者", 4..15));
        assert_eq!(r.string().unwrap().text, "");
        assert!(r.is_eof());
        assert!(r.u8().is_err());

        // Shift-JIS
        let sjis = [3, 0, 0, 0, 0x8C, 0x95, 0];
        assert_eq!(Reader::new(&sjis, 0, false).string().unwrap().text, "剣");
    }

    #[test]
    fn magic_and_unpack() {
        let magic = [0x57, 0, 0, 0x4F, 0x4C, 0, 0x46, 0x43, 0];
        assert_eq!(match_magic(&[0x57, 0, 0, 0x4F, 0x4C, 0x55, 0x46, 0x43, 0, 9], &magic, 5), Some(true));
        assert_eq!(match_magic(&magic, &magic, 5), Some(false));
        assert_eq!(match_magic(&[0x57, 0, 0, 0x4F, 0x4C, 0x55, 0x46, 0x4D, 0], &magic, 5), None);

        let body = b"hello hello hello hello";
        let packed = lz4_flex::block::compress(body);
        let mut w = Writer::default();
        w.raw(b"HDR").u32(body.len() as u32).u32(packed.len() as u32).raw(&packed);
        assert_eq!(unpack(&w.0, 3).unwrap(), [b"HDR".as_slice(), body].concat());
    }
}
