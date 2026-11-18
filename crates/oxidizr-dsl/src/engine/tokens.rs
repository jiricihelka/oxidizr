//! Port of the reference `Tokens` wrapper (the `Tokenizer` lives in
//! [`crate::lexer`]). Every `get(i)` re-applies the reference's two
//! un-escape passes — `\"` → `"` then `\n` → newline, in that order, on the
//! result of the first (erratum 26).
//!
//! Since 2R R-dsl-01 the backing list is a [`Cow`] slice: `Stmt` owns the
//! real `Vec`, while trimmed views (`without_context_start_token`,
//! `sublist`) borrow instead of deep-copying — the reference passes the
//! same `Tokens` reference to every `else if`, which the owned-only port
//! paid for with ~12 full token-list copies per logical line.

use std::borrow::Cow;

/// `Tokens.java`: a raw token list (already `${}`-substituted) with index
/// helpers matching the Java semantics exactly.
#[derive(Debug, Clone)]
pub struct Tokens<'a> {
    raw: Cow<'a, [String]>,
}

impl<'a> Tokens<'a> {
    /// Wrap an owned raw token list.
    #[must_use]
    pub fn new(raw: Vec<String>) -> Tokens<'static> {
        Tokens {
            raw: Cow::Owned(raw),
        }
    }

    /// Borrowed view of an existing raw token slice.
    #[must_use]
    pub fn borrowed(raw: &'a [String]) -> Tokens<'a> {
        Tokens {
            raw: Cow::Borrowed(raw),
        }
    }

    /// The reference's two un-escape passes on one raw token — factored out
    /// of [`Self::get`] for callers holding a bare `&str`.
    /// Java: replaceAll("\\\\\"", "\"") then replaceAll("\\\\n", "\n"):
    /// literal backslash-quote first, then literal backslash-n.
    #[must_use]
    pub fn unescape(raw: &str) -> Cow<'_, str> {
        // Fast path: without a backslash neither pattern can match, so both
        // passes are no-ops. With any backslash run BOTH replaces, keeping
        // the erratum-26 order untouched.
        if raw.contains('\\') {
            Cow::Owned(raw.replace(r#"\""#, "\"").replace(r"\n", "\n"))
        } else {
            Cow::Borrowed(raw)
        }
    }

    /// `get(index)` — unescaped view of the raw token. Borrowed whenever the
    /// token carries no escape (2R R-dsl-02: the old shape allocated two
    /// `String`s per read on a path read several times per logical line).
    #[must_use]
    pub fn get(&self, index: usize) -> Cow<'_, str> {
        Tokens::unescape(&self.raw[index])
    }

    /// Java `tokens.get(i).equals(other)` — the compare-only sites, without
    /// materializing the unescaped view (2R R-dsl-02).
    #[must_use]
    pub fn eq_at(&self, index: usize, other: &str) -> bool {
        Tokens::unescape(&self.raw[index]) == other
    }

    /// Raw (still escaped) token — used only by the dispatch chain tests and
    /// `without_context_start_token`, which Java compares RAW (`tokens
    /// .get(size-1)` is the unescaped view; the brace token has no escapes,
    /// so both agree).
    #[must_use]
    pub fn raw_token(&self, index: usize) -> &str {
        &self.raw[index]
    }

    /// The underlying raw token slice (escaped).
    #[must_use]
    pub fn raw_tokens(&self) -> &[String] {
        &self.raw
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
    pub fn last(&self) -> Option<Cow<'_, str>> {
        self.raw.last().map(|t| Tokens::unescape(t))
    }

    /// `subList(fromIndex, size())` — borrowed view (2R R-dsl-01).
    #[must_use]
    pub fn sublist(&self, from: usize) -> Tokens<'_> {
        Tokens {
            raw: Cow::Borrowed(&self.raw[from.min(self.raw.len())..]),
        }
    }

    /// `withoutContextStartToken()` — drop a trailing `{` (compared with
    /// `equals`, i.e. case-sensitive). Borrowed view either way; the old
    /// `self.clone()` fallback deep-copied every token of every line.
    #[must_use]
    pub fn without_context_start_token(&self) -> Tokens<'_> {
        if self.raw.last().is_some_and(|t| t == "{") {
            Tokens {
                raw: Cow::Borrowed(&self.raw[..self.raw.len() - 1]),
            }
        } else {
            Tokens {
                raw: Cow::Borrowed(&self.raw),
            }
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
    fn unescape_fast_path_matches_slow_path() {
        // No backslash: borrowed pass-through equals the double replace.
        for raw in ["plain", "a=b", "", "ends:"] {
            assert_eq!(Tokens::unescape(raw), raw);
        }
        // Backslash present but neither pattern: replaces are no-ops.
        assert_eq!(Tokens::unescape(r"a\b"), r"a\b");
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
    fn views_read_through_the_same_unescape() {
        let t = Tokens::new(vec![r#"a\"b\nc"#.to_string(), "d".into()]);
        assert_eq!(t.sublist(0).get(0), t.get(0));
        assert_eq!(t.without_context_start_token().get(0), t.get(0));
        assert_eq!(t.last().unwrap(), "d");
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
