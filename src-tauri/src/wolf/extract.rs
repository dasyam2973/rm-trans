//! WOLF RPG 데이터에서 번역 가능한 문자열을 뽑아낸다.
//!
//! 아이템 경로는 바이트 위치가 아니라 구조 경로다 (게임이 갱신돼도 같은 항목을 가리키도록).
//! - Game.dat: `/title` 등
//! - DB(.dat): `/types/{타입}/data/{데이터}/{필드}`
//! - CommonEvent.dat: `/events/{순번}/list/{커맨드}/{문자열 인자}`
//! - 맵(.mps): `/events/{순번}/pages/{페이지}/list/{커맨드}/{문자열 인자}`
//!
//! 아이템마다 문자열의 바이트 범위도 함께 모아서 내보내기(`apply.rs`)가 같은 규칙으로 찾아 쓴다.
//! 파일 하나를 읽지 못해도 나머지는 계속 추출하고 경고로 알린다.

use std::fs;
use std::ops::Range;
use std::path::Path;

use walkdir::WalkDir;

use crate::error::Result;
use crate::model::{Entry, Extracted, Kind};

use super::command::{code, Command};
use super::reader::{WStr, R};
use super::{common, database, game_dat, map, WolfLayout, BASIC_DATA, MAP_DATA};

/// (파일 이름, 표시 이름)
const DATABASES: &[(&str, &str)] = &[("DataBase", "유저 DB"), ("CDataBase", "가변 DB"), ("SysDatabase", "시스템 DB")];

/// 파일 이름으로 보고 추출하지 않는 확장자
const FILE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "gif", "ogg", "wav", "mp3", "mid", "midi", "mp4", "webm", "avi", "wmv", "ttf",
    "otf", "txt", "mps", "dat", "wolf",
];

/// 파싱한 파일
pub(super) enum Parsed {
    Game(game_dat::GameDat),
    Db(database::Database),
    Common(common::CommonEvents),
    Map(map::Map),
}

/// 파일 하나의 추출 결과. `spans[i]`는 `entries[i]` 문자열의 바이트 범위 (파싱한 버퍼 기준)
pub(super) struct Loaded {
    pub parsed: Parsed,
    pub entries: Vec<Entry>,
    pub spans: Vec<Range<usize>>,
}

pub fn extract_all(layout: &WolfLayout) -> Result<Extracted> {
    let data_dir = layout.data_dir();
    let mut files = vec![format!("{BASIC_DATA}/Game.dat")];
    files.extend(DATABASES.iter().map(|(name, _)| format!("{BASIC_DATA}/{name}.dat")));
    files.push(format!("{BASIC_DATA}/CommonEvent.dat"));
    files.extend(map_files(&data_dir));

    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for rel in files {
        if !data_path(layout, &rel).is_file() {
            continue;
        }
        match load(layout, &rel) {
            Some(Ok(loaded)) => entries.extend(loaded.entries),
            Some(Err(e)) => warnings.push(format!("{}: {e}", layout.rel_file(&rel))),
            None => {}
        }
    }
    Ok(Extracted { entries, warnings })
}

/// Data 폴더 기준 상대 경로('/' 구분)의 파일을 읽어 추출한다. 번역 대상 파일 형식이 아니면 None
pub(super) fn load(layout: &WolfLayout, rel: &str) -> Option<R<Loaded>> {
    let path = data_path(layout, rel);
    let file = layout.rel_file(rel);
    let mut out = Out { file: &file, entries: Vec::new(), spans: Vec::new() };
    let name = rel.strip_prefix(&format!("{BASIC_DATA}/")).unwrap_or("");
    let is = |n: &str| name.eq_ignore_ascii_case(n);
    let db = DATABASES.iter().find(|(db, _)| is(&format!("{db}.dat")));
    let is_map = rel.starts_with(&format!("{MAP_DATA}/"))
        && Path::new(rel).extension().is_some_and(|x| x.eq_ignore_ascii_case("mps"));

    let parsed = (|| -> R<Option<Parsed>> {
        Ok(Some(if is("Game.dat") {
            let g = game_dat::parse(&read(&path)?)?;
            extract_game_dat(&g, &mut out);
            Parsed::Game(g)
        } else if is("CommonEvent.dat") {
            let c = common::parse(&read(&path)?)?;
            extract_common(&c, &mut out);
            Parsed::Common(c)
        } else if let Some((_, label)) = db {
            let d = database::parse(&read(&path.with_extension("project"))?, &read(&path)?)?;
            extract_database(&d, label, &mut out);
            Parsed::Db(d)
        } else if is_map {
            let m = map::parse(&read(&path)?)?;
            extract_map(&m, &mut out);
            Parsed::Map(m)
        } else {
            return Ok(None);
        }))
    })();
    match parsed {
        Ok(Some(parsed)) => Some(Ok(Loaded { parsed, entries: out.entries, spans: out.spans })),
        Ok(None) => None,
        Err(e) => Some(Err(e)),
    }
}

