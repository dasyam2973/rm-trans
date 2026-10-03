//! 제어 문자 마스킹.
//!
//! AI에 보내기 전에 RPG Maker 제어 문자(\C[2], \N[1], %1 등)를 `⟦n⟧` 자리표시자로 바꾸고,
//! 응답의 자리표시자를 검사한 뒤 원래 코드로 되돌린다. 제어 문자 보존을 프롬프트 지시에 맡기지 않기 위함.
//!
//! - 연속된 코드는 자리표시자 하나로 묶는다.
//! - 텍스트 앞뒤에 붙은 서식 코드(색, 아이콘, 대기 등)는 아예 떼어 두었다가 그대로 다시 붙인다.
//!   이름·변수처럼 문장 속 위치가 바뀌어야 하는 코드는 앞뒤에 있어도 자리표시자로 보낸다.

const OPEN: char = '⟦';
const CLOSE: char = '⟧';

/// 번역 시 문장 속 위치가 바뀔 수 있는 코드 (\V 변수, \N 액터 이름, \P 파티원 이름, \G 통화 단위)
const CONTENT_CODES: &[&str] = &["V", "N", "P", "G"];

#[derive(Debug, PartialEq)]
enum Piece<'a> {
    Text(&'a str),
    /// `pinnable`: 텍스트 앞뒤에 있을 때 떼어 둬도 되는 서식 코드인지
    Code { src: &'a str, pinnable: bool },
}

/// `src[i..]`가 제어 문자로 시작하면 (길이, 떼어 둬도 되는지)를 돌려준다.
/// 엔진의 `obtainEscapeCode`와 같은 규칙: `\` 뒤에 기호 한 글자 또는 영문자 이름 + 선택적 `[...]`/`<...>` 인자.
fn code_at(src: &str, i: usize) -> Option<(usize, bool)> {
    let rest = &src[i..];
    if let Some(after) = rest.strip_prefix('\\') {
        let c = after.chars().next()?;
        if ".|!><^{}$\\".contains(c) {
            return Some((2, c != '\\'));
        }
        let name_len = after.bytes().take_while(u8::is_ascii_alphabetic).count();
        if name_len == 0 {
            return None;
        }
        let name = &after[..name_len];
        let mut len = 1 + name_len;
        for (open, close) in [('[', ']'), ('<', '>')] {
            if rest[len..].starts_with(open) {
                if let Some(end) = rest[len..].find(close).filter(|&e| !rest[len..len + e].contains('\n')) {
                    len += end + 1;
                }
                break;
            }
        }
        let content = CONTENT_CODES.iter().any(|c| c.eq_ignore_ascii_case(name));
        return Some((len, !content));
    }
    // System 메시지의 포맷 지시자 (%1, %2 …)
    let digits = rest.strip_prefix('%')?.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0).then_some((1 + digits, false))
}

/// 메시지 창에서 차지하는 대략적인 폭 (전각 한 글자 = 1, 반각 = 0.5).
/// 서식 코드는 0, 아이콘은 1, 이름·변수처럼 내용이 들어가는 코드는 추정값을 쓴다.
pub fn display_width(s: &str) -> f32 {
    let mut width = 0.0;
    let mut i = 0;
    while i < s.len() {
        if let Some((len, _)) = code_at(s, i) {
            width += code_width(&s[i..i + len]);
            i += len;
        } else {
            let c = s[i..].chars().next().expect("문자 경계");
            // 라틴/키릴 문자 등은 반각, 한글 자모(U+1100)부터는 전각으로 본다. 반각 가타카나는 예외
            width += if c < '\u{1100}' || ('\u{FF61}'..='\u{FF9F}').contains(&c) { 0.5 } else { 1.0 };
            i += c.len_utf8();
        }
    }
    width
}

fn code_width(code: &str) -> f32 {
    if code.starts_with('%') {
        return 1.0;
    }
    let name: String = code[1..].chars().take_while(char::is_ascii_alphabetic).collect();
    match name.to_ascii_uppercase().as_str() {
        "I" | "G" => 1.0,
        "V" => 2.0,
        "N" | "P" => 4.0,
        "" if code == "\\\\" => 0.5,
        _ => 0.0,
    }
}

