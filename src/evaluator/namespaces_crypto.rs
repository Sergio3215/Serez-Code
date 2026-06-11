// Crypto namespace: sha256, md5, base64, hmacSha256, hexEncode, hexDecode,
// randomBytes (CSPRNG), ed25519Keypair/Sign/Verify (firmas).
// Hashes/encodings en Rust puro. Las primitivas con implicaciones de seguridad
// real usan crates vetados: `getrandom` (entropía del OS) y `ed25519-dalek`
// (RustCrypto/dalek) — NUNCA reimplementar firmas o CSPRNG a mano.

use crate::ast;
use crate::region::{ObjectData, ObjectRef, OwnedValue};
use super::EvalResult;

impl super::Evaluator {
    pub(super) fn eval_crypto_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "sha256" => {
                let s = match self.require_one_string(&dot_call.arguments, "Crypto.sha256") {
                    Ok(v) => v, Err(e) => return e,
                };
                let hex = to_hex(&sha256(s.as_bytes()));
                EvalResult::Value(self.alloc(ObjectData::Str(hex)))
            }
            "md5" => {
                let s = match self.require_one_string(&dot_call.arguments, "Crypto.md5") {
                    Ok(v) => v, Err(e) => return e,
                };
                let hex = to_hex(&md5(s.as_bytes()));
                EvalResult::Value(self.alloc(ObjectData::Str(hex)))
            }
            "hmacSha256" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Crypto.hmacSha256(key, data) requires 2 arguments");
                    return EvalResult::Error;
                }
                let key = match self.eval_to_string(&dot_call.arguments[0], "Crypto.hmacSha256 key") {
                    Ok(v) => v, Err(e) => return e,
                };
                let data = match self.eval_to_string(&dot_call.arguments[1], "Crypto.hmacSha256 data") {
                    Ok(v) => v, Err(e) => return e,
                };
                let hash = hmac_sha256(key.as_bytes(), data.as_bytes());
                EvalResult::Value(self.alloc(ObjectData::Str(to_hex(&hash))))
            }
            "base64encode" => {
                let s = match self.require_one_string(&dot_call.arguments, "Crypto.base64encode") {
                    Ok(v) => v, Err(e) => return e,
                };
                EvalResult::Value(self.alloc(ObjectData::Str(base64_encode(s.as_bytes()))))
            }
            "base64decode" => {
                let s = match self.require_one_string(&dot_call.arguments, "Crypto.base64decode") {
                    Ok(v) => v, Err(e) => return e,
                };
                match base64_decode(&s) {
                    Ok(bytes) => match String::from_utf8(bytes) {
                        Ok(decoded) => EvalResult::Value(self.alloc(ObjectData::Str(decoded))),
                        Err(_) => {
                            let msg = self.alloc(ObjectData::Str("Crypto.base64decode: result is not valid UTF-8".to_string()));
                            EvalResult::Throw(msg)
                        }
                    },
                    Err(e) => {
                        let msg = self.alloc(ObjectData::Str(format!("Crypto.base64decode: {}", e)));
                        EvalResult::Throw(msg)
                    }
                }
            }
            "hexEncode" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Crypto.hexEncode(bytes) requires 1 argument");
                    return EvalResult::Error;
                }
                let arr_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.resolve(arr_ref).cloned() {
                    Some(ObjectData::Array { elements, .. }) => {
                        let mut bytes = Vec::new();
                        for e in elements {
                            match e {
                                OwnedValue::Integer(n) if n >= 0 && n <= 255 => {
                                    bytes.push(n as u8);
                                }
                                _ => {
                                    let msg = self.alloc(ObjectData::Str(
                                        "Crypto.hexEncode: all elements must be integers 0-255".to_string()
                                    ));
                                    return EvalResult::Throw(msg);
                                }
                            }
                        }
                        EvalResult::Value(self.alloc(ObjectData::Str(to_hex(&bytes))))
                    }
                    _ => {
                        eprintln!("❌ ERROR: Crypto.hexEncode() requires an array of bytes");
                        EvalResult::Error
                    }
                }
            }
            "hexDecode" => {
                let s = match self.require_one_string(&dot_call.arguments, "Crypto.hexDecode") {
                    Ok(v) => v, Err(e) => return e,
                };
                match hex_decode(&s) {
                    Ok(bytes) => {
                        let owned: Vec<OwnedValue> = bytes.iter()
                            .map(|&b| OwnedValue::Integer(b as i64))
                            .collect();
                        EvalResult::Value(self.alloc(ObjectData::Array {
                            element_type: Some("int".to_string()),
                            elements: owned,
                        }))
                    }
                    Err(e) => {
                        let msg = self.alloc(ObjectData::Str(format!("Crypto.hexDecode: {}", e)));
                        EvalResult::Throw(msg)
                    }
                }
            }
            "sha1" => {
                let s = match self.require_one_string(&dot_call.arguments, "Crypto.sha1") {
                    Ok(v) => v, Err(e) => return e,
                };
                let hex = to_hex(&sha1(s.as_bytes()));
                EvalResult::Value(self.alloc(ObjectData::Str(hex)))
            }
            "sha1base64" => {
                // SHA-1 hash followed by base64 encode — used for WebSocket handshake
                let s = match self.require_one_string(&dot_call.arguments, "Crypto.sha1base64") {
                    Ok(v) => v, Err(e) => return e,
                };
                let hash = sha1(s.as_bytes());
                let b64 = base64_encode(&hash);
                EvalResult::Value(self.alloc(ObjectData::Str(b64)))
            }
            "randomBytes" => {
                // CSPRNG real (entropía del OS vía getrandom). NO usar Random.* para
                // tokens/claves: Random es un LCG predecible.
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Crypto.randomBytes(n) requires 1 argument");
                    return EvalResult::Error;
                }
                let n = match self.eval_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => return EvalResult::Error,
                };
                const MAX_RANDOM_BYTES: i64 = 1_048_576; // 1 MB: evita DoS por asignación gigante
                if n < 1 || n > MAX_RANDOM_BYTES {
                    let msg = self.alloc(ObjectData::Str(format!(
                        "Crypto.randomBytes: n must be between 1 and {}", MAX_RANDOM_BYTES)));
                    return EvalResult::Throw(msg);
                }
                let mut buf = vec![0u8; n as usize];
                if getrandom::getrandom(&mut buf).is_err() {
                    let msg = self.alloc(ObjectData::Str(
                        "Crypto.randomBytes: OS entropy source unavailable".to_string()));
                    return EvalResult::Throw(msg);
                }
                let owned: Vec<OwnedValue> = buf.iter().map(|&b| OwnedValue::Integer(b as i64)).collect();
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: Some("int".to_string()),
                    elements: owned,
                }))
            }
            "ed25519Keypair" => {
                if !dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: Crypto.ed25519Keypair() takes no arguments");
                    return EvalResult::Error;
                }
                let mut seed = [0u8; 32];
                if getrandom::getrandom(&mut seed).is_err() {
                    let msg = self.alloc(ObjectData::Str(
                        "Crypto.ed25519Keypair: OS entropy source unavailable".to_string()));
                    return EvalResult::Throw(msg);
                }
                let signing = ed25519_dalek::SigningKey::from_bytes(&seed);
                let public = signing.verifying_key();
                let entries = vec![
                    (OwnedValue::Str("private".to_string()), OwnedValue::Str(to_hex(&seed))),
                    (OwnedValue::Str("public".to_string()),  OwnedValue::Str(to_hex(public.as_bytes()))),
                ];
                EvalResult::Value(self.alloc(ObjectData::Dict {
                    key_type: "string".to_string(),
                    value_type: "string".to_string(),
                    entries,
                }))
            }
            "ed25519Sign" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Crypto.ed25519Sign(privateHex, message) requires 2 arguments");
                    return EvalResult::Error;
                }
                let priv_hex = match self.eval_to_string(&dot_call.arguments[0], "Crypto.ed25519Sign privateHex") {
                    Ok(v) => v, Err(e) => return e,
                };
                let message = match self.eval_to_string(&dot_call.arguments[1], "Crypto.ed25519Sign message") {
                    Ok(v) => v, Err(e) => return e,
                };
                let seed: [u8; 32] = match hex_decode(&priv_hex) {
                    Ok(b) if b.len() == 32 => {
                        let mut s = [0u8; 32];
                        s.copy_from_slice(&b);
                        s
                    }
                    _ => {
                        let msg = self.alloc(ObjectData::Str(
                            "Crypto.ed25519Sign: privateHex must be 64 hex chars (32 bytes)".to_string()));
                        return EvalResult::Throw(msg);
                    }
                };
                use ed25519_dalek::Signer;
                let signing = ed25519_dalek::SigningKey::from_bytes(&seed);
                let sig = signing.sign(message.as_bytes());
                EvalResult::Value(self.alloc(ObjectData::Str(to_hex(&sig.to_bytes()))))
            }
            "ed25519Verify" => {
                if dot_call.arguments.len() != 3 {
                    eprintln!("❌ ERROR: Crypto.ed25519Verify(publicHex, message, signatureHex) requires 3 arguments");
                    return EvalResult::Error;
                }
                let pub_hex = match self.eval_to_string(&dot_call.arguments[0], "Crypto.ed25519Verify publicHex") {
                    Ok(v) => v, Err(e) => return e,
                };
                let message = match self.eval_to_string(&dot_call.arguments[1], "Crypto.ed25519Verify message") {
                    Ok(v) => v, Err(e) => return e,
                };
                let sig_hex = match self.eval_to_string(&dot_call.arguments[2], "Crypto.ed25519Verify signatureHex") {
                    Ok(v) => v, Err(e) => return e,
                };
                // Hex malformado / longitud incorrecta = error de programación → throw.
                // Clave/firma bien formadas pero inválidas = resultado → false.
                let pub_bytes: [u8; 32] = match hex_decode(&pub_hex) {
                    Ok(b) if b.len() == 32 => {
                        let mut s = [0u8; 32];
                        s.copy_from_slice(&b);
                        s
                    }
                    _ => {
                        let msg = self.alloc(ObjectData::Str(
                            "Crypto.ed25519Verify: publicHex must be 64 hex chars (32 bytes)".to_string()));
                        return EvalResult::Throw(msg);
                    }
                };
                let sig_bytes: [u8; 64] = match hex_decode(&sig_hex) {
                    Ok(b) if b.len() == 64 => {
                        let mut s = [0u8; 64];
                        s.copy_from_slice(&b);
                        s
                    }
                    _ => {
                        let msg = self.alloc(ObjectData::Str(
                            "Crypto.ed25519Verify: signatureHex must be 128 hex chars (64 bytes)".to_string()));
                        return EvalResult::Throw(msg);
                    }
                };
                let verifying = match ed25519_dalek::VerifyingKey::from_bytes(&pub_bytes) {
                    Ok(k) => k,
                    Err(_) => return EvalResult::Value(self.alloc(ObjectData::Boolean(false))),
                };
                let sig = ed25519_dalek::Signature::from_bytes(&sig_bytes);
                // verify_strict: rechaza claves/firmas no canónicas (malleability).
                let ok = verifying.verify_strict(message.as_bytes(), &sig).is_ok();
                EvalResult::Value(self.alloc(ObjectData::Boolean(ok)))
            }
            _ => {
                eprintln!("❌ ERROR: Unknown Crypto method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── Shared eval helpers ───────────────────────────────────────────────────

    pub(super) fn require_one_string(&mut self, args: &[ast::Expression], ctx: &str) -> Result<String, EvalResult> {
        if args.len() != 1 {
            eprintln!("❌ ERROR: {}(string) requires 1 argument", ctx);
            return Err(EvalResult::Error);
        }
        self.eval_to_string(&args[0], ctx)
    }

    pub(super) fn eval_to_string(&mut self, expr: &ast::Expression, ctx: &str) -> Result<String, EvalResult> {
        let r = match self.eval_expression(expr) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Str(s)) => Ok(s.clone()),
            _ => {
                eprintln!("❌ ERROR: {}: argument must be a string", ctx);
                Err(EvalResult::Error)
            }
        }
    }
}

