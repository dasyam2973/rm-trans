//! 텍스트 파일 추출과 적용 (게임 자체 스크립트 등, 엔진과 무관).
//!
//! 프로젝트 옵션의 `TextRule`에 맞는 파일에서
//! - 명령·주석 접두(`skip_prefixes`)로 시작하지 않는 줄이 이어진 묶음을 문장 하나(`Kind::ScriptText`)로,
//! - 지정한 명령 줄(`arg_commands`)의 인자 토큰 하나를 문장 하나(`Kind::ScriptArg`)로 뽑는다.
//!
//! 경로: 묶음은 `/lines/{첫 줄 번호}`, 명령 인자는 `/lines/{줄 번호}/{인자 번호}` (줄 번호는 0부터).
//! 줄은 바이트 단위로 나누고 줄마다 원래 줄바꿈을 기억해 두므로, 번역하지 않은 줄은 바이트 그대로 남는다
//! (Shift-JIS 파일도 마찬가지). UTF-8 BOM도 유지한다.

use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};
use walkdir::WalkDir;

use crate::model::{Entry, Kind};
use crate::store::{TextRule, WORK_DIR};

const BOM: &[u8] = b"\xEF\xBB\xBF";
/// 명령 인자 번역문의 공백은 인자 구분자로 읽히지 않도록 전각 공백으로 바꾼다
const ARG_SPACE: char = '\u{3000}';

/// 규칙별 파일 패턴
pub struct Rules<'a> {
    rules: Vec<(&'a TextRule, GlobMatcher)>,
}

impl<'a> Rules<'a> {
    /// 잘못된 패턴은 경고로 알리고 건너뛴다
    pub fn new(rules: &'a [TextRule], warnings: &mut Vec<String>) -> Self {
        let rules = rules
            .iter()
            .filter(|r| !r.pattern.trim().is_empty())
            .filter_map(|r| {
                // `*`는 폴더를 넘지 않고 `**`만 넘는다. 윈도우 파일 이름처럼 대소문자는 구분하지 않는다
                let glob = GlobBuilder::new(r.pattern.trim()).literal_separator(true).case_insensitive(true).build();
                match glob {
                    Ok(g) => Some((r, g.compile_matcher())),
                    Err(e) => {
                        warnings.push(format!("텍스트 파일 패턴 {}: {e}", r.pattern));
                        None
                    }
                }
            })
            .collect();
        Rules { rules }
    }

    /// 파일(게임 루트 기준, '/' 구분)에 맞는 첫 규칙
    pub fn rule_for(&self, file: &str) -> Option<&'a TextRule> {
        self.rules.iter().find(|(_, m)| m.is_match(file)).map(|(r, _)| *r)
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// 규칙에 맞는 모든 파일에서 추출한다. 읽을 수 없는 파일은 경고로 알린다.
pub fn extract_all(root: &Path, rules: &[TextRule], out: &mut Vec<Entry>, warnings: &mut Vec<String>) {
    let rules = Rules::new(rules, warnings);
    if rules.is_empty() {
        return;
    }
    let mut files: Vec<(String, &TextRule)> = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !(e.depth() == 1 && e.file_name() == WORK_DIR))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            let rel = e.path().strip_prefix(root).ok()?;
            let rel = rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            let rule = rules.rule_for(&rel)?;
            Some((rel, rule))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    if files.is_empty() {
        warnings.push("텍스트 파일 규칙에 맞는 파일이 없습니다.".into());
    }
    for (file, rule) in files {
        match fs::read(join_rel(root, &file)) {
            Ok(raw) => out.extend(scan(&TextFile::decode(&raw), rule, &file).into_iter().map(|(e, _)| e)),
            Err(e) => warnings.push(format!("{file}: {e}")),
        }
    }
}

fn join_rel(root: &Path, rel: &str) -> std::path::PathBuf {
    rel.split('/').fold(root.to_path_buf(), |p, part| p.join(part))
}

struct Line {
    /// 줄바꿈을 뺀 원래 바이트
    raw: Vec<u8>,
    /// 원래 줄바꿈 (`\r\n`, `\n`, 마지막 줄이면 빈 값)
    eol: &'static [u8],
    text: String,
}

