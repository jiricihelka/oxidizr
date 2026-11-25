//! Value substitution of the parse driver: the per-view dynamic sequence
//! numbers (peek/commit and the parallel frames) and the `${}` constant
//! substitution (`substituteStrings`). The section arms and the parse driver
//! call these through `pub(super)`.

use oxidizr_core::ViewRef;

use super::{NameValueType, Parser, SeqFrame};

impl Parser<'_> {
    // ------------------------------------------------------------------
    // dynamic-view sequence numbers
    // ------------------------------------------------------------------

    pub(super) fn dynamic_idx(view: ViewRef) -> usize {
        match view {
            ViewRef::Dynamic(i) => i,
            _ => usize::MAX,
        }
    }

    /// Read the number a step WOULD take
    /// (top + 1) without burning it. The official parser numbers the step as the
    /// argument of the final relationship-add call, i.e. only after
    /// every lookup/validation throw; pair this with [`Self::seq_commit`]
    /// right after a successful core add so failed steps consume nothing.
    pub(super) fn seq_peek(&mut self, view: ViewRef) -> String {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        if stack.is_empty() {
            stack.push(SeqFrame {
                sequence: 0,
                incremented: false,
            });
        }
        (stack.last().expect("root frame").sequence + 1).to_string()
    }

    /// Burn one sequence number (the peeked increment) — call only after
    /// the core add returned `Ok`, mirroring where the official parser numbers.
    pub(super) fn seq_commit(&mut self, view: ViewRef) {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        if stack.is_empty() {
            stack.push(SeqFrame {
                sequence: 0,
                incremented: false,
            });
        }
        let top = stack.last_mut().expect("root frame");
        top.sequence += 1;
        top.incremented = true;
    }

    /// `startParallelSequence` — a child counter seeded from the parent.
    pub(super) fn seq_start_parallel(&mut self, view: ViewRef) {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        let seed = stack.last().map_or(0, |f| f.sequence);
        stack.push(SeqFrame {
            sequence: seed,
            incremented: false,
        });
    }

    /// Ending the parallel sequence - both end-all branches copy the
    /// child sequence up; the child being un-incremented adds the extra
    /// parent increment (the "empty parallel block doesn't burn a number"
    /// semantics).
    pub(super) fn seq_end_parallel(&mut self, view: ViewRef, end_all: bool) {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        if stack.len() < 2 {
            // The official tool throws when the parent counter is absent; the DSL never
            // closes a parallel sequence that was never started, and recovery
            // truncation can't leave one open. No-op.
            return;
        }
        let child = stack.pop().expect("len >= 2");
        if end_all {
            let parent = stack.last_mut().expect("len >= 1");
            parent.sequence = child.sequence;
            if !child.incremented {
                parent.sequence += 1;
                parent.incremented = true;
            }
        }
    }

    // ------------------------------------------------------------------
    // ${} substitution (`substituteStrings`)
    // ------------------------------------------------------------------

    /// The official replace is literal; the scan iterates occurrences
    /// of the ORIGINAL token while each hit rewrites the current token in
    /// full — occurrences introduced by an earlier replacement are NOT
    /// substituted. That is modeled here by collecting the occurrences from
    /// the original first.
    pub(super) fn substitute_strings(&self, token: &str) -> String {
        let mut token = token.to_string();
        self.substitute_in(&mut token);
        token
    }

    /// The same substitution IN PLACE: the hot per-token leg calls this on
    /// the tokenizer's own `String` — a token without `${` occurrences
    /// (every token on most lines) is left untouched, dropping the copy
    /// the `&str -> String` shape forced on every token.
    pub(super) fn substitute_in(&self, token: &mut String) {
        // `(\$\{[a-zA-Z0-9-_.]+?})` — the lazy `+` equals "up to the first
        // }", and name chars exclude `}`, so scan `${` … nearest `}` with
        // all-valid non-empty name.
        let mut occurrences: Vec<String> = Vec::new();
        let mut i = 0usize;
        while let Some(rel) = token[i..].find("${") {
            let start = i + rel;
            let rest = &token[start + 2..];
            let name_end = rest.find('}');
            let name_ok = name_end.is_some_and(|e| {
                let name = &rest[..e];
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            });
            if name_ok {
                let e = name_end.unwrap_or(0);
                occurrences.push(token[start..=(start + 2 + e)].to_string());
                i = start + 2 + e + 1;
            } else {
                i = start + 1;
            }
        }
        // The scan uses the ORIGINAL token but each replace
        // mutates the working string — so every match (including repeats)
        // triggers a replace-all on the CURRENT token, in scan order.
        // Replacement-introduced `${…}` is never rescanned; a constant's
        // value is substituted RAW (only TextBlocks re-substitute).
        if occurrences.is_empty() {
            return;
        }
        for before in occurrences {
            let name = &before[2..before.len() - 1];
            let after = if let Some(nv) = self.constants.get(name) {
                Some(match nv.vtype {
                    NameValueType::TextBlock => self.substitute_strings(&nv.value),
                    _ => nv.value.clone(),
                })
            } else {
                // env substitution is ON (official parser ctor default)
                // unless a hermetic lookup is injected.
                match &self.opts.env_lookup {
                    Some(lookup) => lookup.get(name),
                    None => std::env::var(name).ok(),
                }
            };
            if let Some(after) = after {
                *token = token.replace(&before, &after);
            }
        }
    }
}
