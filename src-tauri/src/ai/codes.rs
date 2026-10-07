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

/// WOLF RPG에서 내용이 들어가 문장 속 위치가 바뀔 수 있는 코드
/// (셀프 변수, 문자열 변수, 변수, 시스템 변수, DB 값, 루비)
const WOLF_CONTENT_CODES: &[&str] = &["self", "cself", "s", "v", "sys", "sysS", "udb", "cdb", "sdb", "r"];

/// 제어 문자 규칙 (게임 엔진별)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dialect {
    #[default]
    Rpgm,
    Wolf,
}

#[derive(Debug, PartialEq)]
enum Piece<'a> {
    Text(&'a str),
    /// `pinnable`: 텍스트 앞뒤에 있을 때 떼어 둬도 되는 서식 코드인지
    Code { src: &'a str, pinnable: bool },
}

/// `src[i..]`가 제어 문자로 시작하면 (길이, 떼어 둬도 되는지)를 돌려준다.
fn code_at(src: &str, i: usize, dialect: Dialect) -> Option<(usize, bool)> {
    match dialect {
        Dialect::Rpgm => rpgm_code_at(src, i),
        Dialect::Wolf => wolf_code_at(src, i),
    }
}

/// RPG Maker 엔진의 `obtainEscapeCode`와 같은 규칙: `\` 뒤에 기호 한 글자 또는 영문자 이름 + 선택적 `[...]`/`<...>` 인자.
fn rpgm_code_at(src: &str, i: usize) -> Option<(usize, bool)> {
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

/// WOLF RPG 특수 문자: `\` 뒤에 기호 한 글자, `\-[n]`, 또는 영문자 이름 + 선택적 숫자(`\v5[67]`)
/// + 선택적 `+-=`(`\A+`) + 중첩될 수 있는 `[...]` 인자(`\f[\cself[18]]`). 줄 정렬 등의 태그(`<C>`, `<GAUGE>`)도 코드로 본다.
fn wolf_code_at(src: &str, i: usize) -> Option<(usize, bool)> {
    let rest = &src[i..];
    if let Some(after) = rest.strip_prefix('\\') {
        let c = after.chars().next()?;
        if c == '-' {
            return Some((2 + bracket_len(&rest[2..]), true));
        }
        if ".|!><^{}$@\\".contains(c) {
            return Some((2, c != '\\'));
        }
        let name_len = after.bytes().take_while(u8::is_ascii_alphabetic).count();
        if name_len == 0 {
            return None;
        }
        let name = &after[..name_len];
        let mut len = 1 + name_len;
        len += rest[len..].bytes().take_while(u8::is_ascii_digit).count();
        if name == "A" && rest[len..].starts_with(['+', '-', '=']) {
            len += 1;
        }
        len += bracket_len(&rest[len..]);
        let content = WOLF_CONTENT_CODES.iter().any(|c| c.eq_ignore_ascii_case(name));
        return Some((len, !content));
    }
    let tag = rest.strip_prefix('<')?;
    let n = tag.bytes().take_while(u8::is_ascii_uppercase).count();
    ((1..=8).contains(&n) && tag[n..].starts_with('>')).then_some((n + 2, true))
}

/// `s`가 `[`로 시작하면 짝이 맞는 `]`까지의 길이 (중첩 허용, 줄바꿈 전까지). 아니면 0
fn bracket_len(s: &str) -> usize {
    if !s.starts_with('[') {
        return 0;
    }
    let mut depth = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            '\n' => return 0,
            _ => {}
        }
    }
    0
}

/// WOLF RPG 텍스트 앞에서 그대로 떼어 둘 부분의 길이.
/// - `@1\n`: 메시지 맨 앞의 얼굴 그래픽 지정
/// - `smile1\r\n…`: 첫 줄이 공백 없는 영문/숫자 토큰 하나이고 나머지에 비ASCII 문자가 있으면 그 첫 줄
///   (게임 자체 시스템이 쓰는 표정·연출 지정인 경우가 많다)
fn wolf_head_len(src: &str) -> usize {
    let line_len = |s: &str| s.find('\n').map(|n| n + 1);
    let mut head = 0;
    if let Some(after) = src.strip_prefix('@') {
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        let line = &after[digits..];
        if digits > 0 && (line.trim_end().is_empty() || line.starts_with(['\r', '\n'])) {
            head = 1 + digits + line_len(line).unwrap_or(line.len());
        }
    }
    let rest = &src[head..];
    if let Some(n) = line_len(rest) {
        let first = rest[..n].trim_end();
        let is_token = !first.is_empty() && first.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if is_token && !rest[n..].is_ascii() {
            head += n;
        }
    }
    head
}

