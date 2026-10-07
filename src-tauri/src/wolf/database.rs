//! 데이터베이스 (BasicData의 DataBase / CDataBase / SysDatabase, 각각 .project + .dat)
//!
//! .project에는 타입·필드·데이터 이름 등 구조가, .dat에는 값이 들어 있다. 값은 .dat에 있으므로
//! 번역 대상 문자열의 범위는 .dat 기준이다.
//! .dat: `[0x00][매직 9바이트][버전: u8]` 뒤에 본문, 버전 0xC4면 본문이 LZ4로 압축돼 있다.
//! .project는 헤더 없이 바로 본문이고 문자 인코딩은 .dat를 따른다.

use super::common::check_header;
use super::reader::{Buffer, Reader, WStr, R};

const DAT_MAGIC: [u8; 9] = [0x57, 0, 0, 0x4F, 0x4C, 0, 0x46, 0x4D, 0];
const DAT_MAGIC_UTF8_AT: usize = 5;
const HEADER: usize = 11;
const TYPE_SEPARATOR: [u8; 4] = [0xFE, 0xFF, 0xFF, 0xFF];
/// 필드 정보 값이 이 이상이면 문자열 필드 (값 - 2000이 문자열 목록의 인덱스). 그 밖에는 정수 필드
const STRING_START: u32 = 2000;

pub struct Database {
    /// .dat 내용
    pub buf: Buffer,
    pub types: Vec<Type>,
}

pub struct Type {
    pub name: String,
    pub fields: Vec<String>,
    pub data: Vec<Record>,
}

pub struct Record {
    pub name: String,
    /// 필드 순서대로의 문자열 값. 문자열 필드가 아니면 None
    pub values: Vec<Option<WStr>>,
}

struct ProjectType {
    name: String,
    fields: Vec<String>,
    data: Vec<String>,
}

/// .dat 헤더만 확인하고 UTF-8 여부를 돌려준다
pub fn check_dat(dat: &[u8]) -> R<bool> {
    check_header(dat, &DAT_MAGIC, DAT_MAGIC_UTF8_AT, "DB 파일")
}

pub fn parse(project: &[u8], dat: &[u8]) -> R<Database> {
    let utf8 = check_dat(dat)?;
    let version = dat[HEADER - 1];
    let buf = Buffer::load(dat, utf8, (version == 0xC4).then_some(HEADER))?;
    let ptypes = parse_project(project, utf8).map_err(|e| format!(".project: {e}"))?;

    let mut r = Reader::new(&buf.data, HEADER, utf8);
    let count = r.u32()? as usize;
    if count != ptypes.len() {
        return Err(format!(".project와 .dat의 타입 수가 다릅니다 ({} / {count})", ptypes.len()));
    }
    let mut types = Vec::with_capacity(count);
    for (t, pt) in ptypes.into_iter().enumerate() {
        types.push(read_type(&mut r, pt).map_err(|e| format!("타입 #{t}: {e}"))?);
    }
    if r.u8()? != version {
        return Err("DB 종료 표시가 잘못됐습니다".into());
    }
    if !r.is_eof() {
        return Err("DB 끝 뒤에 데이터가 더 있습니다".into());
    }
    Ok(Database { buf, types })
}

fn parse_project(raw: &[u8], utf8: bool) -> R<Vec<ProjectType>> {
    let mut r = Reader::new(raw, 0, utf8);
    let count = r.count(4)?;
    let mut types = Vec::with_capacity(count);
    for t in 0..count {
        types.push(read_project_type(&mut r).map_err(|e| format!("타입 #{t}: {e}"))?);
    }
    if !r.is_eof() {
        return Err("끝 뒤에 데이터가 더 있습니다".into());
    }
    Ok(types)
}

