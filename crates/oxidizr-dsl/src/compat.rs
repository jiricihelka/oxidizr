//! Java-`char`/`String` semantics the reference lexer depends on. Full fidelity for
//! ASCII (where DSL files live); Unicode divergences documented per function.

/// `Character.isWhitespace` for the tokenizer's purposes: ASCII `\t\n\x0B\f\r` and
/// space. Divergence: Java's Unicode whitespace beyond ASCII (e.g. U+2028) also
/// splits tokens there but not here; NBSP is NOT whitespace in Java either, so the
/// common cases match. Recorded in agents/plan/PROGRESS.md.
#[must_use]
pub fn is_java_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r')
}

/// Java regex `\s` (`[ \t\n\x0B\f\r]`) — same ASCII set as
/// [`is_java_whitespace`]; separate name because the reference uses the two in
/// different places (patterns vs tokenizer).
#[must_use]
pub fn is_java_regex_s(c: char) -> bool {
    is_java_whitespace(c)
}

/// `String.trim` semantics: strip leading/trailing code points `<= U+0020`.
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

/// Strip `n` CHARACTERS from the front, mirroring `StringBuilder.append(s, n, len)`:
/// exact `n == char_count` yields the empty string; `n > char_count` is the
/// reference's `IndexOutOfBoundsException` case and maps to `None`.
#[must_use]
pub fn strip_prefix_chars(s: &str, n: usize) -> Option<&str> {
    match s.char_indices().nth(n) {
        Some((byte, _)) => Some(&s[byte..]),
        None if s.chars().count() == n => Some(""),
        None => None,
    }
}

/// Java `Integer.parseInt`: optional `+`/`-` sign, ASCII digits only, `i32`
/// range; `None` is the `NumberFormatException` case.
///
/// Divergence (recorded in `agents/plan/PROGRESS.md` errata list): Java's
/// `Character.digit` also accepts non-ASCII decimal digits (e.g. Arabic-Indic
/// `٣`); those throw here. Style values in real workspaces are ASCII.
#[must_use]
pub fn java_parse_int(s: &str) -> Option<i32> {
    s.parse::<i32>().ok()
}

/// `String.equalsIgnoreCase`: length in UTF-16 code units must match (Rust
/// compares scalar values, so astral-vs-surrogate divergences are the only
/// theoretical difference), then per-character
/// `toUpperCase(a) == toUpperCase(b) || toLowerCase(a) == toLowerCase(b)`.
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

/// `String.split("\\r?\\n")` — the reference parser's line split
/// (`StructurizrDslParser.parse(String)`), used for the retained-DSL source
/// lines (M1-e): one `\r` directly before each `\n` belongs to the
/// separator, and Java's limit-0 `split` drops ALL trailing empty strings —
/// a file ending in a newline never retains one, and trailing blank lines
/// vanish. `""` keeps Java's documented single-empty-element result.
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
        // Final newline: Java drops the trailing empty entry (the M1-e A/B
        // finding — the reference's retained DSL never carries it).
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
        // Interior empties stay; all-empty input reduces to Java's [].
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
        // NBSP (U+00A0) is NOT trimmed by Java trim, and is not whitespace for the
        // tokenizer either:
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
