pub const ALPHABET: &[u8; 70] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789! &()-_+";
pub const ALPHABET_LEN: u32 = 70;
pub const NAME_LEN: usize = 2;
pub const DIR_OBJECTS: u32 = 4900;
pub const OBJECTS_PER_DIR: u32 = 4901;
pub const FILE_LOCAL: u32 = 4901;
pub const FILE_NAME: &[u8] = b"file";
pub const BUF: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Overflow,
}

pub fn alphabet_index(c: u8) -> Option<u32> {
    let mut i = 0usize;
    while i < ALPHABET.len() {
        if ALPHABET[i] == c {
            return Some(i as u32);
        }
        i += 1;
    }
    None
}

pub fn dir_name(local: u32) -> [u8; NAME_LEN] {
    let mut idx = local - 1;
    let b = ALPHABET[(idx % ALPHABET_LEN) as usize];
    idx /= ALPHABET_LEN;
    let a = ALPHABET[(idx % ALPHABET_LEN) as usize];
    [a, b]
}

pub fn dir_local(name: &[u8]) -> Option<u32> {
    if name.len() != NAME_LEN {
        return None;
    }
    let a = alphabet_index(name[0])?;
    let b = alphabet_index(name[1])?;
    Some(a * ALPHABET_LEN + b + 1)
}

fn mul_small(buf: &mut [u8], len: &mut usize, m: u32) -> Result<(), Error> {
    let mut carry: u32 = 0;
    let mut i = 0usize;
    while i < *len {
        let v = buf[i] as u32 * m + carry;
        buf[i] = v as u8;
        carry = v >> 8;
        i += 1;
    }
    while carry != 0 {
        if *len >= buf.len() {
            return Err(Error::Overflow);
        }
        buf[*len] = carry as u8;
        *len += 1;
        carry >>= 8;
    }
    Ok(())
}

fn add_small(buf: &mut [u8], len: &mut usize, a: u32) -> Result<(), Error> {
    let mut carry = a;
    let mut i = 0usize;
    while carry != 0 {
        if i >= buf.len() {
            return Err(Error::Overflow);
        }
        let cur = if i < *len { buf[i] as u32 } else { 0 };
        let v = cur + (carry & 0xFF);
        buf[i] = v as u8;
        carry = (carry >> 8) + (v >> 8);
        if i >= *len {
            *len = i + 1;
        }
        i += 1;
    }
    Ok(())
}

pub struct Babel {
    file: [u8; BUF],
    scratch: [u8; BUF],
}

impl Default for Babel {
    fn default() -> Self {
        Self::new()
    }
}

impl Babel {
    pub const fn new() -> Self {
        Self {
            file: [0; BUF],
            scratch: [0; BUF],
        }
    }

    fn fold(&mut self, path: &[u16]) -> Result<usize, Error> {
        let mut len = 0usize;
        for &h in path {
            mul_small(&mut self.file, &mut len, DIR_OBJECTS)?;
            add_small(&mut self.file, &mut len, h as u32)?;
        }
        Ok(len)
    }

    fn biased_len(&mut self, len: usize) -> Result<usize, Error> {
        if len == 0 {
            return Ok(0);
        }
        self.scratch[..len].copy_from_slice(&self.file[..len]);
        let mut slen = len;
        add_small(&mut self.scratch, &mut slen, 1)?;
        mul_small(&mut self.scratch, &mut slen, 255)?;
        while slen > 0 && self.scratch[slen - 1] == 0 {
            slen -= 1;
        }
        Ok(slen.saturating_sub(1))
    }

    pub fn file_size(&mut self, path: &[u16]) -> Result<usize, Error> {
        let len = self.fold(path)?;
        self.biased_len(len)
    }

    pub fn file_content(&mut self, path: &[u16]) -> Result<&[u8], Error> {
        let len = self.fold(path)?;
        if len == 0 {
            return Ok(&self.file[..0]);
        }
        let l = self.biased_len(len)?;
        let mut borrow = 0i32;
        let mut i = 0usize;
        while i < len {
            let sub = if i < l { 1 } else { 0 };
            let v = self.file[i] as i32 - sub - borrow;
            if v < 0 {
                self.file[i] = (v + 256) as u8;
                borrow = 1;
            } else {
                self.file[i] = v as u8;
                borrow = 0;
            }
            i += 1;
        }
        if borrow != 0 {
            return Err(Error::Overflow);
        }
        Ok(&self.file[..l])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
    }

    fn locals_for_path(path: &str) -> Vec<u16> {
        path.split('/')
            .filter(|s| !s.is_empty())
            .map(|s| dir_local(s.as_bytes()).unwrap() as u16)
            .collect()
    }

