//! SHA-256 and hex, as a leaf.
//!
//! # Why this is its own module
//!
//! Both `Crypto.sha256` and the package manager's integrity check need the same
//! hash, and the package manager must not depend on the evaluator to get it.
//! Reusing it from `evaluator::namespaces_crypto` created
//! `evaluator -> package_manager -> package_install -> evaluator`, which
//! `tests/architecture.rs` failed on — correctly, and before the commit rather
//! than after.
//!
//! So the hash moved **down** instead: a leaf that depends on nothing and that
//! both layers can depend on. That is the same shape `span` and `diagnostic`
//! have, and it is why the fix was to move code rather than to add a line to
//! `KNOWN_CYCLES`.
//!
//! Hand-rolled rather than a crate, which is how it arrived: `Crypto.sha256` has
//! always been implemented here, and `DEVELOPMENT.md`'s "minimal runtime
//! dependencies" invariant applies. Its behaviour is pinned by
//! `tests/unit_crypto.sz` against published test vectors.

pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// SHA-256 over a stream, so a caller need not hold its input in memory.
///
/// # Why this exists
///
/// `sha256` takes a slice and, worse, copies it: `data.to_vec()` means hashing a
/// 256 MiB package tree held **two** copies of it at once, on top of the file
/// reads. `package_install::tree_digest` did exactly that — it concatenated every
/// file in a package into one `Vec<u8>` and hashed the result.
///
/// The two produce identical digests, because the incremental form feeds the
/// compression function the same bytes in the same order; `sha256` is now a
/// wrapper over this. `tests/unit_crypto.sz` pins both against published vectors.
pub(crate) struct Sha256 {
    state: [u32; 8],
    /// Bytes not yet forming a complete 64-byte block.
    pending: [u8; 64],
    pending_len: usize,
    /// Total bytes consumed, for the length suffix.
    total: u64,
}

impl Sha256 {
    pub(crate) fn new() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            pending: [0u8; 64],
            pending_len: 0,
            total: 0,
        }
    }

    pub(crate) fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);

        if self.pending_len > 0 {
            let want = 64 - self.pending_len;
            let take = want.min(data.len());
            self.pending[self.pending_len..self.pending_len + take].copy_from_slice(&data[..take]);
            self.pending_len += take;
            data = &data[take..];
            if self.pending_len == 64 {
                let block = self.pending;
                compress(&mut self.state, &block);
                self.pending_len = 0;
            } else {
                // Everything was taken and the block is still short. Falling
                // through would reach `self.pending_len = rest.len()` below with
                // an empty remainder and silently discard what is buffered.
                debug_assert!(data.is_empty(), "data left over with a short block");
                return;
            }
        }

        let mut chunks = data.chunks_exact(64);
        for chunk in &mut chunks {
            compress(&mut self.state, chunk);
        }

        let rest = chunks.remainder();
        self.pending[..rest.len()].copy_from_slice(rest);
        self.pending_len = rest.len();
    }

    pub(crate) fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.total * 8;

        // The padding is at most two blocks: 0x80, zeroes to offset 56, length.
        let mut tail = [0u8; 128];
        tail[0] = 0x80;
        let pad_to = if self.pending_len < 56 { 56 } else { 120 };
        let fill = pad_to - self.pending_len;
        tail[fill..fill + 8].copy_from_slice(&bit_len.to_be_bytes());
        let tail_len = fill + 8;

        // `update` would add these to `total`, which is already fixed above.
        let keep = self.total;
        self.update(&tail[..tail_len]);
        self.total = keep;
        debug_assert_eq!(self.pending_len, 0, "padding did not complete a block");

        let mut out = [0u8; 32];
        for (i, &word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize()
}

/// One 64-byte block of the SHA-256 compression function.
fn compress(h: &mut [u32; 8], chunk: &[u8]) {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }
    let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
        (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    h[0] = h[0].wrapping_add(a);
    h[1] = h[1].wrapping_add(b);
    h[2] = h[2].wrapping_add(c);
    h[3] = h[3].wrapping_add(d);
    h[4] = h[4].wrapping_add(e);
    h[5] = h[5].wrapping_add(f);
    h[6] = h[6].wrapping_add(g);
    h[7] = h[7].wrapping_add(hh);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeding the same bytes in any number of pieces must give one digest.
    ///
    /// This is the test that was missing. `update` filled a partial block, then
    /// fell through to code that reset `pending_len` from the remainder of an
    /// already-consumed slice — so every call that did not complete a block threw
    /// away what came before it. One-shot `sha256` calls `update` exactly once,
    /// so published vectors could not see it; `tree_digest` calls it four times
    /// per file, and its own tests failed immediately.
    ///
    /// The split sizes are chosen around the 64-byte block: below it, exactly it,
    /// across it, and a one-byte drip, which is the case that reproduced the bug.
    #[test]
    fn a_split_stream_hashes_like_a_whole_one() {
        let data: Vec<u8> = (0..200u32).map(|i| (i % 251) as u8).collect();
        let whole = sha256(&data);

        for split in [1usize, 7, 63, 64, 65, 127, 128, 199] {
            let mut hasher = Sha256::new();
            for piece in data.chunks(split) {
                hasher.update(piece);
            }
            assert_eq!(
                hasher.finalize(),
                whole,
                "a stream split every {} bytes hashed differently",
                split
            );
        }
    }

    /// The shape `tree_digest` actually uses: many short, uneven updates.
    #[test]
    fn many_short_updates_hash_like_the_concatenation() {
        let parts: [&[u8]; 5] = [b"a/one.sz", &[0], &[6, 0, 0, 0, 0, 0, 0, 0], b"out 1;", b""];
        let mut joined = Vec::new();
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update(part);
            joined.extend_from_slice(part);
        }
        assert_eq!(hasher.finalize(), sha256(&joined));
    }

    /// The empty digest, and a boundary where padding needs a second block.
    ///
    /// A 56-byte message is the first length whose padding no longer fits in its
    /// own block, which is the case `finalize` special-cases.
    #[test]
    fn the_padding_boundary_is_where_the_standard_puts_it() {
        assert_eq!(
            to_hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        for len in [55usize, 56, 57, 63, 64, 119, 120] {
            let data = vec![b'z'; len];
            let mut hasher = Sha256::new();
            hasher.update(&data);
            assert_eq!(
                hasher.finalize(),
                sha256(&data),
                "a {}-byte message padded differently through the two paths",
                len
            );
        }
    }
}