// ── Pure-Rust crypto primitives ───────────────────────────────────────────────

// ── SHA-1 ─────────────────────────────────────────────────────────────────────

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 { msg.push(0); }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19  => ((b & c) | ((!b) & d),          0x5A827999u32),
                20..=39 => (b ^ c ^ d,                      0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d),   0x8F1BBCDCu32),
                _       => (b ^ c ^ d,                      0xCA62C1D6u32),
            };
            let temp = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(w[i]);
            e = d; d = c; c = b.rotate_left(30); b = a; a = temp;
        }
        h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, &word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("hex string must have even length".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| format!("invalid hex character at position {}", i))
        })
        .collect()
}

// ── SHA-256 ───────────────────────────────────────────────────────────────────

fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
        0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
        0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
        0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 { msg.push(0); }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
            let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1  = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch  = (e & f) ^ ((!e) & g);
            let t1  = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0  = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2  = s0.wrapping_add(maj);
            hh = g; g = f; f = e; e = d.wrapping_add(t1);
            d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a); h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c); h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e); h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g); h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for (i, &word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

// ── HMAC-SHA256 ───────────────────────────────────────────────────────────────

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k_block = [0u8; 64];
    if key.len() > 64 {
        let h = sha256(key);
        k_block[..32].copy_from_slice(&h);
    } else {
        k_block[..key.len()].copy_from_slice(key);
    }
    let ipad: Vec<u8> = k_block.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k_block.iter().map(|b| b ^ 0x5c).collect();
    let mut inner = ipad;
    inner.extend_from_slice(data);
    let inner_hash = sha256(&inner);
    let mut outer = opad;
    outer.extend_from_slice(&inner_hash);
    sha256(&outer)
}

