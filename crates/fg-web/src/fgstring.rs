//! The texts copied between the web app and the addon: a prefix telling what it holds, then
//! the content compressed (raw DEFLATE) and written with LibDeflate's printable alphabet
//! (`EncodeForPrint`), which the addon decodes with LibDeflate.

/// A guide import (web app -> game): the Lua the app used to write in the addon's files.
pub const GUIDE_PREFIX: &str = "FGG1:";
/// A character's export (game -> web app): its profile and recordings, as JSON.
pub const PROFILE_PREFIX: &str = "FGP1:";

/// Exports bigger than this once decompressed are refused (a profile with its recordings
/// takes a few megabytes at most).
const MAX_DECODED: usize = 64 << 20;

const ALPHABET: &[u8; 64] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789()";

/// `prefix` and `data` compressed and encoded.
pub fn encode(prefix: &str, data: &[u8]) -> String {
    let packed = miniz_oxide::deflate::compress_to_vec(data, 9);
    let mut out = String::with_capacity(prefix.len() + packed.len() * 4 / 3 + 4);
    out.push_str(prefix);
    encode_for_print(&packed, &mut out);
    out
}

/// What a text made by `encode` with `prefix` holds.
pub fn decode(prefix: &str, text: &str) -> Result<Vec<u8>, String> {
    let text = text.trim();
    let body = text.strip_prefix(prefix).ok_or("not_this_kind")?;
    let packed = decode_for_print(body).ok_or("corrupt")?;
    miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, MAX_DECODED).map_err(|_| "corrupt".to_owned())
}

/// LibDeflate's `EncodeForPrint`: 3 bytes as 4 characters of 6 bits, little-endian, the low
/// bits first; the last 1 or 2 bytes as just enough characters.
fn encode_for_print(data: &[u8], out: &mut String) {
    let char_of = |v: u32| char::from(ALPHABET[(v & 63) as usize]);
    let mut chunks = data.chunks_exact(3);
    for c in &mut chunks {
        let v = u32::from(c[0]) | u32::from(c[1]) << 8 | u32::from(c[2]) << 16;
        for shift in [0, 6, 12, 18] {
            out.push(char_of(v >> shift));
        }
    }
    let rest = chunks.remainder();
    let mut cache = 0u32;
    let mut bits = 0i32;
    for (i, &b) in rest.iter().enumerate() {
        cache |= u32::from(b) << (8 * i);
        bits += 8;
    }
    while bits > 0 {
        out.push(char_of(cache));
        cache >>= 6;
        bits -= 6;
    }
}

/// LibDeflate's `DecodeForPrint` (None: a character not of the alphabet).
fn decode_for_print(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| ALPHABET.iter().position(|&a| a == c).map(|v| v as u32);
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4 + 2);
    let mut chunks = bytes.chunks_exact(4);
    for c in &mut chunks {
        let v = value(c[0])? | value(c[1])? << 6 | value(c[2])? << 12 | value(c[3])? << 18;
        out.extend_from_slice(&[v as u8, (v >> 8) as u8, (v >> 16) as u8]);
    }
    let mut cache = 0u32;
    let mut bits = 0;
    for &c in chunks.remainder() {
        cache |= value(c)? << bits;
        bits += 6;
        while bits >= 8 {
            out.push(cache as u8);
            cache >>= 8;
            bits -= 8;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_encoded_decodes_whatever_its_length() {
        for len in 0..40 {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 % 256) as u8).collect();
            let text = encode(GUIDE_PREFIX, &data);
            assert!(text[GUIDE_PREFIX.len()..].bytes().all(|b| ALPHABET.contains(&b)));
            assert_eq!(decode(GUIDE_PREFIX, &format!("  {text}\n")).unwrap(), data);
        }
    }

    /// The printable form of LibDeflate: "abc" is 3 bytes, 4 characters.
    #[test]
    fn printable_form_matches_libdeflate() {
        let mut out = String::new();
        encode_for_print(b"\x00\x00\x00", &mut out);
        assert_eq!(out, "aaaa");
        out.clear();
        encode_for_print(b"\xff\xff\xff", &mut out);
        assert_eq!(out, "))))");
        out.clear();
        encode_for_print(b"\x01", &mut out);
        assert_eq!(out, "ba");
        assert_eq!(decode_for_print("ba").unwrap(), b"\x01");
    }

    #[test]
    fn another_kind_or_a_damaged_text_is_refused() {
        let text = encode(PROFILE_PREFIX, b"{}");
        assert_eq!(decode(GUIDE_PREFIX, &text).unwrap_err(), "not_this_kind");
        assert_eq!(decode(PROFILE_PREFIX, &format!("{text}!")).unwrap_err(), "corrupt");
        assert_eq!(decode(PROFILE_PREFIX, "FGP1:abcdabcd").unwrap_err(), "corrupt");
    }
}
