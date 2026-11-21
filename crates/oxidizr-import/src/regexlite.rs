//! `regexlite` — the std-only backtracking matcher for the Java regex
//! subset the `!docs`/`!adrs` importers actually evaluate (P4-V8 CLOSED:
//! `exclude` is `name.equals(x) || name.matches(x)` — `String.matches` is a
//! FULL MATCH, so no `globset`; the reference's own internals feed the same
//! engine: `findImages` (`!\[.*]\((.*)\)`, `image:{1,2}(.*)\[.*]`) and the
//! ADR link patterns (`(.*) \[.*]\((.*)\)`, `\[.*]\((.*)\)`,
//! `- Status: (.*) \[.*]\((.*)\)`, `- (.*) \[.*]\((.*)\)`) plus the
//! filename filters (`\d{4}-.+?.md`, `\d{4}\d{2}\d{2}-.+?.md`).
//!
//! Supported: literals, `.` (NOT newline), `\` escapes (`\d \D \w \W \s \S`
//! and any escaped punctuation as a literal), classes `[abc]`/`[^a-z0-9]`
//! (leading `]` literal, trailing `-` literal, `\` escapes), anchors `^ $`,
//! capture groups `(...)` and non-capturing `(?:…)`, alternation `|`, and
//! greedy + lazy `* + ? {n} {n,} {n,m}` quantifiers. Inline flags,
//! backreferences and lookaround are OUT OF SCOPE — `Regex::new` reports
//! them (a JVM-extended construct failing here mirrors the reference's
//! failure intent, never a silent miss).
//!
//! Semantics: leftmost-first with Java backtracking preference order.
//! [`Regex::is_match_full`] is `String.matches`; [`Regex::find`] /
//! [`Regex::find_iter`] are `Matcher.find` (group text via
//! [`Found::group`]). Repetition over a single atom iterates a character
//! run in a loop (input length costs no stack depth); repetition over
//! composites recurses through the backtracker (fine at filename and line
//! lengths — the corpus bar).

use std::fmt;

/// Pattern syntax the subset does not carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError(pub String);

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SyntaxError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Char(char),
    /// `.` — any char except newline.
    Any,
    Class(Class),
    Start,
    End,
    Seq(Vec<Node>),
    /// `(...)` carries the capture index (1..=groups); `(?:…)` is `None`.
    Group {
        n: Option<usize>,
        inner: Box<Node>,
    },
    Alt(Vec<Node>),
    Rep {
        inner: Box<Node>,
        min: usize,
        max: Option<usize>,
        greedy: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Class {
    negated: bool,
    /// Inclusive ranges of single chars.
    ranges: Vec<(char, char)>,
    /// Predefined classes unioned into the set (`\d` inside `[… ]`).
    classes: Vec<ClassKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassKind {
    Digit,
    NotDigit,
    Word,
    NotWord,
    Space,
    NotSpace,
}

impl ClassKind {
    fn holds(self, c: char) -> bool {
        let base = match self {
            ClassKind::Digit | ClassKind::NotDigit => c.is_ascii_digit(),
            ClassKind::Word | ClassKind::NotWord => c.is_ascii_alphanumeric() || c == '_',
            ClassKind::Space | ClassKind::NotSpace => {
                matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}' | '\u{b}')
            }
        };
        if matches!(
            self,
            ClassKind::NotDigit | ClassKind::NotWord | ClassKind::NotSpace
        ) {
            !base
        } else {
            base
        }
    }
}

impl Class {
    fn holds(&self, c: char) -> bool {
        let inner = self.ranges.iter().any(|&(lo, hi)| lo <= c && c <= hi)
            || self.classes.iter().any(|k| k.holds(c));
        inner != self.negated
    }
}

/// A compiled pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regex {
    root: Node,
    groups: usize,
}

/// One successful `find` — byte bounds into the source plus capture
/// groups (index `0` is the whole match; groups in untaken branches are
/// `None`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Start (byte offset, inclusive).
    pub start: usize,
    /// End (byte offset, exclusive).
    pub end: usize,
    caps: Vec<Option<(usize, usize)>>,
}