// ── MD5 ───────────────────────────────────────────────────────────────────────

fn md5(data: &[u8]) -> [u8; 16] {
    const T: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20, 5,  9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 { msg.push(0); }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let (mut a0, mut b0, mut c0, mut d0): (u32, u32, u32, u32) =
        (0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476);

    for chunk in msg.chunks(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15  => ((b & c) | ((!b) & d),          i),
                16..=31 => ((d & b) | ((!d) & c),          (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d,                      (3 * i + 5) % 16),
                _       => (c ^ (b | (!d)),                 (7 * i)     % 16),
            };
            let temp = d;
            d = c; c = b;
            b = b.wrapping_add(
                a.wrapping_add(f).wrapping_add(T[i]).wrapping_add(m[g])
                 .rotate_left(S[i])
            );
            a = temp;
        }
        a0 = a0.wrapping_add(a); b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c); d0 = d0.wrapping_add(d);
    }
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

// ── Base64 ────────────────────────────────────────────────────────────────────

const B64_TABLE: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let n = chunk.len();
        let b = [chunk[0], if n > 1 { chunk[1] } else { 0 }, if n > 2 { chunk[2] } else { 0 }];
        let combined = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(B64_TABLE[((combined >> 18) & 0x3f) as usize] as char);
        out.push(B64_TABLE[((combined >> 12) & 0x3f) as usize] as char);
        out.push(if n > 1 { B64_TABLE[((combined >> 6) & 0x3f) as usize] as char } else { '=' });
        out.push(if n > 2 { B64_TABLE[(combined & 0x3f) as usize] as char } else { '=' });
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim_end_matches('=');
    let mut out = Vec::new();
    let chars: Vec<u8> = s.bytes().map(|c| {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+'        => 62,
            b'/'        => 63,
            _           => 255,
        }
    }).collect();
    if chars.iter().any(|&c| c == 255) {
        return Err("invalid base64 character".to_string());
    }
    for chunk in chars.chunks(4) {
        let n = chunk.len();
        let v: u32 = chunk.iter().enumerate()
            .fold(0u32, |acc, (i, &b)| acc | ((b as u32) << (18 - 6 * i)));
        out.push(((v >> 16) & 0xff) as u8);
        if n > 2 { out.push(((v >> 8) & 0xff) as u8); }
        if n > 3 { out.push((v & 0xff) as u8); }
    }
    Ok(out)
}
