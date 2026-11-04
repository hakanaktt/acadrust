//! DWG security and decryption support.
//!
//! Provides password-based decryption for DWG files protected with the
//! `AcDb:Security` section mechanism (AutoCAD 2004+ / AC1018+).
//!
//! Clean-room implementation based on the public Open Design specification
//! and RFC 1321 (MD5 message-digest algorithm). No GPL code.

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

use crate::error::DxfError;

/// Known 16-byte verification string used by AutoCAD to validate passwords.
const SECURITY_TEST_STRING: &[u8; 16] = b"SamirBajajSamirB";

/// Magic value marking the provider section in `AcDb:Security`.
const SECURITY_MAGIC: u32 = 0xABCD_ABCD;

/// Pure Rust RFC 1321 MD5 message-digest implementation (clean-room).
pub fn md5(data: &[u8]) -> [u8; 16] {
    let mut a: u32 = 0x6745_2301;
    let mut b: u32 = 0xEFCD_AB89;
    let mut c: u32 = 0x98BA_DCFE;
    let mut d: u32 = 0x1032_5476;

    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let s: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let k: [u32; 64] = [
        0xD76A_A478, 0xE8C7_B756, 0x2420_70DB, 0xC1BD_CEEE,
        0xF57C_0FAF, 0x4787_C62A, 0xA830_4613, 0xFD46_9501,
        0x6980_98D8, 0x8B44_F7AF, 0xFFFF_5BB1, 0x895C_D7BE,
        0x6B90_1122, 0xFD98_7193, 0xA679_438E, 0x49B4_0821,
        0xF61E_2562, 0xC040_B340, 0x265E_5A51, 0xE9B6_C7AA,
        0xD62F_105D, 0x0244_1453, 0xD8A1_E681, 0xE7D3_FBC8,
        0x21E1_CDE6, 0xC337_07D6, 0xF4D5_0D87, 0x455A_14ED,
        0xA9E3_E905, 0xFCEF_A3F8, 0x676F_02D9, 0x8D2A_4C8A,
        0xFFFA_3942, 0x8771_F681, 0x6D9D_6122, 0xFDE5_380C,
        0xA4BE_EA44, 0x4BDE_CFA9, 0xF6BB_4B60, 0xBEBF_BC70,
        0x289B_7EC6, 0xEAA1_27FA, 0xD4EF_3085, 0x0488_1D05,
        0xD9D4_D039, 0xE6DB_99E5, 0x1FA2_7CF8, 0xC4AC_5665,
        0xF429_2244, 0x432A_FF97, 0xAB94_23A7, 0xFC93_A039,
        0x655B_59C3, 0x8F0C_CC92, 0xFFEF_F47D, 0x8584_5DD1,
        0x6FA8_7E4F, 0xFE2C_E6E0, 0xA301_4314, 0x4E08_11A1,
        0xF753_7E82, 0xBD3A_F235, 0x2AD7_D2BB, 0xEB86_D391,
    ];

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, part) in chunk.chunks_exact(4).enumerate() {
            m[i] = u32::from_le_bytes([part[0], part[1], part[2], part[3]]);
        }

        let (mut aa, mut bb, mut cc, mut dd) = (a, b, c, d);

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((bb & cc) | ((!bb) & dd), i),
                16..=31 => ((dd & bb) | ((!dd) & cc), (5 * i + 1) % 16),
                32..=47 => (bb ^ cc ^ dd, (3 * i + 5) % 16),
                48..=63 => (cc ^ (bb | (!dd)), (7 * i) % 16),
                _ => unreachable!(),
            };
            let temp = dd;
            dd = cc;
            cc = bb;
            bb = bb.wrapping_add(
                (aa.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g])).rotate_left(s[i]),
            );
            aa = temp;
        }

        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..8].copy_from_slice(&b.to_le_bytes());
    out[8..12].copy_from_slice(&c.to_le_bytes());
    out[12..16].copy_from_slice(&d.to_le_bytes());
    out
}