fn data_path(layout: &WolfLayout, rel: &str) -> std::path::PathBuf {
    rel.split('/').fold(layout.data_dir(), |p, part| p.join(part))
}

fn read(path: &Path) -> R<Vec<u8>> {
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Data 폴더 기준 MapData 아래의 .mps 파일 (하위 폴더 포함, 이름순)
fn map_files(data_dir: &Path) -> Vec<String> {
    let mut files: Vec<String> = WalkDir::new(data_dir.join(MAP_DATA))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("mps")))
        .filter_map(|e| {
            let rel = e.path().strip_prefix(data_dir).ok()?;
            Some(rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"))
        })
        .collect();
    files.sort();
    files
}

/// 첫 줄이 "폴더/이름.png" 같은 파일 경로인지 (효과음 설정은 "파일\r\n볼륨\r\n피치" 형태라 첫 줄만 본다).
/// 폴더 구분자가 있으면 이름에 공백이 있어도 경로로 본다 ("BGM/long goodbye.ogg")
fn looks_like_file(s: &str) -> bool {
    let first = s.lines().next().unwrap_or("").trim();
    (first.contains('/') || !first.contains(' '))
        && first.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && FILE_EXTS.iter().any(|x| x.eq_ignore_ascii_case(ext)))
}

/// 제어 코드(`\cself[1]`, `\f[\cself[18]]`, `\>` …)와 정렬 태그(`<C>` …)를 빼고 글자나 숫자가 남는지.
/// 코드만 있는 문자열은 번역할 내용이 없으므로 추출하지 않는다
pub(crate) fn has_text(s: &str) -> bool {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let mut name = false;
                while chars.next_if(char::is_ascii_alphabetic).is_some() {
                    name = true;
                }
                if !name {
                    chars.next(); // 기호 한 글자 코드 (\> \- \\ …)
                } else if chars.next_if_eq(&'[').is_some() {
                    let mut depth = 1;
                    while depth > 0 {
                        match chars.next() {
                            Some('[') => depth += 1,
                            Some(']') => depth -= 1,
                            Some(_) => {}
                            None => break,
                        }
                    }
                }
            }
            '<' if chars.peek().is_some_and(char::is_ascii_uppercase) => {
                let tag: String = chars.clone().take_while(|&c| c != '>').collect();
                if tag.len() <= 8 && tag.chars().all(|c| c.is_ascii_uppercase()) {
                    chars.nth(tag.len()); // 태그 이름과 '>'
                } else {
                    return true;
                }
            }
            c if c.is_alphanumeric() => return true,
            _ => {}
        }
    }
    false
}

/// 파일 하나 단위의 추출 결과 모음
struct Out<'a> {
    file: &'a str,
    entries: Vec<Entry>,
    spans: Vec<Range<usize>>,
}

impl Out<'_> {
    fn group_id(&self, anchor: &str) -> String {
        format!("{}#{}", self.file, anchor)
    }

    fn push(&mut self, s: &WStr, path: String, kind: Kind, group: &str, label: &str, context: Option<&str>) {
        if !has_text(&s.text) {
            return;
        }
        if let Some(entry) = Entry::text(self.file, path, kind, group, label, context, &s.text) {
            self.entries.push(entry);
            self.spans.push(s.span.clone());
        }
    }
}

fn extract_game_dat(g: &game_dat::GameDat, out: &mut Out) {
    let group = out.group_id("/");
    for (key, label, s) in &g.strings {
        out.push(s, format!("/{key}"), Kind::Term, &group, "Game.dat", Some(label));
    }
}

