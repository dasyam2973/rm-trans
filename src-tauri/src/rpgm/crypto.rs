//! RPG Maker 리소스(이미지/오디오) 암호화 방식.
//!
//! 모든 방식의 뼈대는 같다: `[고정 헤더][키와 XOR된 앞부분][나머지는 원본 그대로]`.
//! 방식은 플러그인 유무가 아니라 파일마다 헤더로 판별한다 (한 게임 안에 섞여 있을 수 있음).

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// 복호화 결과 PNG의 처음 16바이트 (시그니처 + IHDR 청크 길이/타입). 키 복구에 쓴다.
const PNG_HEAD: [u8; 16] = [
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Scheme {
    /// MV/MZ 내장 암호화 (rpg_core.js `Decrypter`, rmmz_core.js `Utils.decryptArrayBuffer`)
    Standard,
    /// Arthran의 Encrypterator 3000 (Art_Decrypterator3000.js)
    Arthran,
}

pub const SCHEMES: [Scheme; 2] = [Scheme::Standard, Scheme::Arthran];

impl Scheme {
    pub fn header(self) -> &'static [u8] {
        match self {
            // "RPGMV" + 버전 000301
            Scheme::Standard => b"RPGMV\0\0\0\0\x03\x01\0\0\0\0\0",
            Scheme::Arthran => b"ART\0ENCRYPTER100FREE\0VERSION\0\0\0\0",
        }
    }

    /// 키 길이 = 헤더 뒤에서 XOR되는 바이트 수
    pub fn key_len(self) -> usize {
        match self {
            Scheme::Standard => 16,
            Scheme::Arthran => 32,
        }
    }

    /// 파일 앞부분의 헤더로 방식을 판별한다. 헤더가 없으면 평문(또는 모르는 방식)
    pub fn detect(bytes: &[u8]) -> Option<Scheme> {
        SCHEMES.into_iter().find(|s| bytes.starts_with(s.header()))
    }

    /// System.json `encryptionKey` 문자열에서 실제 XOR 키를 만든다.
    pub fn derive_key(self, src: &str) -> Result<Key> {
        match self {
            Scheme::Standard => {
                let key = Key::from_hex(src)?;
                if key.0.len() != 16 {
                    return Err(Error::msg(format!("암호화 키는 32자리 hex여야 합니다: {src}")));
                }
                Ok(key)
            }
            // md5(키 문자열) 16바이트 + 그걸 뒤집은 16바이트 (SparkMD5.hash는 UTF-8로 해시)
            Scheme::Arthran => Ok(Key::mirrored(&Md5::digest(src.as_bytes()))),
        }
    }

    /// 이 방식으로 암호화된 PNG에서 키를 복구한다. 평문 앞 16바이트가 고정이라 가능하다.
    /// Arthran은 키 뒤 절반이 앞 절반을 뒤집은 것이라 16바이트로 전체 키를 얻는다.
    pub fn recover_key(self, encrypted_png: &[u8]) -> Option<Key> {
        let body = encrypted_png.strip_prefix(self.header())?;
        let head: Vec<u8> = body.get(..16)?.iter().zip(PNG_HEAD).map(|(b, p)| b ^ p).collect();
        Some(match self {
            Scheme::Standard => Key(head),
            Scheme::Arthran => Key::mirrored(&head),
        })
    }

    pub fn decrypt(self, bytes: &[u8], key: &Key) -> Result<Vec<u8>> {
        self.check_key(key)?;
        let body = bytes
            .strip_prefix(self.header())
            .ok_or_else(|| Error::msg("암호화 헤더가 맞지 않습니다."))?;
        let mut out = body.to_vec();
        key.xor(&mut out);
        Ok(out)
    }

    pub fn encrypt(self, plain: &[u8], key: &Key) -> Result<Vec<u8>> {
        self.check_key(key)?;
        let header = self.header();
        let mut out = Vec::with_capacity(header.len() + plain.len());
        out.extend_from_slice(header);
        out.extend_from_slice(plain);
        key.xor(&mut out[header.len()..]);
        Ok(out)
    }

    fn check_key(self, key: &Key) -> Result<()> {
        if key.0.len() == self.key_len() {
            Ok(())
        } else {
            Err(Error::msg(format!("키 길이가 맞지 않습니다: {}바이트 (필요: {}바이트)", key.0.len(), self.key_len())))
        }
    }
}

/// 방식별 실제 XOR 키 바이트
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key(Vec<u8>);

impl Key {
    pub fn from_hex(s: &str) -> Result<Key> {
        let s = s.trim();
        let invalid = || Error::msg(format!("올바른 hex 키가 아닙니다: {s}"));
        if s.is_empty() || s.len() % 2 != 0 || !s.is_ascii() {
            return Err(invalid());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| invalid()))
            .collect::<Result<_>>()
            .map(Key)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// 앞 절반 + 그걸 뒤집은 뒤 절반
    fn mirrored(half: &[u8]) -> Key {
        Key(half.iter().chain(half.iter().rev()).copied().collect())
    }

    /// 앞부분을 키와 XOR한다. 데이터가 키보다 짧으면 있는 만큼만
    fn xor(&self, data: &mut [u8]) {
        for (b, k) in data.iter_mut().zip(&self.0) {
            *b ^= k;
        }
    }
}

