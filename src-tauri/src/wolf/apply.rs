//! 번역문을 WOLF RPG 데이터 파일에 적용한다.
//!
//! RPG Maker의 `rpgm/apply.rs`와 같은 방식이다. 파일을 다시 직렬화하지 않고, 추출할 때 기록한 문자열 범위
//! (`[u32 길이][바이트…][NUL]`)만 새 문자열로 바꾼다. 그래서 알 수 없는 영역도 원본 그대로 남는다.
//! 3.x 파일은 압축을 푼 본문을 고친 뒤 다시 LZ4로 압축하고, Game.dat는 크기·오프셋 필드를 고친다.

use std::collections::HashMap;

use super::extract::{load, Parsed};
use super::reader::{splice, R};
use super::WolfLayout;

pub struct Patched {
    pub bytes: Vec<u8>,
    pub applied: usize,
    /// 경로를 찾지 못했거나 인코딩할 수 없어서 건너뛴 아이템 경로 (이유가 붙을 수 있음)
    pub skipped: Vec<String>,
}

/// `file`(게임 루트 기준 상대 경로)을 원본 게임 폴더에서 읽어 `patches`(경로, 번역문)를 적용한다.
pub fn patch_file(layout: &WolfLayout, file: &str, patches: &[(String, String)]) -> R<Patched> {
    let rel = match layout.data_rel.as_str() {
        "" => Some(file),
        data => file.strip_prefix(data).and_then(|r| r.strip_prefix('/')),
    };
    let loaded = rel.and_then(|rel| load(layout, rel)).ok_or("WOLF RPG 데이터 파일이 아닙니다")??;
    let spans: HashMap<&str, _> = loaded.entries.iter().map(|e| e.path.as_str()).zip(&loaded.spans).collect();
    let buf = match &loaded.parsed {
        Parsed::Game(g) => &g.buf,
        Parsed::Db(d) => &d.buf,
        Parsed::Common(c) => &c.buf,
        Parsed::Map(m) => &m.buf,
    };

    let mut replacements = Vec::new();
    let mut skipped = Vec::new();
    for (path, text) in patches {
        match (spans.get(path.as_str()), buf.encode(text)) {
            (None, _) => skipped.push(path.clone()),
            (Some(_), None) => skipped.push(format!("{path} (Shift-JIS로 나타낼 수 없는 문자가 있음)")),
            (Some(&span), Some(bytes)) => replacements.push((span.clone(), bytes)),
        }
    }
    let applied = replacements.len();
    if let (Parsed::Game(g), true) = (&loaded.parsed, applied > 0) {
        g.check_size()?;
    }
    let mut data = splice(&buf.data, replacements);
    if let Parsed::Game(g) = &loaded.parsed {
        g.fix_sizes(&mut data);
    }
    Ok(Patched { bytes: buf.pack(&data), applied, skipped })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::super::reader::tests::Writer;
    use super::super::{detect, extract_all, game_dat, map, BASIC_DATA};
    use super::*;

    fn make_game(name: &str) -> (PathBuf, WolfLayout) {
        let base = std::env::temp_dir().join(format!("rmtrans-test-wolf-{name}-{}", std::process::id()));
        let basic = base.join("Data").join(BASIC_DATA);
        fs::create_dir_all(&basic).unwrap();
        fs::create_dir_all(base.join("Data/MapData")).unwrap();
        fs::write(base.join("Data/MapData/Town.mps"), map::tests::sample(&["やあ", "またね"])).unwrap();
        fs::write(basic.join("Game.dat"), sample_game_dat("勇者の旅")).unwrap();
        let layout = detect(&base).unwrap().unwrap();
        (base, layout)
    }

    /// 3.x Game.dat (문자열 10개 형식)
    fn sample_game_dat(title: &str) -> Vec<u8> {
        let mut w = Writer::default();
        w.u8(0).raw(&[0x57, 0, 0, 0x4F, 0x4C, 0, 0x46, 0x4D, 0x55]);
        w.u32(2).raw(&[1, 2]).u32(13).str(title).str("0000-0000").u32(0);
        for s in ["Font", "", "", "", "Chara.png", "", "Load.png", "Gauge.png", "起動中", ""] {
            w.str(s);
        }
        let size_at = w.0.len();
        w.u32(0).u32(7).u32(1).raw(&[9, 9]).u32(100).u32(200).raw(&[5, 6, 7]);
        let size = w.0.len() as u32 - 1;
        w.0[size_at..size_at + 4].copy_from_slice(&size.to_le_bytes());
        w.0
    }

    #[test]
    fn patches_compressed_map() {
        let (base, layout) = make_game("map");
        let id = "Data/MapData/Town.mps";
        let r = patch_file(
            &layout,
            id,
            &[("/events/0/pages/0/list/1/0".into(), "또 봐요!".into()), ("/events/9/pages/0/list/0/0".into(), "x".into())],
        )
        .unwrap();
        assert_eq!((r.applied, r.skipped.as_slice()), (1, ["/events/9/pages/0/list/0/0".to_string()].as_slice()));

        // 압축을 풀었을 때 바꾼 문자열 외에는 원본과 같다
        let before = map::parse(&fs::read(base.join(id)).unwrap()).unwrap();
        let after = map::parse(&r.bytes).unwrap();
        let old = before.events[0].pages[0][1].strings[0].span.clone();
        let expected = splice(&before.buf.data, vec![(old, before.buf.encode("또 봐요!").unwrap())]);
        assert_eq!(after.buf.data, expected);
        let texts: Vec<_> = after.events[0].pages[0].iter().map(|c| c.strings[0].text.as_str()).collect();
        assert_eq!(texts, ["やあ", "또 봐요!"]);
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn patches_game_dat_sizes() {
        let (base, layout) = make_game("gamedat");
        let file = "Data/BasicData/Game.dat";
        let src = fs::read(base.join(file)).unwrap();
        let r = patch_file(&layout, file, &[("/title".into(), "용사의 여행길".into())]).unwrap();
        assert_eq!(r.applied, 1);
        let g = game_dat::parse(&r.bytes).unwrap();
        assert_eq!(g.strings[0].2.text, "용사의 여행길");
        g.check_size().unwrap();
        let diff = r.bytes.len() as u32 - src.len() as u32;
        let u32_at = |at: usize| u32::from_le_bytes(r.bytes[at..at + 4].try_into().unwrap());
        assert_eq!((u32_at(g.offsets_at), u32_at(g.offsets_at + 4)), (100 + diff, 200 + diff));
        // 크기 필드 뒤의 나머지 바이트는 그대로
        assert_eq!(&r.bytes[g.offsets_at + 8..], &[5, 6, 7]);
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn shift_jis_cannot_hold_korean() {
        let buf = super::super::reader::Buffer { data: Vec::new(), utf8: false, packed_at: None };
        assert_eq!(buf.encode("剣").unwrap(), [3, 0, 0, 0, 0x8C, 0x95, 0]);
        assert!(buf.encode("검").is_none());
    }

    /// 실제 게임의 모든 파일에 (1) 원문 그대로, (2) 모든 아이템에 "★"를 붙여 적용해 본다.
    /// `WOLF_GAME=<경로> cargo test --lib real_game_roundtrip -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_game_roundtrip() {
        let root = std::env::var("WOLF_GAME").expect("WOLF_GAME 환경 변수가 필요합니다");
        let layout = detect(Path::new(&root)).unwrap().unwrap();
        let entries = extract_all(&layout).unwrap().entries;
        let mut by_file: std::collections::BTreeMap<&str, Vec<&crate::model::Entry>> = Default::default();
        for e in &entries {
            by_file.entry(e.file.as_str()).or_default().push(e);
        }
        for (file, list) in by_file {
            let rel = file.strip_prefix(&format!("{}/", layout.data_rel)).unwrap_or(file);
            let original = load(&layout, rel).unwrap().unwrap();
            let same: Vec<_> = list.iter().map(|e| (e.path.clone(), e.original.clone())).collect();
            let r = patch_file(&layout, file, &same).unwrap();
            assert_eq!((r.applied, r.skipped.len()), (list.len(), 0), "{file}");
            assert_eq!(unpacked(&original.parsed), unpacked_bytes(&original.parsed, &r.bytes), "{file}: 원문 그대로 적용");

            let starred: Vec<_> = list.iter().map(|e| (e.path.clone(), format!("{}★", e.original))).collect();
            let r = patch_file(&layout, file, &starred).unwrap();
            assert_eq!(r.applied, list.len(), "{file}: {:?}", r.skipped);
            // 다시 읽어서 모든 아이템이 바뀌었는지 확인
            let tmp = std::env::temp_dir().join(format!("rmtrans-roundtrip-{}", std::process::id()));
            let out = tmp.join(&layout.data_rel).join(rel);
            fs::create_dir_all(out.parent().unwrap()).unwrap();
            fs::write(&out, &r.bytes).unwrap();
            if rel.ends_with(".dat") && !rel.ends_with("Game.dat") && !rel.ends_with("CommonEvent.dat") {
                fs::copy(Path::new(&root).join(&layout.data_rel).join(rel).with_extension("project"), out.with_extension("project")).unwrap();
            }
            let tmp_layout = WolfLayout { root: tmp.clone(), ..layout.clone() };
            let reread = load(&tmp_layout, rel).unwrap().unwrap();
            let texts: Vec<_> = reread.entries.iter().map(|e| (e.path.as_str(), e.original.as_str())).collect();
            let expected: Vec<_> = starred.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
            assert_eq!(texts, expected, "{file}");
            fs::remove_dir_all(&tmp).ok();
            println!("ok {file}: {} items", list.len());
        }
    }

    fn unpacked(p: &Parsed) -> &[u8] {
        match p {
            Parsed::Game(g) => &g.buf.data,
            Parsed::Db(d) => &d.buf.data,
            Parsed::Common(c) => &c.buf.data,
            Parsed::Map(m) => &m.buf.data,
        }
    }

    /// 같은 종류로 압축을 푼 바이트
    fn unpacked_bytes(p: &Parsed, raw: &[u8]) -> Vec<u8> {
        let at = match p {
            Parsed::Game(g) => g.buf.packed_at,
            Parsed::Db(d) => d.buf.packed_at,
            Parsed::Common(c) => c.buf.packed_at,
            Parsed::Map(m) => m.buf.packed_at,
        };
        match at {
            Some(at) => super::super::reader::unpack(raw, at).unwrap(),
            None => raw.to_vec(),
        }
    }
}