fn extract_database(db: &database::Database, db_label: &str, out: &mut Out) {
    for (t, ty) in db.types.iter().enumerate() {
        for (d, record) in ty.data.iter().enumerate() {
            let anchor = format!("/types/{t}/data/{d}");
            let group = out.group_id(&anchor);
            let label = format!("{db_label} · {t} {} · #{d} {}", ty.name, record.name).trim_end().to_string();
            for (f, value) in record.values.iter().enumerate() {
                let Some(value) = value.as_ref().filter(|v| !looks_like_file(&v.text)) else { continue };
                let field = ty.fields.get(f).map(String::as_str);
                out.push(value, format!("{anchor}/{f}"), Kind::DbValue, &group, &label, field);
            }
        }
    }
}

fn extract_common(ce: &common::CommonEvents, out: &mut Out) {
    for (e, event) in ce.events.iter().enumerate() {
        let label = format!("CE{:03} {}", event.id, event.name.text);
        extract_commands(&event.commands, &format!("/events/{e}/list"), label.trim_end(), out);
    }
}

fn extract_map(m: &map::Map, out: &mut Out) {
    let stem = out.file.rsplit('/').next().unwrap_or(out.file).trim_end_matches(".mps").to_string();
    for (e, event) in m.events.iter().enumerate() {
        for (p, page) in event.pages.iter().enumerate() {
            let label = format!("{stem} · EV{:03} {} · P{}", event.id, event.name.text, p + 1);
            extract_commands(page, &format!("/events/{e}/pages/{p}/list"), &label, out);
        }
    }
}

