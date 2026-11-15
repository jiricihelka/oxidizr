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

#[cfg(test)]
mod tests {
    use super::*;

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
