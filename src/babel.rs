pub const ALPHABET: &[u8; 70] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789! &()-_+";
pub const ALPHABET_LEN: u32 = 70;
pub const NAME_LEN: usize = 2;
pub const DIR_OBJECTS: u32 = 4900;
pub const OBJECTS_PER_DIR: u32 = 4901;
pub const FILE_LOCAL: u32 = 4901;
pub const FILE_NAME: &[u8] = b"file";
pub const BUF: usize = 3137;
pub const LIMBS: usize = 1569;

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

fn mul_add_small(limbs: &mut [u16], n: &mut usize, m: u32, add: u32) -> Result<(), Error> {
    let mut carry = add;
    let mut i = 0usize;
    while i < *n {
        let v = limbs[i] as u32 * m + carry;
        limbs[i] = v as u16;
        carry = v >> 16;
        i += 1;
    }
    while carry != 0 {
        if *n >= limbs.len() {
            return Err(Error::Overflow);
        }
        limbs[*n] = carry as u16;
        *n += 1;
        carry >>= 16;
    }
    Ok(())
}

fn byte_at(limbs: &[u16], i: usize) -> u8 {
    (limbs[i >> 1] >> ((i & 1) * 8)) as u8
}

pub struct Babel {
    limbs: [u16; LIMBS],
    n: usize,
}

impl Default for Babel {
    fn default() -> Self {
        Self::new()
    }
}

impl Babel {
    pub const fn new() -> Self {
        Self {
            limbs: [0; LIMBS],
            n: 0,
        }
    }

    pub fn clear(&mut self) {
        self.n = 0;
    }

    pub fn push(&mut self, local: u16) -> Result<(), Error> {
        mul_add_small(&mut self.limbs, &mut self.n, DIR_OBJECTS, local as u32)
    }

    pub fn load(&mut self, path: &[u16]) -> Result<(), Error> {
        self.n = 0;
        for &h in path {
            self.push(h)?;
        }
        Ok(())
    }

    pub fn byte_len(&self) -> usize {
        if self.n == 0 {
            return 0;
        }
        if self.limbs[self.n - 1] >= 256 {
            2 * self.n
        } else {
            2 * self.n - 1
        }
    }

    pub fn byte(&self, i: usize) -> u8 {
        byte_at(&self.limbs, i)
    }

    pub fn size(&self) -> usize {
        let m = self.byte_len();
        if m <= 1 {
            return m;
        }
        if self.byte(m - 1) != 1 {
            return m;
        }
        let mut i = m - 1;
        while i >= 2 {
            let b = self.byte(i - 1);
            if b != 1 {
                return if b > 1 { m } else { m - 1 };
            }
            i -= 1;
        }
        if self.byte(0) == 0 { m - 1 } else { m }
    }

    pub fn content_into(&self, out: &mut [u8]) -> Result<usize, Error> {
        let l = self.size();
        if out.len() < l {
            return Err(Error::Overflow);
        }
        let mut borrow = 0i32;
        let mut i = 0usize;
        while i < l {
            let v = self.byte(i) as i32 - 1 - borrow;
            if v < 0 {
                out[i] = (v + 256) as u8;
                borrow = 1;
            } else {
                out[i] = v as u8;
                borrow = 0;
            }
            i += 1;
        }
        Ok(l)
    }

    pub fn file_size(&mut self, path: &[u16]) -> Result<usize, Error> {
        self.load(path)?;
        Ok(self.size())
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
        path.split(|c| c == '/' || c == '\\')
            .filter(|s| !s.is_empty())
            .map(|s| dir_local(s.as_bytes()).unwrap() as u16)
            .collect()
    }

    fn content_vec(path: &[u16]) -> Vec<u8> {
        let mut babel = Babel::new();
        babel.load(path).unwrap();
        let mut out = vec![0u8; babel.size()];
        babel.content_into(&mut out).unwrap();
        out
    }