struct TextFile {
    bom: bool,
    utf8: bool,
    lines: Vec<Line>,
}

impl TextFile {
    fn decode(raw: &[u8]) -> TextFile {
        let (bom, body) = match raw.strip_prefix(BOM) {
            Some(rest) => (true, rest),
            None => (false, raw),
        };
        let utf8 = std::str::from_utf8(body).is_ok();
        let mut lines = Vec::new();
        let mut rest = body;
        loop {
            let (content, eol, next): (&[u8], &'static [u8], _) = match rest.iter().position(|&b| b == b'\n') {
                Some(n) if n > 0 && rest[n - 1] == b'\r' => (&rest[..n - 1], b"\r\n", Some(&rest[n + 1..])),
                Some(n) => (&rest[..n], b"\n", Some(&rest[n + 1..])),
                None => (rest, b"", None),
            };
            let text = if utf8 {
                String::from_utf8_lossy(content).into_owned()
            } else {
                encoding_rs::SHIFT_JIS.decode_without_bom_handling(content).0.into_owned()
            };
            lines.push(Line { raw: content.to_vec(), eol, text });
            match next {
                Some(n) => rest = n,
                None => break,
            }
        }
        // 파일이 줄바꿈으로 끝나면 마지막에 생기는 빈 줄은 줄이 아니다
        if lines.len() > 1 && lines.last().is_some_and(|l| l.raw.is_empty()) {
            lines.pop();
        }
        TextFile { bom, utf8, lines }
    }