/// 이벤트 커맨드 목록. 커맨드 하나가 그룹 하나다.
fn extract_commands(cmds: &[Command], base: &str, label: &str, out: &mut Out) {
    for (i, cmd) in cmds.iter().enumerate() {
        // (종류, 표시 이름, 추출할 문자열 인자 범위)
        let (kind, what, range) = match cmd.code {
            code::MESSAGE => (Kind::Message, "메시지", 0..1),
            code::CHOICES => (Kind::Choice, "선택지", 0..cmd.strings.len()),
            code::PICTURE if cmd.is_picture_text() => (Kind::PictureText, "문자열 그림", 0..1),
            code::SET_STRING => (Kind::StringArg, "문자열 조작", 0..1),
            code::STRING_CONDITION => (Kind::StringArg, "문자열 조건", 0..cmd.strings.len()),
            // 나머지 문자열은 이름으로 지정한 타입/데이터/필드 이름이라 번역하면 안 된다
            code::DATABASE => (Kind::StringArg, "DB 조작", 0..1),
            // 첫 문자열은 호출할 커먼 이벤트 이름
            code::COMMON_EVENT_BY_NAME => (Kind::StringArg, "커먼 이벤트 호출", 1..cmd.strings.len()),
            _ => continue,
        };
        let group = out.group_id(&format!("{base}/{i}"));
        let group_label = format!("{label} · #{i} {what}");
        for k in range {
            let Some(s) = cmd.strings.get(k) else { continue };
            if kind != Kind::Message && looks_like_file(&s.text) {
                continue;
            }
            out.push(s, format!("{base}/{i}/{k}"), kind, &group, &group_label, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{command, database, map};
    use super::*;

    #[test]
    fn file_like_strings() {
        assert!(looks_like_file("BGM/Battle01.mid"));
        assert!(looks_like_file("SystemFile/SE_Enter.ogg\r\n90\r\n100"));
        assert!(!looks_like_file("薬草"));
        assert!(!looks_like_file("Ver 1.0"));
        assert!(!looks_like_file("\\cself[8]"));
        assert!(looks_like_file("BGM/20_Long goodbye.ogg"));
        assert!(!looks_like_file("まず readme.txt を読んで"));
    }

    #[test]
    fn code_only_strings() {
        for s in ["\\s[10]", "\\f[\\cself[18]]\\space[0]<C>\\cself[5]", "--------", "\\>\\cself[9]\r\n", "\\c[2]\\E", "<GAUGE>"] {
            assert!(!has_text(s), "{s}");
        }
        for s in ["\\c[0]<C>\\f[50]タポリ", "Now loading...", "\\cself[2]上がった！", "起動中<GAUGE>", "Lv", "\\f[12]速度 X"] {
            assert!(has_text(s), "{s}");
        }
    }

    #[test]
    fn extracts_map_and_database() {
        let m = map::parse(&map::tests::sample(&["やあ", ""])).unwrap();
        let mut out = Out { file: "Data/MapData/Town.mps", entries: Vec::new(), spans: Vec::new() };
        extract_map(&m, &mut out);
        let entries = out.entries;
        let got: Vec<_> = entries.iter().map(|e| (e.id.as_str(), e.group_label.as_str(), e.original.as_str())).collect();
        assert_eq!(got, [("Data/MapData/Town.mps#/events/0/pages/0/list/0/0", "Town · EV003 村人 · P1 · #0 메시지", "やあ")]);

        let (project, dat) = database::tests::sample();
        let db = database::parse(&project, &dat).unwrap();
        let mut out = Out { file: "Data/BasicData/DataBase.dat", entries: Vec::new(), spans: Vec::new() };
        extract_database(&db, "유저 DB", &mut out);
        let entries = out.entries;
        let got: Vec<_> = entries.iter().map(|e| (e.path.as_str(), e.context.as_deref(), e.original.as_str())).collect();
        assert_eq!(
            got,
            [
                ("/types/0/data/0/0", Some("名前"), "薬草"),
                ("/types/0/data/0/2", Some("説明"), "HPを回復"),
                ("/types/0/data/1/0", Some("名前"), "毒消し"),
            ]
        );
        assert_eq!(entries[0].group_label, "유저 DB · 0 アイテム · #0 薬草");
        assert_ne!(entries[1].group, entries[2].group);
    }

    #[test]
    fn command_rules() {
        let mut w = super::super::reader::tests::Writer::default();
        w.u32(4);
        command::tests::write(&mut w, code::DATABASE, &[], &["値", "アイテム", "", "名前"], false);
        command::tests::write(&mut w, code::COMMON_EVENT_BY_NAME, &[], &["X[共]処理", "引数"], false);
        command::tests::write(&mut w, code::SET_STRING, &[], &["Picture/a.png"], false);
        command::tests::write(&mut w, code::CHOICES, &[], &["はい", "いいえ"], false);
        let cmds = command::read_list(&mut super::super::reader::Reader::new(&w.0, 0, true), false).unwrap();
        let mut out = Out { file: "f", entries: Vec::new(), spans: Vec::new() };
        extract_commands(&cmds, "/l", "CE", &mut out);
        let got: Vec<_> = out.entries.iter().map(|e| (e.path.as_str(), e.kind, e.original.as_str())).collect();
        assert_eq!(
            got,
            [
                ("/l/0/0", Kind::StringArg, "値"),
                ("/l/1/1", Kind::StringArg, "引数"),
                ("/l/3/0", Kind::Choice, "はい"),
                ("/l/3/1", Kind::Choice, "いいえ"),
            ]
        );
    }

    /// 로컬의 실제 게임으로 확인한다. 게임 폴더를 WOLF_GAME 환경 변수로 지정해서 실행:
    /// `WOLF_GAME=<경로> cargo test --lib real_game -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_game() {
        let root = std::env::var("WOLF_GAME").expect("WOLF_GAME 환경 변수가 필요합니다");
        let layout = super::super::detect(Path::new(&root)).unwrap().unwrap();
        let ex = extract_all(&layout).unwrap();
        println!("utf8: {}, warnings: {:#?}", layout.utf8, ex.warnings);
        let mut by_kind = std::collections::BTreeMap::new();
        let mut by_file = std::collections::BTreeMap::new();
        for e in &ex.entries {
            *by_kind.entry(format!("{:?}", e.kind)).or_insert(0) += 1;
            *by_file.entry(e.file.as_str()).or_insert(0) += 1;
        }
        println!("{} entries {by_kind:?}\n{by_file:#?}", ex.entries.len());
        // 분석용으로 항목 전체를 JSON으로 저장
        if let Ok(dump) = std::env::var("WOLF_DUMP") {
            fs::write(dump, serde_json::to_string(&ex.entries).unwrap()).unwrap();
        }
        let step = std::env::var("WOLF_STEP").ok().and_then(|s| s.parse().ok()).unwrap_or(97);
        for e in ex.entries.iter().step_by(step) {
            println!("{} | {} | {:?} | {:?}", e.id, e.group_label, e.context, e.original);
        }
        assert!(ex.warnings.is_empty());
    }
}