    fn content_for_path(path: &str) -> Vec<u8> {
        content_vec(&locals_for_path(path))
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
        let mut out = [0u8; 1];
        assert_eq!(babel.content_into(&mut out).unwrap(), 0);
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
            let mut buf = vec![0u8; size];
            let n = babel.content_into(&mut buf).unwrap();
            assert_eq!(size, n, "depth={}", depth);
            assert_eq!(babel.size(), size);
            let n2 = babel.content_into(&mut buf).unwrap();
            assert_eq!(n2, n);
        }
    }

    fn babel_from_bytes(bytes: &[u8]) -> Babel {
        let mut b = Babel::new();
        let mut n = bytes.len().div_ceil(2);
        for i in 0..n {
            let lo = bytes.get(2 * i).copied().unwrap_or(0) as u16;
            let hi = bytes.get(2 * i + 1).copied().unwrap_or(0) as u16;
            b.limbs[i] = lo | (hi << 8);
        }
        while n > 0 && b.limbs[n - 1] == 0 {
            n -= 1;
        }
        b.n = n;
        b
    }

    #[test]
    fn size_boundaries() {
        assert_eq!(babel_from_bytes(&[]).size(), 0);
        assert_eq!(babel_from_bytes(&[1]).size(), 1);
        assert_eq!(babel_from_bytes(&[0, 1]).size(), 1);
        assert_eq!(babel_from_bytes(&[1, 1]).size(), 2);
        assert_eq!(babel_from_bytes(&[0, 0, 1]).size(), 2);
        assert_eq!(babel_from_bytes(&[0, 1, 1]).size(), 2);
        assert_eq!(babel_from_bytes(&[1, 1, 1]).size(), 3);
        assert_eq!(babel_from_bytes(&[0, 1, 1, 1]).size(), 3);
        assert_eq!(babel_from_bytes(&[1, 1, 1, 1]).size(), 4);
        assert_eq!(babel_from_bytes(&[0, 2]).size(), 2);
        assert_eq!(babel_from_bytes(&[0, 0, 2]).size(), 3);
    }

    #[test]
    fn max_path_fits() {
        let path: Vec<u16> = (0..2047).map(|_| 4900u16).collect();
        let mut b = Babel::new();
        assert_eq!(b.file_size(&path).unwrap(), 3137);
        let mut out = vec![0u8; 3137];
        assert_eq!(b.content_into(&mut out).unwrap(), 3137);
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

    #[test]
    fn separator_agnostic() {
        assert_eq!(content_for_path("/AD/HU/Uy/xG"), b"hello".to_vec());
        assert_eq!(content_for_path("\\AD\\HU\\Uy\\xG"), b"hello".to_vec());
        assert_eq!(content_for_path("AD\\HU/Uy\\xG"), b"hello".to_vec());
    }
}

#[cfg(test)]
pub(crate) mod bench {
    use super::*;
    use std::hint::black_box;
    use std::time::Instant;

    pub fn path(d: usize) -> Vec<u16> {
        (0..d).map(|i| ((i * 37) % 4900 + 1) as u16).collect()
    }

    pub fn run(name: &str, iters: u32, mut f: impl FnMut()) {
        for _ in 0..(iters / 8).max(1) {
            f();
        }
        let t = Instant::now();
        for _ in 0..iters {
            f();
        }
        let el = t.elapsed();
        println!(
            "{:<40} {:>12.1} ns/op",
            name,
            el.as_nanos() as f64 / iters as f64
        );
    }

    #[test]
    #[ignore]
    fn bench_babel() {
        let mut b = Babel::new();
        run("dir_name", 2_000_000, || {
            black_box(dir_name(black_box(3137)));
        });
        run("dir_local", 2_000_000, || {
            black_box(dir_local(black_box(b"Qx")));
        });
        run("alphabet_index", 2_000_000, || {
            black_box(alphabet_index(black_box(b'+')));
        });
        let mut cbuf = vec![0u8; BUF];
        for &d in &[1usize, 8, 32, 64, 128, 256, 512, 1024, 2048] {
            let p = path(d);
            let iters: u32 = if d > 512 { 3_000 } else { 100_000 };
            run(&format!("file_size d={}", d), iters, || {
                black_box(b.file_size(black_box(&p)).unwrap());
            });
            run(&format!("file_content d={}", d), iters, || {
                b.load(black_box(&p)).unwrap();
                black_box(b.content_into(&mut cbuf).unwrap());
            });
        }
    }