impl Found {
    /// Capture group text (`0` = the whole match).
    #[must_use]
    pub fn group<'s>(&self, s: &'s str, n: usize) -> Option<&'s str> {
        if n == 0 {
            return Some(&s[self.start..self.end]);
        }
        let (a, b) = self.caps.get(n).copied().flatten()?;
        Some(&s[a..b])
    }
}

type Caps = Vec<Option<(usize, usize)>>;

/// `Parser::counted` result: the quantifier bounds `(min, max)` plus the
/// char span the braces consumed.
type Counted = ((usize, Option<usize>), usize);

impl Regex {
    /// Compile a pattern (subset above).
    ///
    /// # Errors
    ///
    /// Returns [`SyntaxError`] for constructs outside the subset —
    /// unknown escapes, malformed classes/quantifiers, unbalanced
    /// groups/braces.
    pub fn new(pattern: &str) -> Result<Self, SyntaxError> {
        let mut p = Parser {
            chars: pattern.chars().collect(),
            i: 0,
            groups: 0,
        };
        let root = p.alt()?;
        if p.i != p.chars.len() {
            return Err(SyntaxError(format!("unmatched `)` near position {}", p.i)));
        }
        Ok(Regex {
            root,
            groups: p.groups,
        })
    }

    /// `java.lang.String.matches` — the WHOLE input must match.
    #[must_use]
    pub fn is_match_full(&self, s: &str) -> bool {
        let (sc, _) = chars_and_bytes(s);
        let mut caps = vec![None; self.groups + 1];
        self.match_here(&sc, 0, &mut caps, |end, _| (end == sc.len()).then_some(end))
            .is_some()
    }

    /// `Matcher.find` at or after `from` (a BYTE offset; findings report
    /// byte bounds): the leftmost match, group captures recorded.
    #[must_use]
    pub fn find(&self, s: &str, from: usize) -> Option<Found> {
        let (sc, bmap) = chars_and_bytes(s);
        let start_char = bmap.iter().position(|&b| b >= from).unwrap_or(sc.len());
        for start in start_char..=sc.len() {
            let mut caps = vec![None; self.groups + 1];
            if let Some(end) = self.match_here(&sc, start, &mut caps, |end, _| Some(end)) {
                return Some(Found {
                    start: to_byte(&bmap, start, s.len()),
                    end: to_byte(&bmap, end, s.len()),
                    caps: caps
                        .into_iter()
                        .map(|c| {
                            c.map(|(a, b)| (to_byte(&bmap, a, s.len()), to_byte(&bmap, b, s.len())))
                        })
                        .collect(),
                });
            }
        }
        None
    }

    /// All `find` results in order — the `while (matcher.find())` loop
    /// (empty matches advance one char, exactly like `Matcher`).
    pub fn find_iter<'a>(&'a self, s: &'a str) -> impl Iterator<Item = Found> + 'a {
        let mut from = 0_usize;
        std::iter::from_fn(move || {
            let m = self.find(s, from)?;
            from = if m.end == m.start { m.end + 1 } else { m.end };
            Some(m)
        })
    }

    fn match_here(
        &self,
        s: &[char],
        pos: usize,
        caps: &mut Caps,
        k: impl Fn(usize, &mut Caps) -> Option<usize>,
    ) -> Option<usize> {
        run(&self.root, s, pos, caps, &k)
    }
}

/// (chars, char-index → byte-offset map).
fn chars_and_bytes(s: &str) -> (Vec<char>, Vec<usize>) {
    let mut chars = Vec::with_capacity(s.len());
    let mut bytes = Vec::with_capacity(s.len());
    for (b, c) in s.char_indices() {
        chars.push(c);
        bytes.push(b);
    }
    (chars, bytes)
}

fn to_byte(bmap: &[usize], char_pos: usize, s_len: usize) -> usize {
    bmap.get(char_pos).copied().unwrap_or(s_len)
}