/// 메시지 창에서 차지하는 대략적인 폭 (전각 한 글자 = 1, 반각 = 0.5).
/// 서식 코드는 0, 아이콘은 1, 이름·변수처럼 내용이 들어가는 코드는 추정값을 쓴다.
pub fn display_width(s: &str) -> f32 {
    let mut width = 0.0;
    let mut i = 0;
    while i < s.len() {
        if let Some((len, _)) = code_at(s, i, Dialect::Rpgm) {
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

fn split(src: &str, dialect: Dialect) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut text_start = 0;
    let mut i = 0;
    while i < src.len() {
        if let Some((len, pinnable)) = code_at(src, i, dialect) {
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

pub fn mask(src: &str, dialect: Dialect) -> Masked {
    if src.contains([OPEN, CLOSE]) {
        let has_text = !src.trim().is_empty();
        return Masked { text: src.to_string(), prefix: String::new(), suffix: String::new(), codes: Vec::new(), passthrough: true, has_text };
    }
    // 얼굴 그래픽 지정 같은 앞부분은 코드와 마찬가지로 떼어 두었다가 그대로 다시 붙인다
    let head_len = if dialect == Dialect::Wolf { wolf_head_len(src) } else { 0 };
    let (head, src) = src.split_at(head_len);
    let pieces = split(src, dialect);
    let pinned = |p: &Piece| matches!(p, Piece::Code { pinnable: true, .. });
    let lead = pieces.iter().take_while(|p| pinned(p)).count();
    let trail = pieces[lead..].iter().rev().take_while(|p| pinned(p)).count();
    let src_of = |p: &Piece<'_>| match *p {
        Piece::Text(s) | Piece::Code { src: s, .. } => s.to_string(),
    };
    let prefix: String = std::iter::once(head.to_string()).chain(pieces[..lead].iter().map(src_of)).collect();
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
        let m = mask("\\C[2]\\N[1]は\\I[64]\\C[3]ポーション\\C[0]を手に入れた！\\.\\|", Dialect::Rpgm);
        assert_eq!(m.text, "⟦0⟧は⟦1⟧ポーション⟦2⟧を手に入れた！");
        assert_eq!(m.unmask("⟦0⟧ obtained ⟦1⟧Potion⟦2⟧!", true).unwrap(), "\\C[2]\\N[1] obtained \\I[64]\\C[3]Potion\\C[0]!\\.\\|");
    }

    #[test]
    fn format_specifiers_and_symbols() {
        let m = mask("%1の勝利！", Dialect::Rpgm);
        assert_eq!(m.text, "⟦0⟧の勝利！");
        assert_eq!(m.unmask("Victory for ⟦0⟧!", true).unwrap(), "Victory for %1!");
        // \\는 글자 그대로의 역슬래시라 앞에 있어도 떼어 두지 않는다
        assert_eq!(mask("\\\\ここ", Dialect::Rpgm).text, "⟦0⟧ここ");
        // 역슬래시 뒤에 코드가 아닌 문자가 오면 텍스트로 둔다
        assert_eq!(mask("a\\1b", Dialect::Rpgm).text, "a\\1b");
    }

    #[test]
    fn validates_placeholders() {
        let m = mask("\\V[1]個の\\C[2]石\\C[0]", Dialect::Rpgm);
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
        assert!(!mask("\\C[2]\\I[5]", Dialect::Rpgm).has_text());
        assert!(!mask("\\V[1]", Dialect::Rpgm).has_text());
        assert!(mask("\\C[2]아\\C[0]", Dialect::Rpgm).has_text());
    }

    #[test]
    fn width() {
        assert_eq!(display_width("안녕 ab"), 3.5);
        assert_eq!(display_width("\\C[2]\\I[64]ポーション\\C[0]"), 6.0);
        assert_eq!(display_width("\\N[1]は"), 5.0);
    }

    #[test]
    fn wolf_codes() {
        let w = |s| mask(s, Dialect::Wolf);
        // 중첩 괄호, 이름 뒤 숫자, \A-, \-[1], 정렬 태그
        assert_eq!(w("\\f[\\cself[18]]\\space[0]<C>タイトル").text, "タイトル");
        assert_eq!(w("\\cself[5]waitA-rak\\v5[67]-").text, "⟦0⟧waitA-rak⟦1⟧-");
        assert_eq!(w("\\my[-100]\\f[10]\\A-あ\\-[1]い").text, "あ⟦0⟧い");
        assert_eq!(w("\\c[0]<C>\\wE[3]\\f[50]タポリ\n\nみもり").text, "タポリ\n\nみもり");
        // 셀프 변수·DB 값은 앞에 있어도 자리표시자로 보낸다
        let m = w("\\cself[2]上がった！");
        assert_eq!(m.text, "⟦0⟧上がった！");
        assert_eq!(m.unmask("⟦0⟧ 올랐다!", true).unwrap(), "\\cself[2] 올랐다!");
        assert_eq!(w("「\\cdb[63:3:10]」を得た").text, "「⟦0⟧」を得た");
        // RPG Maker 규칙에서는 %1이 코드지만 WOLF RPG에서는 글자
        assert_eq!(w("50%1回").text, "50%1回");
        // 코드만 있으면 보내지 않는다
        let m = w("\\f[\\cself[18]]\\space[0]<C>\\cself[5]");
        assert_eq!(m.text, "⟦0⟧");
        assert!(!m.has_text());
    }

    #[test]
    fn wolf_heads_are_kept() {
        let w = |s| mask(s, Dialect::Wolf);
        // 얼굴 그래픽 지정
        let m = w("@1\n竜姫「んっ……」");
        assert_eq!(m.text, "竜姫「んっ……」");
        assert_eq!(m.unmask("용희 「읏……」", true).unwrap(), "@1\n용희 「읏……」");
        // 표정 지정 첫 줄
        let m = w("smile1\r\n…ふふっ。あたしのことすきなのー？");
        assert_eq!(m.text, "…ふふっ。あたしのことすきなのー？");
        assert_eq!(m.unmask("…후훗. 나 좋아해?", true).unwrap(), "smile1\r\n…후훗. 나 좋아해?");
        assert_eq!(w("@2\nshy\r\nつの…").text, "つの…");
        // 영문만 있는 글은 첫 줄을 떼지 않는다
        assert_eq!(w("Hello\nWorld").text, "Hello\nWorld");
        // RPG Maker 규칙에서는 떼지 않는다
        assert_eq!(mask("@1\nあ", Dialect::Rpgm).text, "@1\nあ");
    }

    #[test]
    fn existing_placeholder_chars_pass_through() {
        let m = mask("⟦特殊⟧\\C[2]", Dialect::Rpgm);
        assert_eq!(m.text, "⟦特殊⟧\\C[2]");
        assert_eq!(m.unmask("⟦특수⟧\\C[2]", true).unwrap(), "⟦특수⟧\\C[2]");
    }
}

#[cfg(test)]
mod real_game {
    use super::*;

    /// 실제 WOLF RPG 게임의 모든 아이템을 마스킹했다가 그대로 되돌리면 원문과 같아야 한다.
    /// `WOLF_GAME=<경로> cargo test --lib wolf_mask_roundtrip -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn wolf_mask_roundtrip() {
        let root = std::env::var("WOLF_GAME").expect("WOLF_GAME 환경 변수가 필요합니다");
        let layout = crate::engine::detect(std::path::Path::new(&root)).ok().expect("게임 폴더");
        let entries = crate::engine::extract_all(&layout, &Default::default()).unwrap().entries;
        let (mut sent, mut heads, mut skipped) = (0, 0, 0);
        for e in &entries {
            let m = mask(&e.original, Dialect::Wolf);
            assert_eq!(m.unmask(&m.text, true).as_deref(), Some(e.original.as_str()), "{}", e.id);
            if !m.has_text() {
                skipped += 1;
                continue;
            }
            sent += 1;
            if !m.prefix.is_empty() && !m.prefix.starts_with(['\\', '<']) {
                heads += 1;
                if heads <= 5 {
                    println!("head {:?} | {:?}", m.prefix, m.text.chars().take(30).collect::<String>());
                }
            }
            if sent % 600 == 0 && !m.codes.is_empty() {
                println!("{:?}\n  -> {:?} codes={:?}", e.original, m.text, m.codes);
            }
        }
        println!("{} items: sent {sent}, code-only {skipped}, head kept {heads}", entries.len());
    }
}