    fn fold_bytewise(path: &[u16], buf: &mut [u8]) -> usize {
        let mut len = 0usize;
        for &h in path {
            let mut carry = 0u32;
            let mut i = 0usize;
            while i < len {
                let v = buf[i] as u32 * DIR_OBJECTS + carry;
                buf[i] = v as u8;
                carry = v >> 8;
                i += 1;
            }
            while carry != 0 {
                buf[len] = carry as u8;
                len += 1;
                carry >>= 8;
            }
            let mut carry = h as u32;
            let mut i = 0usize;
            while carry != 0 {
                let cur = if i < len { buf[i] as u32 } else { 0 };
                let v = cur + (carry & 0xFF);
                buf[i] = v as u8;
                carry = (carry >> 8) + (v >> 8);
                if i >= len {
                    len = i + 1;
                }
                i += 1;
            }
        }
        len
    }

    fn fold_fused(path: &[u16], buf: &mut [u8]) -> usize {
        let mut len = 0usize;
        for &h in path {
            let mut carry = h as u32;
            let mut i = 0usize;
            while i + 1 < len {
                let limb = (buf[i] as u32) | ((buf[i + 1] as u32) << 8);
                let v = limb * DIR_OBJECTS + carry;
                buf[i] = v as u8;
                buf[i + 1] = (v >> 8) as u8;
                carry = v >> 16;
                i += 2;
            }
            if i < len {
                let v = (buf[i] as u32) * DIR_OBJECTS + carry;
                buf[i] = v as u8;
                carry = v >> 8;
            }
            while carry != 0 {
                buf[len] = carry as u8;
                len += 1;
                carry >>= 8;
            }
        }
        len
    }

    fn fold_u16(path: &[u16], buf: &mut [u16]) -> usize {
        let mut n = 0usize;
        for &h in path {
            mul_add_small(buf, &mut n, DIR_OBJECTS, h as u32).unwrap();
        }
        n
    }

    #[test]
    #[ignore]
    fn bench_fold_variants() {
        use std::time::Instant;
        let mut rng = 0x1234_5678_9abc_def0u64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for &d in &[16usize, 64, 256, 1024] {
            let p: Vec<u16> = (0..d).map(|_| (next() % 4900 + 1) as u16).collect();
            let mut bb = vec![0u8; BUF + 4];
            let mut ub = vec![0u16; BUF / 2 + 4];
            let iters: u32 = if d > 256 { 5_000 } else { 50_000 };
            let bench = |name: &str, f: &mut dyn FnMut() -> usize| {
                for _ in 0..(iters / 8).max(1) {
                    f();
                }
                let t = Instant::now();
                let mut acc = 0usize;
                for _ in 0..iters {
                    acc ^= black_box(f());
                }
                black_box(acc);
                println!(
                    "fold {:<12} d={:<5} {:>10.1} ns/op",
                    name,
                    d,
                    t.elapsed().as_nanos() as f64 / iters as f64
                );
            };
            bench("bytewise", &mut || fold_bytewise(black_box(&p), &mut bb));
            bench("fused-pair", &mut || fold_fused(black_box(&p), &mut bb));
            bench("u16-limb", &mut || fold_u16(black_box(&p), &mut ub));
            let la = fold_bytewise(&p, &mut bb);
            let lb = fold_fused(&p, &mut bb);
            let lu = fold_u16(&p, &mut ub);
            let mut ub_bytes = vec![0u8; lu * 2];
            for i in 0..lu {
                ub_bytes[2 * i..2 * i + 2].copy_from_slice(&ub[i].to_le_bytes());
            }
            let mut m = ub_bytes.len();
            while m > 0 && ub_bytes[m - 1] == 0 {
                m -= 1;
            }
            assert_eq!(la, lb);
            assert_eq!(&bb[..la], &ub_bytes[..m]);
        }
    }
}