/// CPS backtracker: `k` is the "rest of the pattern" continuation.
fn run(
    node: &Node,
    s: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &dyn Fn(usize, &mut Caps) -> Option<usize>,
) -> Option<usize> {
    match node {
        Node::Char(c) => {
            if s.get(pos) == Some(c) {
                k(pos + 1, caps)
            } else {
                None
            }
        }
        Node::Any => {
            if s.get(pos).is_some_and(|c| *c != '\n') {
                k(pos + 1, caps)
            } else {
                None
            }
        }
        Node::Class(cl) => {
            if s.get(pos).is_some_and(|c| cl.holds(*c)) {
                k(pos + 1, caps)
            } else {
                None
            }
        }
        Node::Start => {
            if pos == 0 {
                k(pos, caps)
            } else {
                None
            }
        }
        Node::End => {
            if pos == s.len() {
                k(pos, caps)
            } else {
                None
            }
        }
        Node::Seq(items) => run_seq(items, s, pos, caps, k),
        Node::Alt(branches) => {
            for b in branches {
                let mut snapshot = caps.clone();
                if let Some(end) = run(b, s, pos, &mut snapshot, k) {
                    *caps = snapshot;
                    return Some(end);
                }
            }
            None
        }
        Node::Group { n, inner } => {
            let saved = n.map(|i| caps[i]);
            if let Some(i) = *n {
                caps[i] = Some((pos, pos));
            }
            let res = run(inner, s, pos, caps, &|end, caps| {
                if let Some(i) = *n {
                    caps[i] = Some((pos, end));
                }
                k(end, caps)
            });
            if res.is_none() {
                if let (Some(i), Some(saved)) = (*n, saved) {
                    caps[i] = saved;
                }
            }
            res
        }
        Node::Rep {
            inner,
            min,
            max,
            greedy,
        } => {
            if matches!(inner.as_ref(), Node::Char(_) | Node::Any | Node::Class(_)) {
                run_rep_atom(inner, *min, *max, *greedy, s, pos, caps, k)
            } else {
                run_rep_composite(inner, *min, *max, *greedy, s, pos, caps, k)
            }
        }
    }
}

fn run_seq(
    items: &[Node],
    s: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &dyn Fn(usize, &mut Caps) -> Option<usize>,
) -> Option<usize> {
    match items.split_first() {
        None => k(pos, caps),
        Some((head, rest)) => {
            if rest.is_empty() {
                return run(head, s, pos, caps, k);
            }
            run(head, s, pos, caps, &|pos2, caps| {
                run_seq(rest, s, pos2, caps, k)
            })
        }
    }
}

/// Iterative run matcher for `Rep` over `Char`/`Any`/`Class` (the caller
/// guarantees an atom shape) — input length costs a loop, not stack depth.
#[allow(clippy::too_many_arguments)] // fixed CPS signature: the matcher state is one bundle
fn run_rep_atom(
    inner: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    s: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &dyn Fn(usize, &mut Caps) -> Option<usize>,
) -> Option<usize> {
    let mut run_ends = vec![pos];
    let mut run_pos = pos;
    while max.is_none_or(|m| run_ends.len() - 1 < m) {
        let Some(next) = atom_step(inner, s, run_pos) else {
            break;
        };
        run_pos = next;
        run_ends.push(run_pos);
    }
    let counts = run_ends.len() - 1;
    let hi = max.map_or(counts, |m| m.min(counts));
    if min > hi {
        return None;
    }
    if greedy {
        for c in (min..=hi).rev() {
            let mut snapshot = caps.clone();
            if let Some(end) = k(run_ends[c], &mut snapshot) {
                *caps = snapshot;
                return Some(end);
            }
        }
    } else {
        for end in &run_ends[min..=hi] {
            let mut snapshot = caps.clone();
            if let Some(end) = k(*end, &mut snapshot) {
                *caps = snapshot;
                return Some(end);
            }
        }
    }
    None
}