fn split(src: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while i < src.len() {
        if let Some((len, pinnable)) = code_at(src, i) {
            if text_start < i {
                pieces.push(Piece::Text(&src[text_start..i]));
            }
            pieces.push(Piece::Code { src: &src[i..i + len], pinnable });
            i += len;
            text_start = i;
        } else {
            i += src[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    if text_start < src.len() {
        pieces.push(Piece::Text(&src[text_start..]));
    }
    pieces
}

#[derive(Debug)]
pub struct Masked {
    /// AI에 보낼 텍스트
    pub text: String,
    prefix: String,
    suffix: String,
    /// 자리표시자 번호 → 원래 코드 (연속된 코드는 하나로 합쳐져 있다)
    codes: Vec<String>,
    /// 원문에 자리표시자 문자가 이미 있어 마스킹하지 않은 경우
    passthrough: bool,
    has_text: bool,
}

pub fn mask(src: &str) -> Masked {
    if src.contains([OPEN, CLOSE]) {
        let has_text = !src.trim().is_empty();
        return Masked { text: src.to_string(), prefix: String::new(), suffix: String::new(), codes: Vec::new(), passthrough: true, has_text };
    }
    let pieces = split(src);
    let pinned = |p: &Piece| matches!(p, Piece::Code { pinnable: true, .. });
    let lead = pieces.iter().take_while(|p| pinned(p)).count();
    let trail = pieces[lead..].iter().rev().take_while(|p| pinned(p)).count();
    let src_of = |p: &Piece<'_>| match *p {
        Piece::Text(s) | Piece::Code { src: s, .. } => s.to_string(),
    };
    let prefix: String = pieces[..lead].iter().map(src_of).collect();
    let suffix: String = pieces[pieces.len() - trail..].iter().map(src_of).collect();

    let mut text = String::new();
    let mut codes: Vec<String> = Vec::new();
    let mut has_text = false;
    let mut prev_code = false;
    for piece in &pieces[lead..pieces.len() - trail] {
        match *piece {
            Piece::Text(s) => {
                has_text |= !s.trim().is_empty();
                text.push_str(s);
                prev_code = false;
            }
            Piece::Code { src: s, .. } => {
                match codes.last_mut() {
                    Some(last) if prev_code => last.push_str(s),
                    _ => {
                        text.push_str(&format!("{OPEN}{}{CLOSE}", codes.len()));
                        codes.push(s.to_string());
                    }
                }
                prev_code = true;
            }
        }
    }
    Masked { text, prefix, suffix, codes, passthrough: false, has_text }
}

impl Masked {
    /// 제어 문자를 빼면 번역할 텍스트가 남지 않는 경우 false
    pub fn has_text(&self) -> bool {
        self.has_text
    }

    /// 번역문의 자리표시자를 원래 코드로 되돌린다.
    /// 없는 번호나 깨진 자리표시자가 있으면 항상 실패. `require_all`이면 모든 자리표시자가 정확히 한 번씩 있어야 한다.
    pub fn unmask(&self, translated: &str, require_all: bool) -> Option<String> {
        if self.passthrough {
            return Some(translated.to_string());
        }
        let mut out = self.prefix.clone();
        let mut counts = vec![0usize; self.codes.len()];
        let mut rest = translated;
        while let Some(start) = rest.find([OPEN, CLOSE]) {
            let token = rest[start..].strip_prefix(OPEN)?;
            let end = token.find(CLOSE)?;
            let n: usize = token[..end].trim().parse().ok()?;
            *counts.get_mut(n)? += 1;
            out.push_str(&rest[..start]);
            out.push_str(&self.codes[n]);
            rest = &token[end + CLOSE.len_utf8()..];
        }
        if require_all && counts.iter().any(|&c| c != 1) {
            return None;
        }
        out.push_str(rest);
        out.push_str(&self.suffix);
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_and_restores() {
        let m = mask("\\C[2]\\N[1]は\\I[64]\\C[3]ポーション\\C[0]を手に入れた！\\.\\|");
        assert_eq!(m.text, "⟦0⟧は⟦1⟧ポーション⟦2⟧を手に入れた！");
        assert_eq!(m.unmask("⟦0⟧ obtained ⟦1⟧Potion⟦2⟧!", true).unwrap(), "\\C[2]\\N[1] obtained \\I[64]\\C[3]Potion\\C[0]!\\.\\|");
    }

    #[test]
    fn format_specifiers_and_symbols() {
        let m = mask("%1の勝利！");
        assert_eq!(m.text, "⟦0⟧の勝利！");
        assert_eq!(m.unmask("Victory for ⟦0⟧!", true).unwrap(), "Victory for %1!");
        // \\는 글자 그대로의 역슬래시라 앞에 있어도 떼어 두지 않는다
        assert_eq!(mask("\\\\ここ").text, "⟦0⟧ここ");
        // 역슬래시 뒤에 코드가 아닌 문자가 오면 텍스트로 둔다
        assert_eq!(mask("a\\1b").text, "a\\1b");
    }

    #[test]
    fn validates_placeholders() {
        let m = mask("\\V[1]個の\\C[2]石\\C[0]");
        assert_eq!(m.text, "⟦0⟧個の⟦1⟧石");
        // 순서가 바뀌는 건 허용
        assert_eq!(m.unmask("⟦1⟧돌⟦0⟧개", true).unwrap(), "\\C[2]돌\\V[1]개\\C[0]");
        // 빠짐: 엄격 모드에서만 실패
        assert!(m.unmask("돌 ⟦0⟧개", true).is_none());
        assert_eq!(m.unmask("돌 ⟦0⟧개", false).unwrap(), "돌 \\V[1]개\\C[0]");
        // 중복: 엄격 모드에서만 실패
        assert!(m.unmask("⟦0⟧⟦0⟧⟦1⟧돌", true).is_none());
        // 없는 번호나 깨진 자리표시자는 항상 실패
        assert!(m.unmask("⟦0⟧⟦1⟧⟦2⟧", false).is_none());
        assert!(m.unmask("⟦0⟧⟦1 돌", false).is_none());
        assert!(m.unmask("⟦0⟧1⟧ 돌", false).is_none());
    }

    #[test]
    fn code_only_text() {
        assert!(!mask("\\C[2]\\I[5]").has_text());
        assert!(!mask("\\V[1]").has_text());
        assert!(mask("\\C[2]아\\C[0]").has_text());
    }

    #[test]
    fn width() {
        assert_eq!(display_width("안녕 ab"), 3.5);
        assert_eq!(display_width("\\C[2]\\I[64]ポーション\\C[0]"), 6.0);
        assert_eq!(display_width("\\N[1]は"), 5.0);
    }

    #[test]
    fn existing_placeholder_chars_pass_through() {
        let m = mask("⟦特殊⟧\\C[2]");
        assert_eq!(m.text, "⟦特殊⟧\\C[2]");
        assert_eq!(m.unmask("⟦특수⟧\\C[2]", true).unwrap(), "⟦특수⟧\\C[2]");
    }
}