fn read_project_type(r: &mut Reader) -> R<ProjectType> {
    let name = r.string()?.text;
    let fields = (0..r.count(4)?).map(|_| r.string().map(|s| s.text)).collect::<R<Vec<_>>>()?;
    let data = (0..r.count(4)?).map(|_| r.string().map(|s| s.text)).collect::<R<Vec<_>>>()?;
    r.string()?; // 설명
    let n = r.count(1)?;
    r.skip(n)?; // 필드 종류
    for _ in 0..r.count(4)? {
        r.string()?;
    }
    for _ in 0..r.count(4)? {
        for _ in 0..r.count(4)? {
            r.string()?;
        }
    }
    for _ in 0..r.count(4)? {
        let n = r.count(4)?;
        r.skip(n * 4)?;
    }
    let n = r.count(4)?;
    r.skip(n * 4)?; // 기본값
    Ok(ProjectType { name, fields, data })
}

fn read_type(r: &mut Reader, pt: ProjectType) -> R<Type> {
    r.expect(&TYPE_SEPARATOR, "타입 구분 표시")?;
    let flag = r.u32()?;
    let field_count = r.u32()? as usize;
    if flag == 0x0001_D4C0 {
        r.string()?;
    }
    if field_count > pt.fields.len() {
        return Err(format!("필드 수가 .project보다 많습니다 ({field_count} / {})", pt.fields.len()));
    }
    let infos = (0..field_count).map(|_| r.u32()).collect::<R<Vec<_>>>()?;
    let data_count = r.u32()? as usize;
    // .project의 데이터 수가 더 많으면 .dat 기준으로 자른다
    let names = pt.data.into_iter().take(data_count);

    let str_count = infos.iter().filter(|&&i| i >= STRING_START).count();
    let int_count = field_count - str_count;
    let mut data = Vec::new();
    for name in names {
        r.skip(int_count * 4)?;
        let strings = (0..str_count).map(|_| r.string()).collect::<R<Vec<_>>>()?;
        let values = infos
            .iter()
            .map(|&info| info.checked_sub(STRING_START).and_then(|i| strings.get(i as usize).cloned()))
            .collect();
        data.push(Record { name, values });
    }
    Ok(Type { name: pt.name, fields: pt.fields, data })
}

#[cfg(test)]
pub mod tests {
    use super::super::reader::tests::Writer;
    use super::*;

    /// 타입 하나(필드: 이름=문자열, 값=정수, 설명=문자열), 데이터 둘
    pub fn sample() -> (Vec<u8>, Vec<u8>) {
        let mut p = Writer::default();
        p.u32(1).str("アイテム").u32(3).str("名前").str("価格").str("説明");
        p.u32(2).str("薬草").str("毒消し").str("");
        p.u32(3).raw(&[0, 0, 0]).u32(0).u32(0).u32(0).u32(0);

        let mut b = Writer::default();
        b.u32(1).raw(&TYPE_SEPARATOR).u32(0).u32(3).u32(2000).u32(1000).u32(2001);
        b.u32(2);
        b.u32(10).str("薬草").str("HPを回復");
        b.u32(20).str("毒消し").str("");
        b.u8(0xC4);
        let packed = lz4_flex::block::compress(&b.0);
        let mut d = Writer::default();
        d.u8(0).raw(&DAT_MAGIC).u8(0xC4);
        d.0[1 + DAT_MAGIC_UTF8_AT] = 0x55;
        d.u32(b.0.len() as u32).u32(packed.len() as u32).raw(&packed);
        (p.0, d.0)
    }

    #[test]
    fn parses_database() {
        let (project, dat) = sample();
        let db = parse(&project, &dat).unwrap();
        assert!(db.buf.utf8);
        let t = &db.types[0];
        assert_eq!((t.name.as_str(), t.fields.len(), t.data.len()), ("アイテム", 3, 2));
        let values: Vec<_> = t.data[0].values.iter().map(|v| v.as_ref().map(|s| s.text.as_str())).collect();
        assert_eq!(values, [Some("薬草"), None, Some("HPを回復")]);
        assert_eq!(t.data[1].name, "毒消し");
    }
}
