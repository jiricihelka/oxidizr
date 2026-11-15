//! Port of the reference `com.structurizr.util.Url` (`URI.create(url)` +
//! `URI.toURL()` validity test).
//!
//! The reference asks the JVM whether `urlAsString` is a well-formed URI that
//! carries a protocol `java.net.URL` knows how to handle. The JDK's built-in
//! handlers are exactly `http`, `https`, `ftp`, `file` and `jar` — every other
//! scheme (or a scheme-less URI) throws `MalformedURLException("unknown
//! protocol")`, so `isUrl` is false for them. URI well-formedness follows
//! RFC 2396: ASCII only, the unreserved/reserved character sets, and
//! percent-escapes with two hex digits. This module reproduces that decision
//! with std string checks (the reference NEVER fetches anything); the
//! approximation is recorded in `plan/PROGRESS.md`.

/// The protocols `java.net.URL` ships handlers for.
const JAVA_URL_PROTOCOLS: [&str; 5] = ["http", "https", "ftp", "file", "jar"];

/// `Url.isUrl`: non-blank, a well-formed (RFC 2396) URI, and a protocol the
/// JDK can turn into a `URL`.
#[must_use]
pub fn is_url(url: &str) -> bool {
    if url.trim().is_empty() {
        // `StringUtils.isNullOrEmpty` trims.
        return false;
    }
    let Some((scheme, rest)) = url.split_once(':') else {
        return false; // relative URI ⇒ `toURL()` throws
    };
    if !is_java_scheme(scheme) {
        return false;
    }
    JAVA_URL_PROTOCOLS.contains(&scheme.to_lowercase().as_str()) && is_uri_rest(rest)
}

/// `Url.isHttpsUrl`.
#[must_use]
pub fn is_https_url(url: &str) -> bool {
    is_url(url) && url.to_lowercase().starts_with("https://")
}

/// `Url.isHttpUrl`.
#[must_use]
pub fn is_http_url(url: &str) -> bool {
    is_url(url) && url.to_lowercase().starts_with("http://")
}

/// Java scheme grammar: `[A-Za-z][A-Za-z0-9+.-]*`.
fn is_java_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
}

/// RFC 2396 legality of everything after the scheme colon: unreserved +
/// reserved ASCII, with well-formed percent-escapes.
fn is_uri_rest(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'%' {
            // Java `URI` validates every escape.
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return false;
            }
            i += 3;
            continue;
        }
        // unreserved: ALPHA / DIGIT / "-" / "." / "_" / "~"
        // reserved:    ";" | "/" | "?" | ":" | "@" | "&" | "=" | "+" | "$" | ","
        //              | "[" | "]" | "-" | "_" | "." | "!" | "*" | "'" | "(" | ")" | "#"
        // (non-ASCII is illegal — Java URI is ASCII-only.)
        if !(c.is_ascii_alphanumeric()
            || matches!(
                c,
                b'-' | b'.'
                    | b'_'
                    | b'~'
                    | b':'
                    | b'/'
                    | b'?'
                    | b'#'
                    | b'['
                    | b']'
                    | b'@'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
            ))
        {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn accepted_urls() {
        for u in [
            "http://example.com",
            "http://example.com/health?x=1#f",
            "https://example.com/a_b-c~d.e",
            "http://user:pass@example.com:8080/p",
            "ftp://host/file.txt",
            "file:/tmp/x",
            "HTTP://upper.example", // scheme case-insensitive
        ] {
            assert!(is_url(u), "{u} should be a URL");
        }
    }

    #[test]
    fn rejected_urls() {
        for u in [
            "",
            "   ",
            "example.com",                // no scheme ⇒ relative URI
            "www.example.com/x",          // no scheme
            "mailto:a@b.c",               // URI-legal, but no JDK URL handler
            "data:image/png;base64,AA==", // ditto
            "http://exa mple",            // space is not URI-legal
            "http://ex|mple",             // ditto
            "http://exa%mple",            // malformed escape
            "http://é.fr",                // non-ASCII
        ] {
            assert!(!is_url(u), "{u} should NOT be a URL");
        }
    }

    #[test]
    fn scheme_helpers() {
        assert!(is_https_url("HTTPS://x.example/"));
        assert!(!is_https_url("http://x.example/"));
        assert!(is_http_url("http://x.example/"));
        assert!(!is_http_url("https://x.example/"));
    }
}
