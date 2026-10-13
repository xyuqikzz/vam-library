//! Conservative reference extraction for VaM's non-standard JSON text.
//! This is a reference scanner, not a file repairer: every quoted and bare token
//! is checked, including text after the root and tokens merged by missing commas.
use std::{
    collections::BTreeSet,
    io::Read,
    sync::atomic::{AtomicBool, Ordering},
};

/// Keep broad matches for protecting installed packages, but do not treat every
/// dotted value (game versions, plugin storable IDs) as a missing dependency.
#[derive(Default)]
pub(super) struct References {
    pub candidates: BTreeSet<String>,
    pub packages: BTreeSet<String>,
}

fn collect_id(value: &str, refs: &mut References, explicit: bool) {
    if let Some(id) = canonical_id(value) {
        let version = id.rsplit('.').next().unwrap();
        let package_token = (version == "latest" || version.parse::<u64>().is_ok())
            && !id.bytes().all(|b| b.is_ascii_digit() || b == b'.');
        if explicit || package_token {
            refs.packages.insert(id.clone());
        }
        refs.candidates.insert(id);
    }
}

/// Dependency keys and package-qualified paths are explicit even for numeric
/// names or nonstandard versions; they must not be mistaken for ordinary text.
pub(super) fn collect_dependency(value: &str, refs: &mut References) {
    collect_id(value, refs, true);
    collect_value(value, refs);
}

pub(super) fn canonical_id(value: &str) -> Option<String> {
    // VaM metadata may contain type-qualified keys such as clothing:Creator.Pack.latest.
    let value = value.rsplit(':').next()?;
    let (family, version) = value.rsplit_once('.')?;
    let (creator, name) = family.split_once('.')?;
    if creator.is_empty()
        || name.is_empty()
        || version.is_empty()
        || value.contains(['/', '\\', '"'])
        || value.chars().any(char::is_control)
    {
        return None;
    }
    // Unknown version tokens are kept too; targets() conservatively retains all
    // installed versions of their family rather than guessing which one is used.
    Some(value.to_lowercase())
}

pub(super) fn collect_value(value: &str, refs: &mut References) {
    if value.contains('\u{feff}') {
        collect_value(&value.replace('\u{feff}', ""), refs);
    }
    collect_id(value, refs, false);
    let mut start = 0;
    for (at, _) in value.match_indices(':') {
        collect_id(&value[start..at], refs, true);
        start = at + 1;
    }
    // Credits/comments can contain separate mentions; over-retaining is safer
    // than discarding a reference just because it is outside standard JSON.
    for token in value
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '<' | '>' | '(' | ')' | '"'))
    {
        if token != value {
            collect_id(token, refs, false);
            let mut start = 0;
            for (at, _) in token.match_indices(':') {
                collect_id(&token[start..at], refs, true);
                start = at + 1;
            }
        }
    }
}

fn decode_literal(bytes: &[u8]) -> Result<String, String> {
    let mut encoded = Vec::with_capacity(bytes.len() + 2);
    encoded.push(b'"');
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        i += 1;
        if byte == b'\\' {
            let next = *bytes.get(i).ok_or("引用字符串转义未结束")?;
            if next < 32 {
                return Err("引用字符串包含无法确认的转义".into());
            }
            if !matches!(
                next,
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' | b'u'
            ) {
                // SimpleJSON treats unknown escapes as the escaped character.
                // Leave that character for the next iteration, without the slash.
                continue;
            }
            encoded.push(byte);
            encoded.push(next);
            i += 1;
            continue;
        }
        // Keep literal control characters in this first, conservative variant.
        match byte {
            0..=31 => encoded.extend_from_slice(format!("\\u{byte:04x}").as_bytes()),
            _ => encoded.push(byte),
        }
    }
    encoded.push(b'"');
    serde_json::from_slice(&encoded).map_err(|e| format!("引用字符串无法完整解码: {e}"))
}