/// RC4 (Rivest Cipher 4) symmetric stream cipher.
#[derive(Clone)]
pub struct Rc4 {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl Rc4 {
    /// Initialize RC4 state with the provided key.
    pub fn new(key: &[u8]) -> Self {
        assert!(!key.is_empty(), "RC4 key must not be empty");
        let mut s = [0u8; 256];
        for (i, byte) in s.iter_mut().enumerate() {
            *byte = i as u8;
        }
        let mut j: u8 = 0;
        for i in 0..256 {
            j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
            s.swap(i, j as usize);
        }
        Rc4 { s, i: 0, j: 0 }
    }

    /// Encrypt or decrypt data in place.
    pub fn apply(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            self.i = self.i.wrapping_add(1);
            self.j = self.j.wrapping_add(self.s[self.i as usize]);
            self.s.swap(self.i as usize, self.j as usize);
            let k = self.s
                [(self.s[self.i as usize].wrapping_add(self.s[self.j as usize])) as usize];
            *byte ^= k;
        }
    }
}

/// Security metadata parsed from the `AcDb:Security` DWG section.
#[derive(Debug, Clone)]
pub struct DwgSecurityInfo {
    /// Cryptographic provider type (e.g. 1 = PROV_RSA_FULL)
    pub provider_type: u32,
    /// Cryptographic provider name
    pub provider_name: String,
    /// Cryptographic algorithm ID (e.g. 0x6801 = CALG_RC4)
    pub algorithm_id: u32,
    /// Key length in bits (e.g. 40, 56, 128)
    pub key_length_bits: u32,
    /// Test encrypted verification sequence (16 bytes)
    pub test_bytes: Vec<u8>,
}

impl DwgSecurityInfo {
    /// Parse `AcDb:Security` section data.
    pub fn parse(data: &[u8]) -> Result<Self, DxfError> {
        if data.len() < 24 {
            return Err(DxfError::Decryption(
                "AcDb:Security section too short".to_string(),
            ));
        }

        let mut cursor = Cursor::new(data);
        let _hdr_len = cursor.read_i32::<LittleEndian>()?;
        let _flags = cursor.read_i32::<LittleEndian>()?;
        let magic = cursor.read_u32::<LittleEndian>()?;
        if magic != SECURITY_MAGIC {
            return Err(DxfError::Decryption(format!(
                "Invalid security section magic: expected {:#010X}, got {:#010X}",
                SECURITY_MAGIC, magic
            )));
        }

        let provider_type = cursor.read_u32::<LittleEndian>()?;
        let provider_name_byte_len = cursor.read_u32::<LittleEndian>()? as usize;

        let pos = cursor.position() as usize;
        if pos + provider_name_byte_len > data.len() {
            return Err(DxfError::Decryption(
                "Provider name exceeds security section length".to_string(),
            ));
        }

        let name_bytes = &data[pos..pos + provider_name_byte_len];
        cursor.set_position((pos + provider_name_byte_len) as u64);

        // UTF-16LE provider name
        let u16_chars: Vec<u16> = name_bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&c| c != 0)
            .collect();
        let provider_name = String::from_utf16_lossy(&u16_chars);

        let algorithm_id = cursor.read_u32::<LittleEndian>()?;
        let key_length_bits = cursor.read_u32::<LittleEndian>()?;
        let test_len = cursor.read_u32::<LittleEndian>()? as usize;

        let test_pos = cursor.position() as usize;
        if test_pos + test_len > data.len() {
            return Err(DxfError::Decryption(
                "Security test data exceeds section length".to_string(),
            ));
        }
        let test_bytes = data[test_pos..test_pos + test_len].to_vec();