    fn encode_text(&self, text: &str) -> Option<Vec<u8>> {
        if self.utf8 {
            return Some(text.as_bytes().to_vec());
        }
        let (bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode(text);
        (!had_errors).then(|| bytes.into_owned())
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut out = if self.bom { BOM.to_vec() } else { Vec::new() };
        for line in &self.lines {
            out.extend_from_slice(&line.raw);
            out.extend_from_slice(line.eol);
        }
        out
    }
}

/// 문장의 위치
#[derive(Clone)]
enum Loc {
    /// 줄 범위
    Block(Range<usize>),
    /// 줄 하나 안의 바이트 범위 (인자 토큰)
    Arg { line: usize, range: Range<usize> },
}

fn is_blank(s: &str) -> bool {
    s.trim_matches([' ', '\t']).is_empty()
}

fn is_skip(rule: &TextRule, s: &str) -> bool {
    let t = s.trim_start_matches([' ', '\t', '\u{feff}']);
    rule.skip_prefixes.iter().any(|p| !p.is_empty() && t.starts_with(p.as_str()))
}

/// 제어 코드(`\cself[5]` 등)와 태그를 빼고 글자가 남는지 (WOLF RPG 규칙이지만 RPG Maker 코드도 같은 형태)
fn has_text(s: &str) -> bool {
    crate::wolf::has_text(s)
}

/// 공백(반각 공백, 탭)으로 나눈 토큰들의 바이트 범위
fn tokens(s: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in s.char_indices().chain([(s.len(), ' ')]) {
        match (c == ' ' || c == '\t', start) {
            (true, Some(st)) => {
                out.push(st..i);
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    out
}

fn scan(tf: &TextFile, rule: &TextRule, file: &str) -> Vec<(Entry, Loc)> {
    let name = file.rsplit('/').next().unwrap_or(file);
    let mut out = Vec::new();
    let mut push = |path: String, kind, label: String, context: Option<&str>, text: &str, loc: Loc| {
        if !has_text(text) {
            return;
        }
        let group = format!("{file}#{path}");
        if let Some(e) = Entry::text(file, path, kind, &group, &label, context, text) {
            out.push((e, loc));
        }
    };

    let lines = &tf.lines;
    let mut i = 0;
    while i < lines.len() {
        let text = &lines[i].text;
        if is_blank(text) {
            i += 1;
            continue;
        }
        if is_skip(rule, text) {
            let toks = tokens(text);
            let command = toks.first().map(|r| &text[r.clone()]);
            for ac in rule.arg_commands.iter().filter(|ac| Some(ac.command.as_str()) == command) {
                if let Some(range) = toks.get(ac.arg) {
                    let label = format!("{name} · {}행 {}", i + 1, ac.command);
                    let loc = Loc::Arg { line: i, range: range.clone() };
                    push(format!("/lines/{i}/{}", ac.arg), Kind::ScriptArg, label, None, &text[range.clone()], loc);
                }
            }
            i += 1;
            continue;
        }
        // 바로 앞 줄이 명령이면 화자 등을 알 수 있도록 맥락으로 붙인다
        let context = i.checked_sub(1).map(|p| lines[p].text.trim()).filter(|p| is_skip(rule, p));
        let start = i;
        while i < lines.len() && !is_blank(&lines[i].text) && !is_skip(rule, &lines[i].text) {
            i += 1;
        }
        let block: Vec<&str> = lines[start..i].iter().map(|l| l.text.as_str()).collect();
        let label = format!("{name} · {}행", start + 1);
        push(format!("/lines/{start}"), Kind::ScriptText, label, context, &block.join("\n"), Loc::Block(start..i));
    }
    out
}

pub struct Patched {
    pub bytes: Vec<u8>,
    pub applied: usize,
    /// 건너뛴 아이템 경로 (이유가 붙을 수 있음)
    pub skipped: Vec<String>,
}

/// 원본 파일(`root` 기준 `file`)에 번역을 적용한 내용을 만든다.
pub fn patch_file(root: &Path, rule: &TextRule, file: &str, patches: &[(String, String)]) -> Result<Patched, String> {
    let raw = fs::read(join_rel(root, file)).map_err(|e| e.to_string())?;
    let mut tf = TextFile::decode(&raw);
    let locs: HashMap<String, Loc> = scan(&tf, rule, file).into_iter().map(|(e, loc)| (e.path, loc)).collect();

    let mut blocks = Vec::new();
    let mut args: Vec<(usize, Range<usize>, Vec<u8>)> = Vec::new();
    let mut skipped = Vec::new();
    for (path, text) in patches {
        let Some(loc) = locs.get(path) else {
            skipped.push(path.clone());
            continue;
        };
        let reject = |why: &str| format!("{path} ({why})");
        match loc {
            Loc::Block(range) => {
                // 빈 줄은 묶음을 끝내고, 명령 접두로 시작하는 줄은 명령으로 읽히므로 쓸 수 없다
                let new_lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).filter(|l| !is_blank(l)).collect();
                if new_lines.is_empty() {
                    skipped.push(reject("빈 번역문"));
                } else if new_lines.iter().any(|l| is_skip(rule, l)) {
                    skipped.push(reject("명령 접두로 시작하는 줄이 있음"));
                } else {
                    match new_lines.iter().map(|l| tf.encode_text(l)).collect::<Option<Vec<_>>>() {
                        Some(encoded) => blocks.push((range.clone(), encoded)),
                        None => skipped.push(reject("Shift-JIS로 나타낼 수 없는 문자가 있음")),
                    }
                }
            }
            Loc::Arg { line, range } => {
                let token: String =
                    text.chars().filter(|c| !matches!(c, '\r' | '\n')).map(|c| if c == ' ' || c == '\t' { ARG_SPACE } else { c }).collect();
                if token.trim_matches(ARG_SPACE).is_empty() {
                    skipped.push(reject("빈 번역문"));
                    continue;
                }
                // 토큰 범위는 디코딩한 글자 기준이므로 원래 바이트 위치로 바꾼다
                let line_text = &tf.lines[*line].text;
                let (Some(before), Some(after)) = (tf.encode_text(&line_text[..range.start]), tf.encode_text(&line_text[range.end..])) else {
                    skipped.push(reject("원본 줄을 다시 인코딩할 수 없음"));
                    continue;
                };
                match tf.encode_text(&token) {
                    Some(bytes) => args.push((*line, before.len()..tf.lines[*line].raw.len() - after.len(), bytes)),
                    None => skipped.push(reject("Shift-JIS로 나타낼 수 없는 문자가 있음")),
                }
            }
        }
    }
    let applied = blocks.len() + args.len();

    // 인자는 같은 줄 안에서 뒤에서부터, 묶음은 줄 수가 바뀌므로 아래에서부터 바꾼다
    args.sort_by(|a, b| (b.0, b.1.start).cmp(&(a.0, a.1.start)));
    for (line, range, bytes) in args {
        tf.lines[line].raw.splice(range, bytes);
    }
    blocks.sort_by(|a, b| b.0.start.cmp(&a.0.start));
    for (range, encoded) in blocks {
        // 새 줄들은 묶음 첫 줄의 줄바꿈을 쓰고, 마지막 줄은 원래 마지막 줄의 줄바꿈을 유지한다
        let (first_eol, last_eol) = (tf.lines[range.start].eol, tf.lines[range.end - 1].eol);
        let n = encoded.len();
        let new_lines = encoded.into_iter().enumerate().map(|(k, raw)| Line {
            raw,
            eol: if k + 1 == n { last_eol } else if first_eol.is_empty() { b"\n" } else { first_eol },
            text: String::new(),
        });
        tf.lines.splice(range, new_lines.collect::<Vec<_>>());
    }
    Ok(Patched { bytes: tf.to_bytes(), applied, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ArgCommand;

    fn rule() -> TextRule {
        TextRule {
            pattern: "Data/Text_Script/**/*.txt".into(),
            skip_prefixes: vec!["@".into(), "#".into(), "::".into()],
            arg_commands: vec![ArgCommand { command: "@putselect".into(), arg: 1 }, ArgCommand { command: "@select".into(), arg: 3 }],
        }
    }

    const SCRIPT: &str = "::initialize\r\n\r\n#コメント\r\n@mes パルン odoodo1\r\nセラおねえ……\r\nひさしぶりー……\r\n\r\n@putselect つぶあん\r\n@select 1 100 どっちが好き？\r\n@wait 30\r\n\\cself[5]\r\n";

    fn write(base: &Path, rel: &str, bytes: &[u8]) {
        let path = join_rel(base, rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn extracts_blocks_and_args() {
        let tf = TextFile::decode(&[BOM, SCRIPT.as_bytes()].concat());
        let got: Vec<_> = scan(&tf, &rule(), "a.txt")
            .into_iter()
            .map(|(e, _)| (e.path, e.kind, e.original, e.context))
            .collect();
        assert_eq!(
            got,
            [
                ("/lines/4".into(), Kind::ScriptText, "セラおねえ……\nひさしぶりー……".into(), Some("@mes パルン odoodo1".into())),
                ("/lines/7/1".into(), Kind::ScriptArg, "つぶあん".into(), None),
                ("/lines/8/3".into(), Kind::ScriptArg, "どっちが好き？".into(), None),
            ]
        );
    }

    #[test]
    fn patches_keep_untouched_bytes() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-text-{}", std::process::id()));
        let file = "Data/Text_Script/ev/a.txt";
        let src = [BOM, SCRIPT.as_bytes()].concat();
        write(&base, file, &src);
        let r = patch_file(
            &base,
            &rule(),
            file,
            &[
                ("/lines/4".into(), "세라 언니……\n\n오랜만이야……\n정말로".into()),
                ("/lines/7/1".into(), "통팥 앙금".into()),
                ("/lines/8/3".into(), "@어느 쪽?".into()),
                ("/lines/99".into(), "x".into()),
            ],
        )
        .unwrap();
        assert_eq!((r.applied, r.skipped.as_slice()), (3, ["/lines/99".to_string()].as_slice()));
        let expected = SCRIPT
            .replace("セラおねえ……\r\nひさしぶりー……", "세라 언니……\r\n오랜만이야……\r\n정말로")
            .replace("つぶあん", "통팥\u{3000}앙금")
            .replace("どっちが好き？", "@어느\u{3000}쪽?");
        assert_eq!(r.bytes, [BOM, expected.as_bytes()].concat());

        // 명령 접두로 시작하는 줄이 생기면 적용하지 않는다
        let r = patch_file(&base, &rule(), file, &[("/lines/4".into(), "#해시\n둘째".into())]).unwrap();
        assert_eq!(r.applied, 0);
        assert_eq!(r.bytes, src);
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn shift_jis_files() {
        let base = std::env::temp_dir().join(format!("rmtrans-test-text-sjis-{}", std::process::id()));
        let file = "Data/Text_Script/b.txt";
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode("@mes 村人\n剣を買う\n盾\n");
        write(&base, file, &sjis);
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        extract_all(&base, &[rule()], &mut out, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(out.iter().map(|e| e.original.as_str()).collect::<Vec<_>>(), ["剣を買う\n盾"]);

        let r = patch_file(&base, &rule(), file, &[("/lines/1".into(), "검을 산다".into())]).unwrap();
        assert_eq!(r.applied, 0);
        assert!(r.skipped[0].contains("Shift-JIS"));
        let r = patch_file(&base, &rule(), file, &[("/lines/1".into(), "盾を買う".into())]).unwrap();
        assert_eq!(r.bytes, encoding_rs::SHIFT_JIS.encode("@mes 村人\n盾を買う\n").0.into_owned());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn rule_matching() {
        let rules = [rule()];
        let mut w = Vec::new();
        let r = Rules::new(&rules, &mut w);
        assert!(r.rule_for("Data/Text_Script/ev-PA/a.txt").is_some());
        assert!(r.rule_for("data/text_script/A.TXT").is_some());
        assert!(r.rule_for("Data/Other/a.txt").is_none());
        let bad = [TextRule { pattern: "a/[".into(), ..rule() }];
        Rules::new(&bad, &mut w);
        assert_eq!(w.len(), 1);
    }
}

#[cfg(test)]
mod real_game {
    use super::*;
    use crate::store::ArgCommand;

    /// 실제 게임의 텍스트 파일로 확인한다 (규칙은 Text_Script 형식 기준).
    /// `TEXT_GAME=<게임 폴더> cargo test --lib text_roundtrip -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn text_roundtrip() {
        let root = std::path::PathBuf::from(std::env::var("TEXT_GAME").expect("TEXT_GAME 환경 변수가 필요합니다"));
        let arg = |command: &str, arg| ArgCommand { command: command.into(), arg };
        let rule = TextRule {
            pattern: "Data/Text_Script/**/*.txt".into(),
            skip_prefixes: vec!["@".into(), "#".into(), "::".into()],
            arg_commands: vec![arg("@putselect", 1), arg("@select", 3), arg("@jimaku", 2), arg("@toki-teach", 1)],
        };
        let mut entries = Vec::new();
        let mut warnings = Vec::new();
        extract_all(&root, std::slice::from_ref(&rule), &mut entries, &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut by_file: std::collections::BTreeMap<&str, Vec<&Entry>> = Default::default();
        for e in &entries {
            by_file.entry(e.file.as_str()).or_default().push(e);
        }
        let (texts, args) = entries.iter().fold((0, 0), |(t, a), e| if e.kind == Kind::ScriptText { (t + 1, a) } else { (t, a + 1) });
        println!("{} files, {texts} blocks, {args} args", by_file.len());
        for e in entries.iter().step_by(2500) {
            println!("{} | {:?} | {:?}", e.id, e.context, e.original);
        }

        for (file, list) in by_file {
            let raw = fs::read(join_rel(&root, file)).unwrap();
            let same: Vec<_> = list.iter().map(|e| (e.path.clone(), e.original.clone())).collect();
            let r = patch_file(&root, &rule, file, &same).unwrap();
            assert!(r.skipped.is_empty(), "{file}: {:?}", r.skipped);
            assert_eq!(r.bytes, raw, "{file}: 원문 그대로 적용");

            let starred: Vec<_> = list.iter().map(|e| (e.path.clone(), format!("{}★", e.original))).collect();
            let r = patch_file(&root, &rule, file, &starred).unwrap();
            assert_eq!(r.applied, list.len(), "{file}: {:?}", r.skipped);
            let reread = scan(&TextFile::decode(&r.bytes), &rule, file);
            let got: Vec<_> = reread.iter().map(|(e, _)| e.original.as_str()).collect();
            let expected: Vec<_> = starred.iter().map(|(_, t)| t.as_str()).collect();
            assert_eq!(got, expected, "{file}");
        }
    }
}