/// Recursive consume for `Rep` over composites, empty-loop guarded.
#[allow(clippy::too_many_arguments)] // fixed CPS signature: forwards the whole bundle
fn run_rep_composite(
    inner: &Node,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    s: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &dyn Fn(usize, &mut Caps) -> Option<usize>,
) -> Option<usize> {
    rep_step(inner, 0, min, max, greedy, s, pos, caps, k)
}

#[allow(clippy::too_many_arguments)]
fn rep_step(
    inner: &Node,
    count: usize,
    min: usize,
    max: Option<usize>,
    greedy: bool,
    s: &[char],
    pos: usize,
    caps: &mut Caps,
    k: &dyn Fn(usize, &mut Caps) -> Option<usize>,
) -> Option<usize> {
    let stop =
        |caps: &mut Caps| -> Option<usize> { if count >= min { k(pos, caps) } else { None } };
    let consume = |caps: &mut Caps| -> Option<usize> {
        if max.is_some_and(|m| count >= m) {
            return None;
        }
        let mut snapshot = caps.clone();
        let r = run(inner, s, pos, &mut snapshot, &|pos2, caps2| {
            if pos2 == pos {
                return None; // empty-loop guard
            }
            rep_step(inner, count + 1, min, max, greedy, s, pos2, caps2, k)
        });
        if r.is_some() {
            *caps = snapshot;
        }
        r
    };
    if greedy {
        consume(caps).or_else(|| stop(caps))
    } else {
        stop(caps).or_else(|| consume(caps))
    }
}

/// One-char step for atom nodes — `Some(next_pos)`.
fn atom_step(node: &Node, s: &[char], pos: usize) -> Option<usize> {
    match node {
        Node::Char(c) if s.get(pos) == Some(c) => Some(pos + 1),
        Node::Any if s.get(pos).is_some_and(|c| *c != '\n') => Some(pos + 1),
        Node::Class(cl) if s.get(pos).is_some_and(|c| cl.holds(*c)) => Some(pos + 1),
        _ => None,
    }
}

struct Parser {
    chars: Vec<char>,
    i: usize,
    groups: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn alt(&mut self) -> Result<Node, SyntaxError> {
        let mut branches = vec![self.seq()?];
        while self.peek() == Some('|') {
            self.i += 1;
            branches.push(self.seq()?);
        }
        Ok(if branches.len() == 1 {
            branches.pop().expect("one branch")
        } else {
            Node::Alt(branches)
        })
    }