    fn content_for_path(path: &str) -> Vec<u8> {
        let mut babel = Babel::new();
        babel.file_content(&locals_for_path(path)).unwrap().to_vec()
    }

    #[test]
    fn alphabet_is_70() {
        assert_eq!(ALPHABET.len(), 70);
        assert_eq!(DIR_OBJECTS, 4900);
        assert_eq!(OBJECTS_PER_DIR, 4901);
    }

    #[test]
    fn names_roundtrip() {
        for local in 1..=4900u32 {
            let name = dir_name(local);
            assert_eq!(dir_local(&name), Some(local));
        }
        assert_eq!(dir_name(1), *b"AA");
        assert_eq!(dir_name(4900), *b"++");
        assert_eq!(dir_local(b"AA"), Some(1));
        assert_eq!(dir_local(b"++"), Some(4900));
        assert_eq!(dir_local(b"A"), None);
    }

    #[test]
    fn empty_file_is_empty() {
        let mut babel = Babel::new();
        assert_eq!(babel.file_size(&[]).unwrap(), 0);
        assert_eq!(babel.file_content(&[]).unwrap().len(), 0);
    }

    #[test]
    fn deep_path_is_bounded() {
        let mut babel = Babel::new();
        let path: Vec<u16> = (0..3000).map(|_| 1u16).collect();
        assert!(babel.file_size(&path).is_err());
    }

    #[test]
    fn random_paths_are_consistent() {
        let mut rng = Rng(0x0bad_c0de_1234_5678);
        let mut babel = Babel::new();
        for _ in 0..500 {
            let depth = (rng.next() % 60) as usize;
            let path: Vec<u16> = (0..depth).map(|_| (rng.next() % 4900 + 1) as u16).collect();
            let size = babel.file_size(&path).unwrap();
            let content = babel.file_content(&path).unwrap().to_vec();
            assert_eq!(size, content.len(), "depth={}", depth);
            assert_eq!(babel.file_content(&path).unwrap(), &content[..]);
        }
    }