pub(super) fn scan(
    mut reader: impl Read,
    refs: &mut References,
    cancel: &AtomicBool,
    allow_unclosed_container: bool,
) -> Result<(), String> {
    let mut chunk = [0; 64 * 1024];
    let mut token = Vec::new();
    let mut literal = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0_usize;
    let mut root_seen = false;
    let mut literal_seen = false;
    let mut token_dirty = false;
    let mut bare_escape = false;
    let collect = |token: &[u8], refs: &mut References| -> Result<(), String> {
        let value = std::str::from_utf8(token).map_err(|_| "引用文本包含无效 UTF-8")?;
        collect_value(value, refs);
        Ok(())
    };
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("操作已取消".into());
        }
        let count = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        for &byte in &chunk[..count] {
            if bare_escape {
                // Outside quotes the game discards the slash and next character,
                // but preserves the surrounding token (which may form an ID).
                if !byte.is_ascii() {
                    return Err("非引号文本包含无法确认的 Unicode 转义".into());
                }
                bare_escape = false;
                continue;
            }
            if quoted {
                if escaped {
                    if byte < 32 {
                        return Err("引用字符串包含无法确认的转义".into());
                    }
                    literal.push(byte);
                    escaped = false;
                } else if byte == b'\\' {
                    literal.push(byte);
                    escaped = true;
                } else if byte == b'"' {
                    let value = decode_literal(&literal)?;
                    collect_value(&value, refs);
                    // The game's parser discards raw CR/LF even inside quotes.
                    // Escaped \\r/\\n keep their meaning, so normalize raw bytes only.
                    let native = if literal.contains(&b'\r') || literal.contains(&b'\n') {
                        decode_literal(
                            &literal
                                .iter()
                                .copied()
                                .filter(|b| !matches!(b, b'\r' | b'\n'))
                                .collect::<Vec<_>>(),
                        )?
                    } else {
                        value
                    };
                    collect_value(&native, refs);
                    token.extend_from_slice(native.as_bytes());
                    token_dirty = true;
                    literal.clear();
                    quoted = false;
                    literal_seen = true;
                } else {
                    literal.push(byte);
                }
            } else {
                match byte {
                    b'"' => {
                        quoted = true;
                        root_seen = true;
                    }
                    b'{' | b'[' => {
                        collect(&token, refs)?;
                        token.clear();
                        token_dirty = false;
                        depth = depth.checked_add(1).ok_or("引用文本层级过深")?;
                        root_seen = true;
                    }
                    b'}' | b']' => {
                        collect(&token, refs)?;
                        token.clear();
                        token_dirty = false;
                        depth = depth.checked_sub(1).ok_or("引用文本存在多余结束括号")?;
                    }
                    b':' | b',' | b'/' => {
                        // A key or package path qualifier is explicit. Numeric
                        // values at commas/container ends remain ordinary text.
                        if byte == b':' {
                            let value = std::str::from_utf8(&token)
                                .map_err(|_| "引用文本包含无效 UTF-8")?;
                            collect_dependency(value, refs);
                        }
                        collect(&token, refs)?;
                        token.clear();
                        token_dirty = false;
                    }
                    b' ' | b'\t' | b'\r' | b'\n' => {
                        if token_dirty {
                            collect(&token, refs)?;
                            token_dirty = false;
                        }
                    }
                    b'\\' => bare_escape = true,
                    _ => {
                        token.push(byte);
                        token_dirty = true;
                    }
                }
            }
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("操作已取消".into());
    }
    if quoted || escaped || bare_escape {
        return Err("引用文本字符串未结束".into());
    }
    collect(&token, refs)?;
    if !root_seen || (depth > 0 && (!allow_unclosed_container || !literal_seen)) {
        return Err("引用文本不完整，无法确认全部引用".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn refs(input: &[u8], metadata: bool) -> Result<BTreeSet<String>, String> {
        let mut refs = References::default();
        scan(input, &mut refs, &AtomicBool::new(false), metadata)?;
        Ok(refs.candidates)
    }
    #[test]
    fn package_evidence_distinguishes_plain_text_from_explicit_references() {
        let mut found = References::default();
        for value in ["1.22.0.3", "plugin#0_MacGruber.PostMagic.DepthOfField"] {
            collect_value(value, &mut found);
            assert!(found.candidates.contains(&value.to_lowercase()));
        }
        assert!(found.packages.is_empty());
        for value in ["1.22.0.3:/Custom/x", "A.Pack.unknown:/x", "Author.Pack.1", "Author.Pack.latest"] {
            collect_value(value, &mut found);
        }
        collect_dependency("clothing:123.456.1", &mut found);
        assert_eq!(found.packages, BTreeSet::from([
            "1.22.0.3".into(), "a.pack.unknown".into(), "author.pack.1".into(),
            "author.pack.latest".into(), "123.456.1".into(),
        ]));
    }
    #[test]
    fn compatible_scan_keeps_explicit_numeric_references_without_promoting_versions() {
        let mut found = References::default();
        scan(&br#"{"version":"1.22.0.3","id":"plugin#0_MacGruber.PostMagic.DepthOfField",}"#[..],
            &mut found, &AtomicBool::new(false), true).unwrap();
        assert!(found.packages.is_empty());
        scan(&br#"{"dependencies":{"1.22.0.3":{}} "path":"123.456.1:/x"}"#[..],
            &mut found, &AtomicBool::new(false), true).unwrap();
        assert!(found.packages.contains("1.22.0.3"));
        assert!(found.packages.contains("123.456.1"));
    }
    #[test]
    fn missing_commas_tail_credits_and_type_prefixes_preserve_references() {
        let found = refs(br#"{"dependencies":{"clothing:A.Scene.latest":{} "B.Scene.1":{}}} Thanks:"C.Scene.2:/file""#, true).unwrap();
        for id in ["a.scene.latest", "b.scene.1", "c.scene.2"] {
            assert!(found.contains(id), "{id}");
        }
    }
    #[test]
    fn escaped_keys_and_strings_are_decoded_before_reference_matching() {
        let found = refs(
            br#"{"dependencies":{"\u0041.Scene.1":{}} "asset":"B.\u0053cene.2\u003a/file"}"#,
            true,
        )
        .unwrap();
        assert!(found.contains("a.scene.1"));
        assert!(found.contains("b.scene.2"));
    }
    #[test]
    fn malformed_shapes_never_turn_declared_dependency_arrays_into_empty_sets() {
        assert!(refs(br#"{"dependencies":["A.Scene.1"]}"#, true)
            .unwrap()
            .contains("a.scene.1"));
        assert!(refs(br#"{"asset":"A.Scene.1:/file"]"#, false)
            .unwrap()
            .contains("a.scene.1"));
        assert!(refs(br#"{"dependencies":{"A.Scene.1":{}}"#, true)
            .unwrap()
            .contains("a.scene.1"));
    }
    #[test]
    fn unreadable_escapes_strings_encoding_and_truncated_local_files_stay_blocked() {
        for input in [
            &b"{"[..],
            &b"{\"asset\":\"A.Scene.1:/file}"[..],
            &b"{\"ref\":\"\\uZZZZ\"}"[..],
            &b"{\"ref\":\"\xff\"}"[..],
        ] {
            assert!(refs(input, false).is_err());
        }
    }
    #[test]
    fn concatenated_tokens_are_kept_in_addition_to_individual_literals() {
        let found = refs(br#"{"asset":"A.Scene.1""B.Scene.2:/file"}"#, false).unwrap();
        assert!(found.contains("a.scene.1b.scene.2"));
        assert!(found.contains("b.scene.2"));
    }
    #[test]
    fn raw_newlines_are_ignored_like_the_game_but_escaped_newlines_are_preserved() {
        assert!(refs(b"{\"asset\":\"A.Sce\rne.1:/file\"}", false)
            .unwrap()
            .contains("a.scene.1"));
        assert!(!refs(br#"{"asset":"A.Sce\rne.1:/file"}"#, false)
            .unwrap()
            .contains("a.scene.1"));
    }
    #[test]
    fn game_escape_rules_preserve_references_in_quoted_and_bare_tokens() {
        let found = refs(
            br#"{"asset":"A.Sc\ene.1:/file" "bare":B.Sce\qne.2:/file}"#,
            false,
        )
        .unwrap();
        assert!(found.contains("a.scene.1"));
        assert!(found.contains("b.scene.2"));
    }
}
