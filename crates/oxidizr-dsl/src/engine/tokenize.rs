//! Bug-compatible port of the reference `Tokenizer.java`, operating on the
//! joined logical-line text. Kept separate from [`crate::lexer`] on purpose:
//! the CST lexer tracks spans and strips quote characters from pieces, while
//! the reference tokenizer operates on the already-joined text and leaves
//! `\`-escapes in the token values for `Tokens.get(i)` to unescape.
//!
//! Quirks replicated (verified against `Tokenizer.java` @ `ad34036`):
//!
//! * The escaped-quote test is a RAW LOOKBEHIND AT THE LINE, not the token:
//!   `c == '"' && line.charAt(i - 1) == '\\'` — reached for any character
//!   (quoted or bare), so a `\` before a quote in a BARE token also swallows
//!   that quote into the token. The backslash stays in the value.
//! * A `"` opens a quoted token only at token start; inside a quoted token
//!   it closes and FLUSHES (quote chars are dropped from the value).
//! * Inside a bare token a `"` is an ordinary character (`a"b` is one token).
//! * A bare token flushes on Java whitespace; a quoted token does not.
//! * End of text flushes an open token; an unterminated quoted token is NOT
//! * an error at this level, and `""` flushes an EMPTY token.

/// `Tokenizer.tokenize(line.trim())` — returns the raw token values (escape
/// sequences still intact; unescape per `Tokens::get`).
#[must_use]
pub fn tokenize(line: &str) -> Vec<String> {
    let line = crate::compat::java_trim(line);
    let mut tokens: Vec<String> = Vec::new();

    let mut token_started = false;
    let mut quoted = false;
    let mut token = String::new();

    // The reference indexes the ORIGINAL line (`line.charAt(i - 1)`), so
    // the lookbehind is done on the trimmed line bytes, not the token text.
    let bytes = line.as_bytes();
    for (i, c) in line.char_indices() {
        if !token_started {
            if c == '"' {
                quoted = true;
                token_started = true;
                token.clear();
            } else if crate::compat::is_java_whitespace(c) {
                // skip
            } else {
                quoted = false;
                token_started = true;
                token.clear();
                token.push(c);
            }
        } else if c == '"' && i > 0 && bytes[i - 1] == b'\\' {
            // escaped quote — appended regardless of quoted/bare
            token.push(c);
        } else if quoted && c == '"' {
            tokens.push(std::mem::take(&mut token));
            token_started = false;
            quoted = false;
        } else if !quoted && crate::compat::is_java_whitespace(c) {
            tokens.push(std::mem::take(&mut token));
            token_started = false;
        } else {
            token.push(c);
        }
    }

    if token_started {
        tokens.push(token);
    }

    tokens
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn t(line: &str) -> Vec<String> {
        tokenize(line)
    }

    #[test]
    fn basic_quoting() {
        assert_eq!(t(" a  b \"c d\"  "), ["a", "b", "c d"]);
        assert_eq!(t(r#"a"b"#), [r#"a"b"#]);
        // `""` closes an empty quoted token — the reference flushes it
        assert_eq!(t("\"\""), [""]);
    }

    #[test]
    fn escaped_quote_stays_with_backslash() {
        // value keeps the backslash; Tokens::get unescapes it
        assert_eq!(t(r#""a\"b""#), [r#"a\"b"#]);
        assert_eq!(t(r#""a\nb""#), [r"a\nb"]);
    }

    #[test]
    fn unterminated_quoted_token_flushes() {
        assert_eq!(t("\"abc"), ["abc"]);
    }

    #[test]
    fn backslash_before_closing_quote_is_literal() {
        // the final `"` is escaped, so the token stays open to EOL
        assert_eq!(t(r#""ab\""#), [r#"ab\""#]);
    }

    #[test]
    fn quoted_token_survives_inner_whitespace() {
        assert_eq!(
            t(r#"name "one two" "three four""#),
            ["name", "one two", "three four"]
        );
    }

    #[test]
    fn bare_token_swallows_backslash_quote() {
        // reference quirk: the line lookbehind fires in bare tokens too
        assert_eq!(t(r#"a\"b c"#), [r#"a\"b"#, "c"]);
    }
}