/// 암호화 파일 확장자 → 원래 확장자 (MV: .rpgmvp 등, MZ: .png_ 등)
pub fn decrypted_ext(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "rpgmvp" | "png_" => Some("png"),
        "rpgmvo" | "ogg_" => Some("ogg"),
        "rpgmvm" | "m4a_" => Some("m4a"),
        _ => None,
    }
}

/// 원래 확장자 → 가능한 암호화 확장자 (MZ, MV 순)
pub fn encrypted_exts(plain: &str) -> &'static [&'static str] {
    match plain {
        "png" => &["png_", "rpgmvp"],
        "ogg" => &["ogg_", "rpgmvo"],
        "m4a" => &["m4a_", "rpgmvm"],
        _ => &[],
    }
}

/// 복호화 결과가 기대한 형식으로 시작하는지 확인한다 (키가 틀렸는지 판별용)
pub fn looks_like(plain: &[u8], ext: &str) -> bool {
    match ext {
        "png" => plain.starts_with(&PNG_HEAD[..8]),
        "ogg" => plain.starts_with(b"OggS"),
        "m4a" => plain.get(4..8) == Some(b"ftyp"),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        Key::from_hex(s).unwrap().0
    }

    /// 실제 게임(Art_Decrypterator3000 사용)의 img/battlebacks1/Carpet4_mvt.png_ 앞 64바이트
    fn arthran_sample() -> Vec<u8> {
        let mut v = Scheme::Arthran.header().to_vec();
        v.extend(hex("5d39829de7c908be33449a8e57e3620d5f26a8f6839a46d7bc11c3eada67a13b"));
        v
    }
    const ARTHRAN_SRC_KEY: &str = "0bbaa902299f82666958ccbb322684e3";
    const ARTHRAN_PLAIN: &str = "89504e470d0a1a0a0000000d49484452000003e8000002e40803000000abc8ef";

    #[test]
    fn header_lengths() {
        assert_eq!(Scheme::Standard.header().len(), 16);
        assert_eq!(Scheme::Arthran.header().len(), 32);
    }

    #[test]
    fn arthran_real_file() {
        let key = Scheme::Arthran.derive_key(ARTHRAN_SRC_KEY).unwrap();
        assert_eq!(key.to_hex(), "d469ccdaeac312b433449a831eab265f5f26ab1e839a4433b412c3eadacc69d4");

        let sample = arthran_sample();
        assert_eq!(Scheme::detect(&sample), Some(Scheme::Arthran));
        assert_eq!(Scheme::Arthran.decrypt(&sample, &key).unwrap(), hex(ARTHRAN_PLAIN));
        assert_eq!(Scheme::Arthran.recover_key(&sample), Some(key.clone()));
        assert_eq!(Scheme::Arthran.encrypt(&hex(ARTHRAN_PLAIN), &key).unwrap(), sample);
    }

    #[test]
    fn standard_roundtrip_and_recover() {
        let key = Scheme::Standard.derive_key("0bbaa902299f82666958ccbb322684e3").unwrap();
        let mut plain = PNG_HEAD.to_vec();
        plain.extend_from_slice(b"rest of the png data");

        let enc = Scheme::Standard.encrypt(&plain, &key).unwrap();
        assert_eq!(&enc[..16], Scheme::Standard.header());
        assert_eq!(&enc[16 + 16..], b"rest of the png data"); // XOR는 앞 16바이트만
        assert_eq!(Scheme::detect(&enc), Some(Scheme::Standard));
        assert_eq!(Scheme::Standard.decrypt(&enc, &key).unwrap(), plain);
        assert_eq!(Scheme::Standard.recover_key(&enc), Some(key));
        assert!(looks_like(&plain, "png"));
    }

    #[test]
    fn short_body_xors_what_exists() {
        let key = Scheme::Arthran.derive_key("abc").unwrap();
        let enc = Scheme::Arthran.encrypt(b"OggS", &key).unwrap();
        assert_eq!(enc.len(), 32 + 4);
        assert_eq!(Scheme::Arthran.decrypt(&enc, &key).unwrap(), b"OggS");
        assert_eq!(Scheme::Arthran.recover_key(&enc), None); // PNG 16바이트가 안 됨
    }

    #[test]
    fn rejects_bad_input() {
        let std_key = Scheme::Standard.derive_key("00112233445566778899aabbccddeeff").unwrap();
        assert!(Scheme::Standard.derive_key("0011").is_err());
        assert!(Scheme::Standard.derive_key("zz112233445566778899aabbccddeeff").is_err());
        assert!(Key::from_hex("가나").is_err());
        // 키 길이가 방식과 안 맞음
        assert!(Scheme::Arthran.decrypt(&arthran_sample(), &std_key).is_err());
        // 헤더가 다름
        assert!(Scheme::Standard.decrypt(&arthran_sample(), &std_key).is_err());
        assert_eq!(Scheme::detect(&PNG_HEAD), None);
    }

    #[test]
    fn ext_mapping() {
        assert_eq!(decrypted_ext("rpgmvp"), Some("png"));
        assert_eq!(decrypted_ext("OGG_"), Some("ogg"));
        assert_eq!(decrypted_ext("rpgmvm"), Some("m4a"));
        assert_eq!(decrypted_ext("png"), None);
        for plain in ["png", "ogg", "m4a"] {
            assert!(encrypted_exts(plain).iter().all(|e| decrypted_ext(e) == Some(plain)));
        }
    }
}
