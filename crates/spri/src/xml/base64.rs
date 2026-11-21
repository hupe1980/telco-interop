//! `xsd:base64Binary` (RFC 2045 alphabet, padded), without a dependency.
//!
//! Decoding ignores XML whitespace (line breaks are common in base64 bodies) and refuses
//! anything else outside the alphabet, misplaced padding and non-zero padding bits.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes `bytes` on one line.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk.first().copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                let idx = usize::try_from((n >> shift) & 63).unwrap_or(0);
                out.push(char::from(ALPHABET.get(idx).copied().unwrap_or(b'A')));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn value(c: u8) -> Option<u32> {
    let v = match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(u32::from(v))
}

/// Why base64 text does not decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Base64Error {
    /// Not base64.
    Invalid,
    /// Decodes to more than the limit.
    TooLarge,
}

/// Decodes `text`, refusing output larger than `max` bytes.
pub(crate) fn decode(text: &str, max: usize) -> Result<Vec<u8>, Base64Error> {
    let symbols: Vec<u8> = text.bytes().filter(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r')).collect();
    if !symbols.len().is_multiple_of(4) {
        return Err(Base64Error::Invalid);
    }
    if symbols.len() / 4 * 3 > max.saturating_add(2) {
        return Err(Base64Error::TooLarge);
    }
    let mut out = Vec::with_capacity(symbols.len() / 4 * 3);
    let groups = symbols.len() / 4;
    for (g, quad) in symbols.chunks(4).enumerate() {
        let last = g + 1 == groups;
        let pad = quad.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return Err(Base64Error::Invalid);
        }
        let mut n = 0u32;
        for (i, &c) in quad.iter().enumerate() {
            let v = if i >= 4 - pad { 0 } else { value(c).ok_or(Base64Error::Invalid)? };
            n = (n << 6) | v;
        }
        let [_, b0, b1, b2] = n.to_be_bytes();
        let bytes = [b0, b1, b2];
        let keep = 3 - pad;
        // The bits after the last byte must be zero (the canonical form XSD admits).
        let spare = match pad {
            1 => n & 0xFF,
            2 => n & 0xFFFF,
            _ => 0,
        };
        if spare != 0 {
            return Err(Base64Error::Invalid);
        }
        out.extend(bytes.iter().take(keep));
    }
    if out.len() > max {
        return Err(Base64Error::TooLarge);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for len in 0..40 {
            let bytes: Vec<u8> = (0..len).map(|i: u32| u8::try_from((i * 37 + 11) % 256).unwrap()).collect();
            let text = encode(&bytes);
            assert_eq!(decode(&text, 1000), Ok(bytes), "{len}");
        }
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(encode(b"M"), "TQ==");
    }

    #[test]
    fn whitespace_is_ignored_and_errors_are_refused() {
        assert_eq!(decode("TW\nFu\r\n TWE=", 100), Ok(b"ManMa".to_vec()));
        for bad in ["TWF", "TW=u", "TQ==TWFu", "T===", "TWF*", "TR==", "TWF="] {
            assert_eq!(decode(bad, 100), Err(Base64Error::Invalid), "{bad}");
        }
    }

    #[test]
    fn size_is_bounded() {
        assert_eq!(decode("TWFu", 3), Ok(b"Man".to_vec()));
        assert_eq!(decode("TWFuTWFu", 5), Err(Base64Error::TooLarge));
        assert_eq!(decode("TWFuTWE=", 5), Ok(b"ManMa".to_vec()));
        assert_eq!(decode(&"AAAA".repeat(1000), 3), Err(Base64Error::TooLarge));
    }
}