    fn seq(&mut self) -> Result<Node, SyntaxError> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            items.push(self.quantified()?);
        }
        Ok(match items.len() {
            0 => Node::Seq(Vec::new()),
            1 => items.pop().expect("one item"),
            _ => Node::Seq(items),
        })
    }

    fn quantified(&mut self) -> Result<Node, SyntaxError> {
        let atom = self.atom()?;
        let Some(q) = self.peek() else {
            return Ok(atom);
        };
        let (min, max, span) = match q {
            '*' => (0, None, 1),
            '+' => (1, None, 1),
            '?' => (0, Some(1), 1),
            '{' => match self.counted()? {
                Some(((min, max), span)) => (min, max, span),
                None => return Ok(atom), // literal `{`
            },
            _ => return Ok(atom),
        };
        self.i += span;
        let greedy = self.peek() != Some('?');
        if !greedy {
            self.i += 1;
        }
        Ok(Node::Rep {
            inner: Box::new(atom),
            min,
            max,
            greedy,
        })
    }

    /// `{n}` / `{n,}` / `{n,m}` at `self.i` → bounds plus the char span.
    /// `Ok(None)` means "not a counted quantifier" — the brace stays a
    /// literal (Java's lenient reading).
    fn counted(&mut self) -> Result<Option<Counted>, SyntaxError> {
        let start = self.i;
        let mut j = self.i + 1;
        let digits = |j: &mut usize| -> String {
            self.chars[*j..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .copied()
                .inspect(|_| *j += 1)
                .collect()
        };
        let lo_s = digits(&mut j);
        if lo_s.is_empty() {
            return Ok(None);
        }
        let lo: usize = lo_s
            .parse()
            .map_err(|_| SyntaxError("quantifier too large".into()))?;
        let hi = match self.chars.get(j) {
            Some(',') => {
                j += 1;
                let hi_s = digits(&mut j);
                if hi_s.is_empty() {
                    None
                } else {
                    Some(
                        hi_s.parse::<usize>()
                            .map_err(|_| SyntaxError("quantifier too large".into()))?,
                    )
                }
            }
            Some('}') => Some(lo),
            _ => return Ok(None),
        };
        if self.chars.get(j) != Some(&'}') || hi.is_some_and(|h| h < lo) {
            return Ok(None);
        }
        j += 1;
        Ok(Some(((lo, hi), j - start)))
    }

    fn atom(&mut self) -> Result<Node, SyntaxError> {
        let c = self
            .peek()
            .ok_or_else(|| SyntaxError("truncated pattern".into()))?;
        match c {
            '(' => {
                let non_capturing = self.chars.get(self.i + 1) == Some(&'?')
                    && self.chars.get(self.i + 2) == Some(&':');
                if self.chars.get(self.i + 1) == Some(&'?') && !non_capturing {
                    return Err(SyntaxError(
                        "groups beyond `(...)`/`(?:…)` are not supported by regexlite".into(),
                    ));
                }
                self.i += if non_capturing { 3 } else { 1 };
                let n = if non_capturing {
                    None
                } else {
                    self.groups += 1;
                    Some(self.groups)
                };
                let inner = self.alt()?;
                if self.peek() != Some(')') {
                    return Err(SyntaxError("missing `)` for group".into()));
                }
                self.i += 1;
                Ok(Node::Group {
                    n,
                    inner: Box::new(inner),
                })
            }
            '.' => {
                self.i += 1;
                Ok(Node::Any)
            }
            '^' => {
                self.i += 1;
                Ok(Node::Start)
            }
            '$' => {
                self.i += 1;
                Ok(Node::End)
            }
            '[' => self.class(),
            '\\' => self.escape(),
            '*' | '+' | '?' => Err(SyntaxError(format!(
                "dangling quantifier `{c}` near position {}",
                self.i
            ))),
            ')' => Err(SyntaxError(format!(
                "unmatched `)` near position {}",
                self.i
            ))),
            _ => {
                self.i += 1;
                Ok(Node::Char(c))
            }
        }
    }

    fn escape(&mut self) -> Result<Node, SyntaxError> {
        self.i += 1; // backslash
        let c = self
            .peek()
            .ok_or_else(|| SyntaxError("trailing backslash".into()))?;
        self.i += 1;
        Ok(match class_escape(c)? {
            Some(kind) => Node::Class(match kind {
                ClassKind::Digit | ClassKind::Word | ClassKind::Space => Class {
                    negated: false,
                    ranges: Vec::new(),
                    classes: vec![kind],
                },
                ClassKind::NotDigit | ClassKind::NotWord | ClassKind::NotSpace => Class {
                    negated: true,
                    ranges: Vec::new(),
                    // negate the POSITIVE member (double negation would flip)
                    classes: vec![match kind {
                        ClassKind::NotDigit => ClassKind::Digit,
                        ClassKind::NotWord => ClassKind::Word,
                        _ => ClassKind::Space,
                    }],
                },
            }),
            None => Node::Char(c),
        })
    }

    fn class(&mut self) -> Result<Node, SyntaxError> {
        self.i += 1; // `[`
        let mut class = Class {
            negated: false,
            ranges: Vec::new(),
            classes: Vec::new(),
        };
        if self.peek() == Some('^') {
            class.negated = true;
            self.i += 1;
        }
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return Err(SyntaxError("unterminated character class".into()));
            };
            if c == ']' && !first {
                self.i += 1;
                return Ok(Node::Class(class));
            }
            first = false;
            let lo = if c == '\\' {
                self.i += 1;
                let e = self
                    .peek()
                    .ok_or_else(|| SyntaxError("trailing backslash in class".into()))?;
                self.i += 1;
                match class_escape(e)? {
                    Some(kind) => {
                        class.classes.push(kind);
                        continue;
                    }
                    None => e,
                }
            } else {
                self.i += 1;
                c
            };
            // range?
            if self.peek() == Some('-') && self.chars.get(self.i + 1).is_some_and(|&n| n != ']') {
                self.i += 1;
                let hi = if self.peek() == Some('\\') {
                    self.i += 1;
                    let e = self
                        .peek()
                        .ok_or_else(|| SyntaxError("trailing backslash in class".into()))?;
                    self.i += 1;
                    e
                } else {
                    let h = self.peek().expect("checked above");
                    self.i += 1;
                    h
                };
                if hi < lo {
                    return Err(SyntaxError("invalid character range".into()));
                }
                class.ranges.push((lo, hi));
            } else {
                class.ranges.push((lo, lo));
            }
        }
    }
}

