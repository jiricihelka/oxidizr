//! Character/string semantics as the official tooling defines them, which
//! the lexer and parser depend on. Full fidelity for ASCII (where DSL files
//! live); Unicode divergences documented per function.

/// Whitespace for the tokenizer's purposes: ASCII `\t\n\x0B\f\r` and space.
/// Divergence: Unicode whitespace beyond ASCII (e.g. U+2028) splits tokens in
/// the official tooling but not here; NBSP is not whitespace there either, so
/// the common cases match.
#[must_use]
pub fn is_java_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

/// The official tooling's regex `\s` set (`[ \t\n\x0B\f\r]`) — same ASCII set
/// as [`is_java_whitespace`]; separate name because the official tooling uses
/// the two in different places (line patterns vs tokenizer).
#[must_use]
pub fn is_java_regex_s(c: char) -> bool {
    is_java_whitespace(c)
}

/// Trim as the official tooling defines it: strip leading/trailing code
/// points `<= U+0020`.
#[must_use]
pub fn java_trim(s: &str) -> &str {
    let start = s
        .char_indices()
        .find(|(_, c)| (u32::from(*c)) > 0x20)
        .map_or(s.len(), |(i, _)| i);
    let mut end = s.len();
    while end > start {
        let ch = s[start..end].chars().next_back().unwrap_or('\0');
        if (u32::from(ch)) > 0x20 {
            break;
        }
        end -= ch.len_utf8();
    }
    &s[start..end]
}

/// Strip `n` CHARACTERS from the front: exact `n == char_count` yields the
/// empty string; `n > char_count` is the official tooling's
/// index-out-of-bounds case and maps to `None`.
#[must_use]
pub fn strip_prefix_chars(s: &str, n: usize) -> Option<&str> {
    match s.char_indices().nth(n) {
        Some((byte, _)) => Some(&s[byte..]),
        None if s.chars().count() == n => Some(""),
        None => None,
    }
}

/// Integer parsing as the official tooling defines it: optional `+`/`-`
/// sign, ASCII digits only, `i32` range; `None` is the rejected case.
///
/// Divergence: the official tooling also accepts non-ASCII decimal digits
/// (e.g. Arabic-Indic `٣`); those fail here. Style values in real workspaces
/// are ASCII.
#[must_use]
pub fn java_parse_int(s: &str) -> Option<i32> {
    s.parse::<i32>().ok()
}

/// Case-insensitive string equality as the official tooling defines it:
/// length in UTF-16 code units must match (Rust compares scalar values, so
/// astral-vs-surrogate divergences are the only theoretical difference), then
/// per-character upper-or-lowercase equality.
#[must_use]
pub fn java_eq_ignore_case(a: &str, b: &str) -> bool {
    let mut ia = a.chars();
    let mut ib = b.chars();
    loop {
        let (x, y) = match (ia.next(), ib.next()) {
            (Some(x), Some(y)) => (x, y),
            (None, None) => return true,
            _ => return false,
        };
        if x == y {
            continue;
        }
        let up = x.to_uppercase().eq(y.to_uppercase());
        let low = x.to_lowercase().eq(y.to_lowercase());
        if !(up || low) {
            return false;
        }
    }
}

/// The official tooling's line split, used for the retained-DSL source
/// lines: one `\r` directly before each `\n` belongs to the separator, and
/// ALL trailing empty strings are dropped — a file ending in a newline never
/// retains one, and trailing blank lines vanish. The empty string splits to
/// a single empty element.
#[must_use]
pub fn java_split_dsl_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    for line in &mut lines {
        *line = line.strip_suffix('\r').unwrap_or(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.into_iter().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_split_dsl_lines_mirrors_regex_split() {
        // No final newline: plain split.
        assert_eq!(
            java_split_dsl_lines("a\nb"),
            ["a".to_string(), "b".to_string()]
        );
        // Final newline: the trailing empty entry is dropped (the official
        // tooling's retained DSL never carries it).
        assert_eq!(
            java_split_dsl_lines("a\nb\n"),
            ["a".to_string(), "b".to_string()]
        );
        // ALL trailing empties go: blank lines at EOF vanish too.
        assert_eq!(java_split_dsl_lines("a\n\n\n"), ["a".to_string()]);
        // `\r\n` consumed as one separator; a lone `\r` before the `\n` pair
        // stays in the line (the regex eats at most one).
        assert_eq!(
            java_split_dsl_lines("a\r\nb"),
            ["a".to_string(), "b".to_string()]
        );
        assert_eq!(java_split_dsl_lines("a\r\r\n"), ["a\r".to_string()]);
        // Interior empties stay; all-empty input reduces to [].
        assert_eq!(
            java_split_dsl_lines("a\n\nb\n\n"),
            ["a".to_string(), String::new(), "b".to_string()]
        );
        assert_eq!(java_split_dsl_lines("\n"), Vec::<String>::new());
        assert_eq!(java_split_dsl_lines(""), [String::new()]);
    }

    #[test]
    fn trim_matches_java_control_boundary() {
        assert_eq!(java_trim("  \t x \r "), "x");
        assert_eq!(java_trim("\u{0}x\u{1F}"), "x");
        // NBSP (U+00A0) is NOT trimmed, and is not whitespace for the
        // tokenizer either (same as the official tooling):
        assert_eq!(java_trim("\u{A0}x"), "\u{A0}x");
        assert!(!is_java_whitespace('\u{A0}'));
    }

    #[test]
    fn strip_prefix_on_char_boundary() {
        assert_eq!(strip_prefix_chars("  ab", 2), Some("ab"));
        assert_eq!(strip_prefix_chars("é x", 2), Some("x"));
        assert_eq!(strip_prefix_chars("ab", 2), Some(""));
        assert_eq!(strip_prefix_chars("ab", 3), None);
        assert_eq!(strip_prefix_chars("", 0), Some(""));
    }
}