        Ok(DwgSecurityInfo {
            provider_type,
            provider_name,
            algorithm_id,
            key_length_bits,
            test_bytes,
        })
    }

    /// Derive and verify decryption key from password.
    ///
    /// Derivation algorithm:
    /// 1. Convert password to uppercase.
    /// 2. Encode to UTF-16LE bytes.
    /// 3. MD5 hash the UTF-16LE bytes.
    /// 4. Extract `key_length_bits / 8` bytes.
    /// 5. Validate by decrypting `test_bytes` and checking against `"SamirBajajSamirB"`.
    pub fn derive_key(&self, password: &str) -> Result<Vec<u8>, DxfError> {
        let pwd_upper = password.to_uppercase();
        let utf16_bytes: Vec<u8> = pwd_upper
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let hash = md5(&utf16_bytes);

        let key_len = (self.key_length_bits / 8) as usize;
        if key_len == 0 || key_len > hash.len() {
            return Err(DxfError::Decryption(format!(
                "Unsupported encryption key length: {} bits",
                self.key_length_bits
            )));
        }

        let key = hash[..key_len].to_vec();

        // Verify key against test sequence
        if !self.test_bytes.is_empty() {
            let mut decrypted_test = self.test_bytes.clone();
            let mut rc4 = Rc4::new(&key);
            rc4.apply(&mut decrypted_test);

            if decrypted_test != SECURITY_TEST_STRING {
                return Err(DxfError::Decryption(
                    "Invalid password for encrypted DWG drawing".to_string(),
                ));
            }
        }

        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc1321_md5_vectors() {
        // RFC 1321 Section A.5 test vectors
        assert_eq!(
            md5(b""),
            [0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8, 0x42, 0x7e]
        );
        assert_eq!(
            md5(b"a"),
            [0x0c, 0xc1, 0x75, 0xb9, 0xc0, 0xf1, 0xb6, 0xa8, 0x31, 0xc3, 0x99, 0xe2, 0x69, 0x77, 0x26, 0x61]
        );
        assert_eq!(
            md5(b"abc"),
            [0x90, 0x01, 0x50, 0x98, 0x3c, 0xd2, 0x4f, 0xb0, 0xd6, 0x96, 0x3f, 0x7d, 0x28, 0xe1, 0x7f, 0x72]
        );
        assert_eq!(
            md5(b"message digest"),
            [0xf9, 0x6b, 0x69, 0x7d, 0x7c, 0xb7, 0x93, 0x8d, 0x52, 0x5a, 0x2f, 0x31, 0xaa, 0xf1, 0x61, 0xd0]
        );
    }

    #[test]
    fn test_rc4_cipher() {
        let key = b"Key";
        let mut data = b"Plaintext".to_vec();
        let mut cipher = Rc4::new(key);
        cipher.apply(&mut data);
        assert_eq!(data, [0xBB, 0xF3, 0x16, 0xE8, 0xD9, 0x40, 0xAF, 0x0A, 0xD3]);

        // Decrypt back
        let mut decipher = Rc4::new(key);
        decipher.apply(&mut data);
        assert_eq!(data, b"Plaintext");
    }

    #[test]
    fn test_dwg_security_info() {
        let raw = vec![
            0x0C, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00,
            0xCD, 0xAB, 0xCD, 0xAB, // magic 0xABCDABCD
            0x01, 0x00, 0x00, 0x00, // provider_type = 1
            0x10, 0x00, 0x00, 0x00, // name len = 16 bytes (8 UTF-16 chars)
            b'T', 0, b'e', 0, b's', 0, b't', 0, b'P', 0, b'r', 0, b'o', 0, b'v', 0,
            0x01, 0x68, 0x00, 0x00, // alg_id = 0x6801 (RC4)
            0x38, 0x00, 0x00, 0x00, // key_len = 56 bits (7 bytes)
            0x10, 0x00, 0x00, 0x00, // test_len = 16
            // Encrypted "SamirBajajSamirB" with uppercase "BUDWEISER"
            0x83, 0xF8, 0xC2, 0x56, 0x46, 0x24, 0x41, 0x5F,
            0x36, 0x1C, 0x80, 0xBA, 0x10, 0x39, 0x69, 0xC4,
        ];

        let sec = DwgSecurityInfo::parse(&raw).unwrap();
        assert_eq!(sec.provider_type, 1);
        assert_eq!(sec.provider_name, "TestProv");
        assert_eq!(sec.algorithm_id, 0x6801);
        assert_eq!(sec.key_length_bits, 56);

        // Case-insensitive password validation
        let key_lower = sec.derive_key("budweiser").unwrap();
        let key_upper = sec.derive_key("BUDWEISER").unwrap();
        assert_eq!(key_lower, key_upper);
        assert_eq!(key_lower.len(), 7);

        // Wrong password must fail
        assert!(sec.derive_key("wrong_password").is_err());
    }
}
