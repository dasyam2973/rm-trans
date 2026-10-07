//! 게임 리소스(이미지/오디오/동영상/폰트) 목록과 암호화 키를 찾는다.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::store::WORK_DIR;

use super::crypto::{self, Key, Scheme};
use super::detect::GameLayout;
use super::plugins_js;

/// web 폴더 아래에서 리소스를 찾을 폴더
const ASSET_DIRS: &[&str] = &["img", "audio", "movies", "fonts", "effects", "icon"];

/// 헤더 판별과 PNG 키 복구에 필요한 앞부분 (가장 긴 헤더 32 + PNG 16)
const PREFIX_LEN: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    Image,
    Audio,
    Video,
    Font,
}

impl AssetKind {
    pub(crate) fn of(ext: &str) -> Option<AssetKind> {
        Some(match ext {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" => AssetKind::Image,
            "ogg" | "m4a" | "mp3" | "wav" => AssetKind::Audio,
            "webm" | "mp4" | "ogv" => AssetKind::Video,
            "ttf" | "otf" | "woff" | "woff2" => AssetKind::Font,
            _ => return None,
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFile {
    /// 루트 기준 상대 경로, '/' 구분
    pub path: String,
    /// 복호화했을 때의 경로 (암호화 확장자를 원래 확장자로). 평문이면 path와 같다
    pub plain_path: String,
    pub kind: AssetKind,
    /// 암호화 확장자인지
    pub encrypted: bool,
    /// 헤더로 판별한 방식. 암호화 확장자인데 None이면 모르는 방식
    pub scheme: Option<Scheme>,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeySource {
    /// System.json의 encryptionKey에서 만듦
    System,
    /// 암호화된 PNG에서 복구함
    Recovered,
    /// 찾지 못함 (사용자 입력 필요)
    None,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemeInfo {
    pub scheme: Scheme,
    pub file_count: usize,
    /// 실제 XOR 키 (hex)
    pub key: Option<String>,
    pub key_source: KeySource,
    /// System.json 키와 PNG에서 복구한 키가 다름 (복구한 키를 씀)
    pub key_mismatch: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetScan {
    /// 리소스 폴더의 루트 기준 상대 경로 ("www" 또는 ""). 표시용
    pub web_dir: String,
    /// System.json의 encryptionKey 원문
    pub system_key: Option<String>,
    pub schemes: Vec<SchemeInfo>,
    /// 켜져 있는 암호화 관련 플러그인 이름 (방식을 모를 때 참고용)
    pub crypto_plugins: Vec<String>,
    pub files: Vec<AssetFile>,
    /// 번역 이미지가 등록된 리소스의 plain_path
    pub replacements: Vec<String>,
    pub warnings: Vec<String>,
}

/// 방식별 실제 XOR 키 (프론트엔드에서 hex로 주고받는다)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemeKey {
    pub scheme: Scheme,
    pub key: String,
}

pub struct KeyRing(Vec<(Scheme, Key)>);

impl KeyRing {
    pub fn new(keys: &[SchemeKey]) -> Result<KeyRing> {
        keys.iter().map(|k| Ok((k.scheme, Key::from_hex(&k.key)?))).collect::<Result<_>>().map(KeyRing)
    }

    fn get(&self, scheme: Scheme) -> Option<&Key> {
        self.0.iter().find(|(s, _)| *s == scheme).map(|(_, k)| k)
    }
}

pub fn scan(layout: &GameLayout) -> Result<AssetScan> {
    let web_rel = layout
        .web_rel()
        .ok_or_else(|| Error::msg("data 폴더 자체를 열어서 리소스 폴더를 찾을 수 없습니다. 게임 폴더를 열어 주세요."))?;
    let web = join_rel(&layout.root, web_rel);
    let mut warnings = Vec::new();

    let system_key = read_system_key(&layout.data_dir().join("System.json"));
    let mut files = Vec::new();
    // 방식별 파일 수와 키 복구용 PNG 앞부분
    let mut found: BTreeMap<u8, (Scheme, usize, Option<Vec<u8>>)> = BTreeMap::new();

    for dir in ASSET_DIRS {
        let walker = WalkDir::new(web.join(dir)).into_iter().filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'));
        for entry in walker {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(rel) = rel_path(&layout.root, entry.path()) else { continue };
            let ext = ext_of(&rel);
            let plain_ext = crypto::decrypted_ext(&ext);
            let Some(kind) = AssetKind::of(plain_ext.unwrap_or(&ext)) else { continue };

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let (scheme, plain_path) = match plain_ext {
                Some(plain) => {
                    let prefix = read_prefix(entry.path()).map_err(|e| Error::io(entry.path(), e))?;
                    let scheme = Scheme::detect(&prefix);
                    if let Some(s) = scheme {
                        let slot = found.entry(s as u8).or_insert((s, 0, None));
                        slot.1 += 1;
                        if slot.2.is_none() && plain == "png" {
                            slot.2 = Some(prefix);
                        }
                    }
                    (scheme, with_ext(&rel, &ext, plain))
                }
                None => (None, rel.clone()),
            };
            files.push(AssetFile { encrypted: plain_ext.is_some(), plain_path, path: rel, kind, scheme, size });
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let unknown = files.iter().filter(|f| f.encrypted && f.scheme.is_none()).count();
    if unknown > 0 {
        warnings.push(format!("암호화 방식을 알 수 없는 파일이 {unknown}개 있습니다."));
    }

    let schemes = found
        .into_values()
        .map(|(scheme, file_count, png)| {
            let derived = system_key.as_deref().and_then(|k| scheme.derive_key(k).ok());
            let recovered = png.and_then(|p| scheme.recover_key(&p));
            let key_mismatch = matches!((&derived, &recovered), (Some(d), Some(r)) if d != r);
            let (key, key_source) = match (recovered, derived) {
                (Some(r), Some(_)) if key_mismatch => (Some(r), KeySource::Recovered),
                (_, Some(d)) => (Some(d), KeySource::System),
                (Some(r), None) => (Some(r), KeySource::Recovered),
                (None, None) => (None, KeySource::None),
            };
            if key_mismatch {
                warnings.push(format!("System.json의 암호화 키가 실제 파일과 맞지 않아 이미지에서 복구한 키를 사용합니다 ({scheme:?})."));
            }
            SchemeInfo { scheme, file_count, key: key.map(|k| k.to_hex()), key_source, key_mismatch }
        })
        .collect();

    Ok(AssetScan {
        web_dir: web_rel.to_string(),
        system_key,
        schemes,
        crypto_plugins: layout.plugins_rel().map(|p| crypto_plugins(&join_rel(&layout.root, &p))).unwrap_or_default(),
        files,
        replacements: list_replacements(&layout.root),
        warnings,
    })
}

// ── 번역 이미지 ──
// 번역 이미지는 <루트>/.rmtrans/assets/<plain_path>에 평문 PNG로 둔다. 이 폴더가 곧 등록 목록이다.
// 내보낼 때 원본 리소스와 같은 경로(암호화됐다면 같은 방식과 키로 재암호화)에 쓴다.

fn replacement_dir(root: &Path) -> PathBuf {
    root.join(WORK_DIR).join("assets")
}

/// 등록된 번역 이미지의 plain_path 목록
pub fn list_replacements(root: &Path) -> Vec<String> {
    let dir = replacement_dir(root);
    let mut list: Vec<String> = WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| rel_path(&dir, e.path()))
        .collect();
    list.sort();
    list
}

/// 리소스(path)의 번역 이미지로 source 이미지를 복사해 등록한다. 등록된 plain_path를 돌려준다.
/// 게임은 확장자까지 포함한 이름으로 이미지를 찾으므로 원본과 같은 형식이어야 한다.
/// PNG는 암호화된 원본도 되고(내보낼 때 재암호화), JPG는 평문 원본만 된다 (WOLF RPG 등).
pub fn set_replacement(root: &Path, path: &str, source: &Path) -> Result<String> {
    let plain_rel = plain_path(path);
    let ext = ext_of(&plain_rel);
    let replaceable = ext == "png" || (matches!(ext.as_str(), "jpg" | "jpeg") && plain_rel == path);
    if !replaceable {
        return Err(Error::msg("번역 이미지는 PNG 리소스와 암호화되지 않은 JPG 리소스에만 등록할 수 있습니다."));
    }
    let bytes = fs::read(source).map_err(|e| Error::io(source, e))?;
    if !crypto::looks_like(&bytes, &ext) {
        return Err(Error::msg(format!("{} 파일이 아닙니다: {}", ext.to_uppercase(), source.display())));
    }
    let dest = safe_join(&replacement_dir(root), &plain_rel)?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    fs::write(&dest, bytes).map_err(|e| Error::io(&dest, e))?;
    Ok(plain_rel)
}

pub fn remove_replacement(root: &Path, path: &str) -> Result<()> {
    let file = safe_join(&replacement_dir(root), &plain_path(path))?;
    match fs::remove_file(&file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Error::io(&file, e)),
        _ => Ok(()),
    }
}

pub fn read_replacement(root: &Path, path: &str) -> Result<Vec<u8>> {
    let file = safe_join(&replacement_dir(root), &plain_path(path))?;
    fs::read(&file).map_err(|e| Error::io(&file, e))
}

/// 번역 이미지(plain_rel)를 내보낼 (루트 기준 경로, 내용) 목록으로 만든다.
/// 같은 이름의 평문/암호화 원본이 있는 경로마다 하나씩. 암호화된 원본은 그 파일에서 키를 복구해 같은 방식으로 재암호화한다.
pub fn build_replacement(root: &Path, plain_rel: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let src = safe_join(&replacement_dir(root), plain_rel)?;
    let plain = fs::read(&src).map_err(|e| Error::io(&src, e))?;
    let ext = ext_of(plain_rel);
    let stem = &plain_rel[..plain_rel.len() - ext.len()];

    let mut out = Vec::new();
    if safe_join(root, plain_rel)?.is_file() {
        out.push((plain_rel.to_string(), plain.clone()));
    }
    for enc in crypto::encrypted_exts(&ext) {
        let target = format!("{stem}{enc}");
        let path = safe_join(root, &target)?;
        if !path.is_file() {
            continue;
        }
        let original = read_prefix(&path).map_err(|e| Error::io(&path, e))?;
        let fail = |why: &str| Error::msg(format!("{target}: {why}"));
        let scheme = Scheme::detect(&original).ok_or_else(|| fail("알 수 없는 암호화 방식입니다."))?;
        let key = scheme.recover_key(&original).ok_or_else(|| fail("암호화 키를 복구할 수 없습니다."))?;
        out.push((target, scheme.encrypt(&plain, &key)?));
    }
    if out.is_empty() {
        return Err(Error::msg(format!("{plain_rel}: 원본 리소스가 없습니다.")));
    }
    Ok(out)
}

/// 리소스 하나를 읽어 복호화한다. 평문 파일은 그대로 돌려준다.
pub fn read_plain(root: &Path, rel: &str, keys: &KeyRing) -> Result<Vec<u8>> {
    let path = safe_join(root, rel)?;
    let bytes = fs::read(&path).map_err(|e| Error::io(&path, e))?;
    let Some(plain_ext) = crypto::decrypted_ext(&ext_of(rel)) else { return Ok(bytes) };
    let scheme = Scheme::detect(&bytes).ok_or_else(|| Error::msg(format!("{rel}: 알 수 없는 암호화 방식입니다.")))?;
    let key = keys.get(scheme).ok_or_else(|| Error::msg(format!("{rel}: 암호화 키가 없습니다.")))?;
    let plain = scheme.decrypt(&bytes, key).map_err(|e| Error::msg(format!("{rel}: {e}")))?;
    if !crypto::looks_like(&plain, plain_ext) {
        return Err(Error::msg(format!("{rel}: 복호화 결과가 올바르지 않습니다. 암호화 키를 확인해 주세요.")));
    }
    Ok(plain)
}

/// '/' 구분 상대 경로를 구성 요소별로 붙인다 (canonicalize된 Windows 경로는 '/'를 구분자로 인식하지 않음)
pub fn join_rel(base: &Path, rel: &str) -> PathBuf {
    rel.split('/').filter(|p| !p.is_empty()).fold(base.to_path_buf(), |p, part| p.join(part))
}

/// 프론트엔드에서 받은 상대 경로가 루트 밖을 가리키지 않는지 확인하고 붙인다
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    if rel.split('/').any(|p| p == ".." || p.contains(['\\', ':'])) {
        return Err(Error::msg(format!("잘못된 경로입니다: {rel}")));
    }
    Ok(join_rel(base, rel))
}

pub(crate) fn rel_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<_> = rel.components().map(|c| c.as_os_str().to_str()).collect::<Option<_>>()?;
    Some(parts.join("/"))
}

/// 복호화했을 때의 경로 (암호화 확장자를 원래 확장자로 바꾼다)
pub fn plain_path(rel: &str) -> String {
    let ext = ext_of(rel);
    match crypto::decrypted_ext(&ext) {
        Some(plain) => with_ext(rel, &ext, plain),
        None => rel.to_string(),
    }
}

fn with_ext(rel: &str, old: &str, new: &str) -> String {
    format!("{}{new}", &rel[..rel.len() - old.len()])
}

pub(crate) fn ext_of(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

fn read_prefix(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(PREFIX_LEN);
    File::open(path)?.take(PREFIX_LEN as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

fn read_system_key(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let key = json.get("encryptionKey")?.as_str()?.trim();
    (!key.is_empty()).then(|| key.to_string())
}

/// 켜져 있는 플러그인 중 이름에 Crypt가 들어간 것 (Decrypter, Encrypter 등)
fn crypto_plugins(plugins_js: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(plugins_js) else { return Vec::new() };
    let Some(range) = plugins_js::json_range(&text) else { return Vec::new() };
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str(&text[range]) else { return Vec::new() };
    list.iter()
        .filter(|p| p.get("status").and_then(|s| s.as_bool()).unwrap_or(false))
        .filter_map(|p| p.get("name")?.as_str())
        .filter(|n| n.to_ascii_lowercase().contains("crypt"))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::rpgm::detect;

    pub const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR rest of image";

    /// www 배포본 형태의 테스트 게임. 내장 방식 PNG/OGG, Arthran 방식 PNG, 평문 PNG, 알 수 없는 방식 파일
    pub fn make_game(name: &str) -> (PathBuf, PathBuf) {
        let base = std::env::temp_dir().join(format!("rmtrans-assets-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let www = base.join("game/www");
        let sys_key = "00112233445566778899aabbccddeeff";
        fs::create_dir_all(www.join("data")).unwrap();
        fs::create_dir_all(www.join("img/pictures")).unwrap();
        fs::create_dir_all(www.join("audio/bgm")).unwrap();
        fs::create_dir_all(www.join("js")).unwrap();
        fs::write(www.join("data/System.json"), format!(r#"{{"encryptionKey":"{sys_key}"}}"#)).unwrap();
        fs::write(www.join("js/plugins.js"), "var $plugins =\n[\n{\"name\":\"Art_Decrypterator3000\",\"status\":true},{\"name\":\"OffCrypt\",\"status\":false}\n];\n").unwrap();

        let std_key = Scheme::Standard.derive_key(sys_key).unwrap();
        let art_key = Scheme::Arthran.derive_key(sys_key).unwrap();
        fs::write(www.join("img/pictures/a.rpgmvp"), Scheme::Standard.encrypt(PNG, &std_key).unwrap()).unwrap();
        fs::write(www.join("img/pictures/b.png_"), Scheme::Arthran.encrypt(PNG, &art_key).unwrap()).unwrap();
        fs::write(www.join("img/pictures/plain.png"), PNG).unwrap();
        fs::write(www.join("img/pictures/notes.txt"), "x").unwrap();
        fs::write(www.join("img/pictures/odd.rpgmvp"), b"XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX").unwrap();
        fs::write(www.join("audio/bgm/song.rpgmvo"), Scheme::Standard.encrypt(b"OggS audio", &std_key).unwrap()).unwrap();
        (base.clone(), base.join("game"))
    }

    pub fn keys_of(scan: &AssetScan) -> Vec<SchemeKey> {
        scan.schemes.iter().map(|s| SchemeKey { scheme: s.scheme, key: s.key.clone().unwrap() }).collect()
    }

    #[test]
    fn scans_and_reads() {
        let (base, game) = make_game("scan");
        let layout = detect::detect(&game).unwrap();
        let scan = scan(&layout).unwrap();

        assert_eq!(scan.web_dir, "www");
        assert_eq!(scan.crypto_plugins, ["Art_Decrypterator3000"]);
        let paths: Vec<_> = scan.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "www/audio/bgm/song.rpgmvo",
                "www/img/pictures/a.rpgmvp",
                "www/img/pictures/b.png_",
                "www/img/pictures/odd.rpgmvp",
                "www/img/pictures/plain.png",
            ]
        );
        let a = &scan.files[1];
        assert_eq!((a.plain_path.as_str(), a.kind, a.scheme), ("www/img/pictures/a.png", AssetKind::Image, Some(Scheme::Standard)));
        assert_eq!(scan.files[3].scheme, None);
        assert_eq!(scan.warnings.len(), 1); // 알 수 없는 방식 1개

        let schemes: Vec<_> = scan.schemes.iter().map(|s| (s.scheme, s.file_count, s.key_source, s.key_mismatch)).collect();
        assert_eq!(
            schemes,
            [(Scheme::Standard, 2, KeySource::System, false), (Scheme::Arthran, 1, KeySource::System, false)]
        );

        let keys = KeyRing::new(&keys_of(&scan)).unwrap();
        assert_eq!(read_plain(&game, "www/img/pictures/a.rpgmvp", &keys).unwrap(), PNG);
        assert_eq!(read_plain(&game, "www/img/pictures/b.png_", &keys).unwrap(), PNG);
        assert_eq!(read_plain(&game, "www/audio/bgm/song.rpgmvo", &keys).unwrap(), b"OggS audio");
        assert_eq!(read_plain(&game, "www/img/pictures/plain.png", &keys).unwrap(), PNG);
        assert!(read_plain(&game, "www/img/pictures/odd.rpgmvp", &keys).is_err());
        assert!(read_plain(&game, "www/../secret", &keys).is_err());

        // 틀린 키는 형식 검사에서 걸린다
        let wrong = KeyRing::new(&[SchemeKey { scheme: Scheme::Standard, key: "ff".repeat(16) }]).unwrap();
        assert!(read_plain(&game, "www/img/pictures/a.rpgmvp", &wrong).is_err());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn replacement_reencrypts_like_original() {
        let (base, game) = make_game("replace");
        let new_png: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR translated image";
        let src = base.join("translated.png");
        fs::write(&src, new_png).unwrap();

        // PNG가 아니거나 PNG 리소스가 아니면 거부
        assert!(set_replacement(&game, "www/img/pictures/a.rpgmvp", &base.join("game/www/data/System.json")).is_err());
        assert!(set_replacement(&game, "www/audio/bgm/song.rpgmvo", &src).is_err());

        for path in ["www/img/pictures/a.rpgmvp", "www/img/pictures/b.png_", "www/img/pictures/plain.png"] {
            set_replacement(&game, path, &src).unwrap();
        }
        assert_eq!(
            list_replacements(&game),
            ["www/img/pictures/a.png", "www/img/pictures/b.png", "www/img/pictures/plain.png"]
        );
        assert_eq!(read_replacement(&game, "www/img/pictures/b.png_").unwrap(), new_png);

        // 원본과 같은 방식으로 재암호화되어, 게임 쪽 키로 복호화하면 번역 이미지가 나온다
        let scan = scan(&detect::detect(&game).unwrap()).unwrap();
        assert_eq!(scan.replacements.len(), 3);
        let keys = KeyRing::new(&keys_of(&scan)).unwrap();
        for (plain, target) in [
            ("www/img/pictures/a.png", "www/img/pictures/a.rpgmvp"),
            ("www/img/pictures/b.png", "www/img/pictures/b.png_"),
            ("www/img/pictures/plain.png", "www/img/pictures/plain.png"),
        ] {
            let built = build_replacement(&game, plain).unwrap();
            assert_eq!(built.len(), 1);
            assert_eq!(built[0].0, target);
            let scheme = Scheme::detect(&built[0].1);
            let decoded = match scheme {
                Some(s) => s.decrypt(&built[0].1, keys.get(s).unwrap()).unwrap(),
                None => built[0].1.clone(),
            };
            assert_eq!(decoded, new_png);
        }

        remove_replacement(&game, "www/img/pictures/a.rpgmvp").unwrap();
        remove_replacement(&game, "www/img/pictures/a.rpgmvp").unwrap(); // 없어도 오류 아님
        assert_eq!(list_replacements(&game).len(), 2);

        // 원본이 사라진 번역 이미지
        fs::remove_file(game.join("www/img/pictures/plain.png")).unwrap();
        assert!(build_replacement(&game, "www/img/pictures/plain.png").is_err());
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn recovers_when_system_key_wrong() {
        let (base, game) = make_game("recover");
        fs::write(game.join("www/data/System.json"), r#"{"encryptionKey":"ffffffffffffffffffffffffffffffff"}"#).unwrap();
        let scan = scan(&detect::detect(&game).unwrap()).unwrap();
        assert!(scan.schemes.iter().all(|s| s.key_source == KeySource::Recovered && s.key_mismatch));
        let keys = KeyRing::new(&keys_of(&scan)).unwrap();
        assert_eq!(read_plain(&game, "www/img/pictures/b.png_", &keys).unwrap(), PNG);
        fs::remove_dir_all(&base).ok();
    }
}