    #[test]
    fn decoder_fixtures() {
        let cases: &[(&[u8], &str)] = &[
            (&[], ""),
            (&[0x1b], "/Ab"),
            (&[0xc1], "/C1"),
            (&[0x8f, 0xe6], "/AL/G7"),
            (&[0xe8, 0x6e], "/AE/7S"),
            (&[0xdc, 0x9a, 0xa5], "/f5/X)"),
            (&[0xa8, 0x63, 0x6c], "/U!/HC"),
            (&[0x4d, 0xfd, 0x7a, 0xe4], "/CT/Y3/d "),
            (&[0xcb, 0x36, 0x9a, 0xac], "/By/Vr/ef"),
            (&[0x7b, 0xc0, 0xf6, 0xca, 0x34], "/AA/-g/Ul/gT"),
            (&[0x97, 0x47, 0xc9, 0x64, 0x3d, 0x1e], "/EK/+p/7u/Zd"),
            (
                &[0x62, 0x8a, 0xf9, 0xee, 0x22, 0x15, 0x0d],
                "/AF/9e/!E/Va/)C",
            ),
            (
                &[0x98, 0xb5, 0x64, 0x59, 0x8f, 0xa0, 0x56, 0x59],
                "/AB/VV/cu/KH/a&/mg",
            ),
            (
                &[0x01, 0xc6, 0x84, 0x05, 0xd7, 0x07, 0x48, 0x06, 0x3d],
                "/F2/E /DF/&2/Kk/OZ",
            ),
            (
                &[
                    0x13, 0xf3, 0xdb, 0x63, 0x8c, 0x58, 0xaa, 0x59, 0xde, 0x6c, 0xdd, 0x48,
                ],
                "/E4/FX/8P/jn/Y_/H-/F7/XB",
            ),
            (
                &[
                    0x64, 0x0f, 0xf0, 0x31, 0x8d, 0xa7, 0xb8, 0xd0, 0x69, 0x1b, 0x7b, 0x1b, 0xae,
                    0x54, 0xd6, 0xb2,
                ],
                "/Ac/-I/Rc/Q0/Kb/7g/&2/aZ/ZT/fN/Ak",
            ),
            (
                &[
                    0x6d, 0x30, 0x79, 0xca, 0x42, 0xa2, 0xf5, 0x12, 0x5b, 0x1e, 0x57, 0xf3, 0xca,
                    0x70, 0xc0, 0x7d, 0x13, 0xfa, 0x0e, 0x6d,
                ],
                "/u6/2_/mX/Kx/kj/en/rg/U!/I-/FE/kY/Fn/&+",
            ),
            (
                &[
                    0xf8, 0x18, 0x90, 0xa6, 0x98, 0x03, 0xb9, 0x2b, 0xbf, 0xe2, 0x0d, 0xee, 0xe0,
                    0x97, 0x89, 0x1b, 0x21, 0xa9, 0x57, 0x14, 0x26, 0xd5, 0xeb, 0x64,
                ],
                "/Bn/9w/tU/Ci/r&/l7/ch/mf/XV/D-/gF/vw/Tq/-q/kB/7e",
            ),
            (
                &[
                    0x6c, 0x9e, 0x41, 0xc0, 0x65, 0x2c, 0xe2, 0x2a, 0xb8, 0xf5, 0xd4, 0x0a, 0xc0,
                    0x73, 0x56, 0x39, 0xb2, 0xf7, 0x0c, 0x0a, 0x15, 0x7c, 0xa3, 0xa4, 0x6e, 0xaf,
                    0x5f, 0x8b, 0xc9, 0xfd, 0x16, 0xfe,
                ],
                "/Z9/RN/qO/RP/-J/_c/A0/RO/ex/8u/zT/rL/mg/RB/YM/KD/gF/0E/Rr/K-/hG",
            ),
            (
                &[
                    0x21, 0xbe, 0x8a, 0x03, 0x3f, 0xf6, 0x88, 0x00, 0x84, 0x2f, 0x19, 0x9e, 0x97,
                    0x84, 0x17, 0x14, 0x10, 0xc5, 0xaa, 0x78, 0x29, 0xa2, 0xc9, 0x36, 0x3e, 0x39,
                    0x6f, 0xae, 0xc8, 0x39, 0x3f, 0x4c, 0x15, 0x1d, 0x88, 0x0d, 0x52, 0x2e, 0xea,
                    0x81, 0xe1, 0x12, 0x44, 0x1b, 0x14, 0x3c, 0x16, 0x69,
                ],
                "/AF/nL/Dp/8a/31/PM/)w/Gz/YZ/WI/WB/J-/(I/CD/ZX/&C/sL/-d/UP/B+/Va/NB/Hh/dX/Xr/pj/8T/f-/VX/6+/TL/Rj",
            ),
            (
                &[
                    0x18, 0x45, 0xc7, 0x60, 0xff, 0xb5, 0x96, 0xd4, 0x77, 0xb9, 0xc7, 0xe6, 0xb2,
                    0x1c, 0xfd, 0x4b, 0x95, 0x0d, 0x9f, 0x36, 0x20, 0xd2, 0x14, 0xe7, 0x07, 0xc6,
                    0xd9, 0x2a, 0x24, 0xbc, 0x06, 0x81, 0x05, 0x23, 0xc0, 0xca, 0x7a, 0x5d, 0xc4,
                    0x4d, 0xe4, 0xfc, 0x2d, 0x66, 0xaf, 0x7a, 0xc7, 0x55, 0x37, 0x4b, 0x60, 0xe1,
                    0x87, 0x32, 0x88, 0xed, 0xb8, 0x69, 0x9b, 0xb5, 0x59, 0xcb, 0xe9, 0xf2,
                ],
                "/JM/La/2a/s6/XX/52/!h/he/z5/1j/0N/Tx/dW/TM/P3/Qq/Wy/Vf/AH/fV/Rf/eJ/)n/St/ m/HW/rm/-v/dQ/)h/WK/vT/yz/W /ky/ky/WE/Yp/IW/)I/xz/Iw",
            ),
            (&[0x00], "/AA"),
            (&[0xff], "/Dt"),
            (&[0x00, 0x00, 0x00, 0x00], "/xG/Yc"),
            (&[0xff, 0xff, 0xff, 0xff], "/Cm/o8/UD"),
            (
                &[
                    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
                    0x0d, 0x0e, 0x0f,
                ],
                "/AB/vR/Yn/_f/tY/DV/dz/hZ/)q/AR/!U",
            ),
            (
                &[
                    0x74, 0x68, 0x65, 0x20, 0x71, 0x75, 0x69, 0x63, 0x6b, 0x20, 0x62, 0x72, 0x6f,
                    0x77, 0x6e, 0x20, 0x66, 0x6f, 0x78,
                ],
                "/AN/Jj/JT/oC/SI/a5/bC/4n/qp/1K/!9/gF/7a",
            ),
            (
                &[
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                ],
                "/AG/JQ/Sj/7f/kC/&Y/cQ/p_/nr/8U/ty/mt/QK/VW/sa/Ca/Wc/Z+/+c/lR/bu",
            ),
            (
                &[
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                ],
                "/aE/5I/00/g!/8W/a!/WY/ob/_D/nM/Sk/bE/HX/Cd/W7/z!/+c/Jq/9+/TS/N9",
            ),
        ];
        for (data, path) in cases {
            assert_eq!(content_for_path(path), data.to_vec(), "path={}", path);
        }
    }
}
