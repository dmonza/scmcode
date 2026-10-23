//! scmCode: minimal percent-decoding, a clean-room replacement for the
//! `urlencoding` crate so the app carries one less direct dependency.
//!
//! Only `%XX` escapes are decoded. `+` is left untouched (it is a
//! query-string convention, not path encoding), matching the single caller in
//! `lib.rs`, which decodes the path of a custom-protocol request before it is
//! canonicalized and security-checked. The security-relevant invariant is kept
//! unchanged: a sequence that does not decode to valid UTF-8 yields `Err`, so
//! the caller rejects the request instead of acting on raw bytes.

use std::borrow::Cow;
use std::str::Utf8Error;

/// Value of a single ASCII hex digit, or `None` if it is not one.
fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode `input`.
///
/// When the input has no `%`, it is returned borrowed with no allocation. A `%`
/// not followed by two hex digits is kept literally (lenient decode). Returns
/// `Err` when the decoded bytes are not valid UTF-8.
pub fn decode(input: &str) -> Result<Cow<'_, str>, Utf8Error> {
    let bytes = input.as_bytes();
    if !bytes.contains(&b'%') {
        return Ok(Cow::Borrowed(input));
    }

    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let (Some(hi), Some(lo)) = (
                bytes.get(i + 1).copied().and_then(hex_val),
                bytes.get(i + 2).copied().and_then(hex_val),
            ) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }

    match String::from_utf8(out) {
        Ok(s) => Ok(Cow::Owned(s)),
        Err(e) => Err(e.utf8_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrows_when_nothing_to_decode() {
        let decoded = decode("/home/user/file.txt").unwrap();
        assert!(matches!(decoded, Cow::Borrowed(_)));
        assert_eq!(decoded, "/home/user/file.txt");
    }

    #[test]
    fn decodes_basic_escapes() {
        assert_eq!(decode("%20").unwrap(), " ");
        assert_eq!(decode("a%2Fb").unwrap(), "a/b");
        assert_eq!(decode("%7e").unwrap(), "~");
    }

    #[test]
    fn decodes_encoded_traversal() {
        // The decoder must faithfully reveal an encoded traversal so the
        // downstream canonicalize/allow-list check can reject it.
        assert_eq!(decode("%2e%2e%2fetc").unwrap(), "../etc");
    }

    #[test]
    fn hex_is_case_insensitive() {
        assert_eq!(decode("%2F").unwrap(), decode("%2f").unwrap());
    }

    #[test]
    fn plus_is_not_a_space() {
        assert_eq!(decode("a+b").unwrap(), "a+b");
    }

    #[test]
    fn malformed_escapes_are_literal() {
        assert_eq!(decode("%2").unwrap(), "%2");
        assert_eq!(decode("%zz").unwrap(), "%zz");
        assert_eq!(decode("100%").unwrap(), "100%");
    }

    #[test]
    fn multibyte_utf8_round_trips() {
        assert_eq!(decode("%C3%A9").unwrap(), "é");
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        assert!(decode("%ff%fe").is_err());
    }
}
