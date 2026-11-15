//! Port of the reference `Tokens` wrapper (the `Tokenizer` lives in
//! [`crate::lexer`]). Every `get(i)` re-applies the reference's two
//! un-escape passes — `\"` → `"` then `\n` → newline, in that order, on the
//! result of the first (erratum 26).

/// `Tokens.java`: a raw token list (already `${}`-substituted) with index
/// helpers matching the Java semantics exactly.
#[derive(Debug, Clone)]
pub struct Tokens {
    raw: Vec<String>,
}

impl Tokens {
    /// Wrap a raw token list.
    #[must_use]
    pub fn new(raw: Vec<String>) -> Self {
        Tokens { raw }
    }

    /// `get(index)` — unescaped view of the raw token.
    #[must_use]
    pub fn get(&self, index: usize) -> String {
        // Java: replaceAll("\\\\\"", "\"") then replaceAll("\\\\n", "\n"):
        // literal backslash-quote first, then literal backslash-n.
        self.raw[index].replace(r#"\""#, "\"").replace(r"\n", "\n")
    }

    /// Raw (still escaped) token — used only by the dispatch chain tests and
    /// `without_context_start_token`, which Java compares RAW (`tokens
    /// .get(size-1)` is the unescaped view; the brace token has no escapes,
    /// so both agree).
    #[must_use]
    pub fn raw_token(&self, index: usize) -> &str {
        &self.raw[index]
    }

    /// `size()`.
    #[must_use]
    pub fn size(&self) -> usize {
        self.raw.len()
    }

    /// True when there is no token.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// `includes(index)`.
    #[must_use]
    pub fn includes(&self, index: usize) -> bool {
        self.raw.len() > index
    }

    /// `hasMoreThan(index)`.
    #[must_use]
    pub fn has_more_than(&self, index: usize) -> bool {
        self.includes(index + 1)
    }

    /// The last token (unescaped view), if any — `tokens.get(size()-1)`.
    #[must_use]
    pub fn last(&self) -> Option<String> {
        self.raw.last().map(|_| self.get(self.raw.len() - 1))
    }

    /// `subList(fromIndex, size())`.
    #[must_use]
    pub fn sublist(&self, from: usize) -> Tokens {
        Tokens {
            raw: self.raw[from.min(self.raw.len())..].to_vec(),
        }
    }

    /// `withoutContextStartToken()` — drop a trailing `{` (compared with
    /// `equals`, i.e. case-sensitive).
    #[must_use]
    pub fn without_context_start_token(&self) -> Tokens {
        if self.raw.last().is_some_and(|t| t == "{") {
            Tokens {
                raw: self.raw[..self.raw.len() - 1].to_vec(),
            }
        } else {
            self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_unescapes_quote_before_newline() {
        // \" -> " happens FIRST, then \n -> newline (sequential passes).
        let t = Tokens::new(vec![r#"a\"b\nc"#.to_string()]);
        assert_eq!(t.get(0), "a\"b\nc");
    }

    #[test]
    fn without_context_start_token_drops_trailing_brace() {
        let t = Tokens::new(vec!["a".into(), "{".into()]);
        assert_eq!(t.without_context_start_token().size(), 1);
        let t2 = Tokens::new(vec!["a".into()]);
        assert_eq!(t2.without_context_start_token().size(), 1);
        // case-sensitive equals:
        let t3 = Tokens::new(vec!["a".into(), "{ ".into()]);
        assert_eq!(t3.without_context_start_token().size(), 2);
    }

    #[test]
    fn includes_and_has_more_than() {
        let t = Tokens::new(vec!["a".into(), "b".into()]);
        assert!(t.includes(1));
        assert!(!t.includes(2));
        assert!(t.has_more_than(0));
        assert!(!t.has_more_than(1));
    }
}