/// The six predefined classes; `None` for literal escapes. `\b` is an
/// explicit no.
fn class_escape(e: char) -> Result<Option<ClassKind>, SyntaxError> {
    Ok(match e {
        'd' => Some(ClassKind::Digit),
        'D' => Some(ClassKind::NotDigit),
        'w' => Some(ClassKind::Word),
        'W' => Some(ClassKind::NotWord),
        's' => Some(ClassKind::Space),
        'S' => Some(ClassKind::NotSpace),
        'b' => {
            return Err(SyntaxError("`\\b` is not supported by regexlite".into()));
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full(p: &str, s: &str) -> bool {
        Regex::new(p).expect("compiles").is_match_full(s)
    }

    #[test]
    fn full_match_is_string_matches() {
        assert!(full("README.md", "README.md")); // unescaped `.` is ANY — still matches
        assert!(full("README\\.md", "README.md"));
        assert!(!full("README\\.md", "READMEXmd"));
        assert!(full("README.*", "README.md"));
        assert!(!full("README.*", "XREADME.md")); // FULL match, not find
        assert!(!full("readme\\.md", "README.md")); // case-sensitive
    }

    #[test]
    fn adr_filename_filters() {
        assert!(full(r"\d{4}-.+?.md", "0001-x.md"));
        assert!(!full(r"\d{4}-.+?.md", "0001.md"));
        assert!(!full(r"\d{4}-.+?.md", "abcd-x.md"));
        assert!(full(r"\d{4}\d{2}\d{2}-.+?.md", "20240111-x.md"));
        assert!(!full(r"\d{4}\d{2}\d{2}-.+?.md", "2024011-x.md"));
        // the `.` before md is ANY char, faithfully
        assert!(full(r"\d{4}-.+?.md", "0001-xXmd"));
        // `+` requires at least one repetition
        assert!(!full(r"\d{4}-.+?.md", "0001-.md"));
    }

    #[test]
    fn find_images_markdown_pattern() {
        let re = Regex::new(r"!\[.*]\((.*)\)").expect("compiles");
        let s = "see ![alt](sub/diagram.png) here";
        let m = re.find(s, 0).expect("match");
        assert_eq!(m.group(s, 1), Some("sub/diagram.png"));
        // greedy across two links (Java `.*` runs to the LAST closing paren,
        // and one whole-line match registers only the trailing target)
        let s = "![a](x.png) ![b](y.png)";
        let m = re.find(s, 0).expect("match");
        assert_eq!(m.group(s, 1), Some("y.png"));
    }

    #[test]
    fn find_images_adoc_pattern() {
        let re = Regex::new(r"image:{1,2}(.*)\[.*]").expect("compiles");
        let s = "image::diagram.png[alt]";
        let m = re.find(s, 0).expect("match");
        assert_eq!(m.group(s, 1), Some("diagram.png"));
        let s = "image:x.png[a]";
        let m = re.find(s, 0).expect("match");
        assert_eq!(m.group(s, 1), Some("x.png"));
    }

    #[test]
    fn link_patterns_capture_description_and_target() {
        let re = Regex::new(r"(.*) \[.*]\((.*)\)").expect("compiles");
        let s = "This is superseded by [0002-x.md](0002-x.md)";
        let m = re.find(s, 0).expect("match");
        assert_eq!(m.group(s, 1), Some("This is superseded by"));
        assert_eq!(m.group(s, 2), Some("0002-x.md"));

        let re = Regex::new(r"\[.*]\((.*)\)").expect("compiles");
        let s = "see [text](0003-y.md) now";
        let m = re.find(s, 0).expect("match");
        assert_eq!(m.group(s, 1), Some("0003-y.md"));
        // two links on one line: greedy `.*` runs the whole match to the
        // LAST target — the Java `while (find())` collects one too
        let s = "[a](0001-a.md) and [b](0002-b.md)";
        let got: Vec<&str> = re.find_iter(s).map(|m| m.group(s, 1).unwrap()).collect();
        assert_eq!(got, vec!["0002-b.md"]);
    }

    #[test]
    fn lazy_and_greedy_alternation_and_classes() {
        let s = "abzaz";
        let m = Regex::new(r"a.*?z").expect("c").find(s, 0).expect("m");
        assert_eq!((m.start, m.end), (0, 3)); // lazy: first `z`
        let m = Regex::new(r"a.*z").expect("c").find(s, 0).expect("m");
        assert_eq!((m.start, m.end), (0, 5)); // greedy: last `z`
        assert!(full("[a-c]+", "abcba"));
        assert!(!full("[^a-c]+", "abx"));
        assert!(full("[^a-c]+", "xyz"));
        assert!(full("a|b*c", "bbc"));
        // literal brace when not a valid quantifier
        assert!(full(r"a{2", "a{2"));
        // `.` does not match newline
        assert!(!full("a.*b", "a\nb"));
        // find resumes after a match
        let re = Regex::new(r"a+").expect("c");
        let s = "baaaca";
        let all: Vec<(usize, usize)> = re.find_iter(s).map(|m| (m.start, m.end)).collect();
        assert_eq!(all, vec![(1, 4), (5, 6)]);
    }

    #[test]
    fn groups_across_alternation_and_nesting() {
        let re = Regex::new(r"(a(b)|c)(d)").expect("c");
        let s = "abd";
        let m = re.find(s, 0).expect("m");
        assert_eq!(m.group(s, 1), Some("ab"));
        assert_eq!(m.group(s, 2), Some("b"));
        assert_eq!(m.group(s, 3), Some("d"));
        let s = "cd";
        let m = re.find(s, 0).expect("m");
        assert_eq!(m.group(s, 1), Some("c"));
        assert_eq!(m.group(s, 2), None);
        assert_eq!(m.group(s, 3), Some("d"));
        // non-capturing groups keep numbering tight
        let re = Regex::new(r"(?:a|b)(x)").expect("c");
        let s = "bx";
        let m = re.find(s, 0).expect("m");
        assert_eq!(m.group(s, 1), Some("x"));
    }

    #[test]
    fn unsupported_constructs_error() {
        assert!(Regex::new(r"(?i)a").is_err());
        assert!(Regex::new(r"\bword").is_err());
        assert!(Regex::new(r"a(").is_err());
        assert!(Regex::new(r"[abc").is_err());
        assert!(Regex::new(r"a)").is_err());
    }

    #[test]
    fn long_line_is_a_loop_not_a_stack() {
        let line = "x".repeat(200_000);
        let s = format!("![alt]({line}.png)");
        let re = Regex::new(r"!\[.*]\((.*)\)").expect("c");
        let m = re.find(&s, 0).expect("match");
        assert_eq!(m.group(&s, 1).map(str::len), Some(line.len() + 4));
    }

    #[test]
    fn utf8_offsets_are_bytes() {
        let s = "décision ![alt](図.png) fin";
        let re = Regex::new(r"!\[.*]\((.*)\)").expect("c");
        let m = re.find(s, 0).expect("m");
        assert_eq!(m.group(s, 1), Some("図.png"));
        assert_eq!(&s[m.start..m.end], "![alt](図.png)");
    }
}
