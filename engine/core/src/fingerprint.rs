//! The fingerprint of a database's build: a hash of the sources its answers come from, core's and
//! the database crate's own. Two engines with the same fingerprint give the same answer, which is
//! how a client decides whether the engine inside a server's `reylo` extension may answer for it,
//! and how Reylo keys its cache of analyses. Each crate's build script hashes only its own
//! directory (`reylo-engine-build`); the database crate combines the two here, at compile time, so
//! a change in core changes every database's fingerprint and a change in one database no other's.

/// The hash of core's sources (`build.rs`).
pub const SOURCE_HASH: u64 = parse_hex(env!("REYLO_ENGINE_CORE_HASH"));

/// A hash a build script wrote as 16 hex digits.
pub const fn parse_hex(text: &str) -> u64 {
    let bytes = text.as_bytes();
    assert!(bytes.len() == 16, "a source hash is 16 hex digits");
    let mut value: u64 = 0;
    let mut i = 0;
    while i < bytes.len() {
        let digit = match bytes[i] {
            b @ b'0'..=b'9' => b - b'0',
            b @ b'a'..=b'f' => b - b'a' + 10,
            _ => panic!("a source hash is 16 lowercase hex digits"),
        };
        value = value << 4 | digit as u64;
        i += 1;
    }
    value
}

/// FNV-1a over the parts' bytes, as 16 lowercase hex digits: the format a fingerprint has always had.
pub const fn combine(parts: &[u64]) -> [u8; 16] {
    let mut hash: u64 = 0xcbf29ce484222325;
    let mut p = 0;
    while p < parts.len() {
        let bytes = parts[p].to_le_bytes();
        let mut i = 0;
        while i < bytes.len() {
            hash ^= bytes[i] as u64;
            hash = hash.wrapping_mul(0x100000001b3);
            i += 1;
        }
        p += 1;
    }
    let mut out = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        let nibble = (hash >> (60 - 4 * i)) & 0xf;
        out[i] = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + nibble as u8 - 10 };
        i += 1;
    }
    out
}

/// The digits [`combine`] wrote, as text.
pub const fn as_str(hex: &'static [u8; 16]) -> &'static str {
    match std::str::from_utf8(hex) {
        Ok(text) => text,
        Err(_) => panic!("combine writes hex digits"),
    }
}

/// A database's fingerprint, from its own source hash: `fingerprint!("REYLO_ENGINE_POSTGRES_HASH")`
/// in the database crate, whose build script set that variable.
#[macro_export]
macro_rules! fingerprint {
    ($var:literal) => {{
        const OWN: u64 = $crate::fingerprint::parse_hex(env!($var));
        const HEX: [u8; 16] = $crate::fingerprint::combine(&[$crate::fingerprint::SOURCE_HASH, OWN]);
        $crate::fingerprint::as_str(&HEX)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerprint_is_sixteen_hex_digits_and_turns_on_every_part() {
        static A: [u8; 16] = combine(&[1, 2]);
        static B: [u8; 16] = combine(&[1, 3]);
        assert_eq!(as_str(&A).len(), 16);
        assert!(as_str(&A).bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        assert_ne!(as_str(&A), as_str(&B));
        assert_eq!(parse_hex("00000000000000ff"), 255);
    }
}
