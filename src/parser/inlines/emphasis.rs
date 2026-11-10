//! Emphasis/strong resolution via the CommonMark delimiter-stack algorithm
//! (spec section 6.2 "Emphasis and strong emphasis", Appendix A).
//!
//! Replaces the previous approach of scanning forward for a matching closer
//! at every `*`/`_` position (which re-scanned overlapping text for every
//! nested/unbalanced delimiter and made pathological input super-linear).
//! Here, [`super::parse_inlines_from_span`] first tokenizes the whole span
//! left-to-right into a flat [`Item`] sequence (delimiter runs are left
//! unresolved as [`Item::Delim`], everything else is a fully-parsed
//! [`Node`]); [`resolve_emphasis`] then runs a single linear-amortized pass
//! over that sequence, matching delimiters with an explicit doubly-linked
//! stack (an `openers_bottom` bound per the spec skips re-scanning dead
//! ranges, which is what keeps this linear on adversarial input).

use super::cjk;
use crate::parser::ast::{Node, NodeKind};
use crate::parser::inlines::shared::{opt_span_range, GrammarSpan};
use nom::Input;
use unicode_general_category::{get_general_category, GeneralCategory};

/// A pending, not-yet-resolved run of `*` or `_` characters.
#[derive(Debug, Clone, Copy)]
pub(super) struct Delimiter<'a> {
    /// Current (possibly shrunk) remaining sub-span of the original run.
    run: GrammarSpan<'a>,
    ch: char,
    /// Length of the run as first tokenized; fixed for the "rule of 3" check.
    original_count: usize,
    can_open: bool,
    can_close: bool,
}

impl<'a> Delimiter<'a> {
    fn count(&self) -> usize {
        self.run.fragment().len()
    }

    /// Consume `n` characters from the side nearest the wrapped content when
    /// this run is used as an *opener* (the right/inner side), returning the
    /// span of the consumed characters and shrinking `self` to the leftover
    /// left/outer prefix.
    fn consume_as_opener(&mut self, n: usize) -> GrammarSpan<'a> {
        let keep = self.count() - n;
        let consumed = self.run.take_from(keep);
        self.run = self.run.take(keep);
        consumed
    }

    /// Consume `n` characters from the side nearest the wrapped content when
    /// this run is used as a *closer* (the left/inner side), returning the
    /// span of the consumed characters and shrinking `self` to the leftover
    /// right/outer suffix.
    fn consume_as_closer(&mut self, n: usize) -> GrammarSpan<'a> {
        let consumed = self.run.take(n);
        self.run = self.run.take_from(n);
        consumed
    }

    fn into_text_node(self) -> Node {
        Node {
            kind: NodeKind::Text(self.ch.to_string().repeat(self.count())),
            span: crate::parser::shared::opt_span(self.run),
            children: Vec::new(),
        }
    }
}

/// One slot in the flat, left-to-right tokenized inline sequence.
pub(super) enum Item<'a> {
    /// A fully-resolved inline node (text, code span, link, etc.).
    Node(Node),
    /// An unresolved `*`/`_` run awaiting emphasis resolution.
    Delim(Delimiter<'a>),
    /// A slot whose content has been absorbed into a neighboring wrap node.
    Consumed,
}

impl<'a> Item<'a> {
    /// Last character this item would contribute if flattened to text now.
    /// Used by boundary heuristics (e.g. task-checkbox start detection) that
    /// need to see "what character comes right before this position" even
    /// though emphasis resolution hasn't run yet.
    pub(super) fn last_char(&self) -> Option<char> {
        match self {
            Item::Node(n) => last_char_in_node(n),
            Item::Delim(d) => Some(d.ch),
            Item::Consumed => None,
        }
    }
}

fn last_char_in_node(node: &Node) -> Option<char> {
    match &node.kind {
        NodeKind::Text(t) => t.chars().last(),
        _ => node.children.iter().rev().find_map(last_char_in_node),
    }
}

// ---------------------------------------------------------------------------
// Flanking-rule classification (CommonMark spec 6.2, rules 1-8)
// ---------------------------------------------------------------------------

/// CommonMark "Unicode whitespace character": Rust's `char::is_whitespace`
/// (Unicode `White_Space` property) is a superset of the spec's literal
/// wording (Zs, tab, LF, FF, CR) that also includes a couple of additional
/// separator/control characters (NEL, VT, Zl, Zp); those never appear in
/// the CommonMark spec test corpus, and other Rust Markdown implementations
/// make the same simplification.
fn is_unicode_whitespace(c: char) -> bool {
    c.is_whitespace()
}

/// CommonMark "Unicode punctuation character": Unicode P (punctuation) or
/// S (symbol) general category.
fn is_unicode_punctuation(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::ConnectorPunctuation
            | GeneralCategory::DashPunctuation
            | GeneralCategory::OpenPunctuation
            | GeneralCategory::ClosePunctuation
            | GeneralCategory::InitialPunctuation
            | GeneralCategory::FinalPunctuation
            | GeneralCategory::OtherPunctuation
            | GeneralCategory::MathSymbol
            | GeneralCategory::CurrencySymbol
            | GeneralCategory::ModifierSymbol
            | GeneralCategory::OtherSymbol
    )
}

#[derive(Clone, Copy, Default)]
struct CharClass {
    whitespace: bool,
    punctuation: bool,
    cjk: bool,
    ivs: bool,
    non_emoji_vs: bool,
}

impl CharClass {
    fn of(c: Option<char>) -> Self {
        let Some(c) = c else {
            return Self {
                whitespace: true,
                ..Self::default()
            };
        };
        if is_unicode_whitespace(c) {
            return Self {
                whitespace: true,
                ..Self::default()
            };
        }
        if cjk::is_non_emoji_general_use_vs(c) {
            return Self {
                non_emoji_vs: true,
                ..Self::default()
            };
        }
        if cjk::is_ivs(c) {
            return Self {
                ivs: true,
                ..Self::default()
            };
        }
        Self {
            punctuation: is_unicode_punctuation(c),
            cjk: cjk::is_cjk(c),
            ..Self::default()
        }
    }

    fn preceding(before: Option<char>, two_before: Option<char>) -> Self {
        let class = Self::of(before);
        if !class.non_emoji_vs {
            return class;
        }
        let modified = Self::of(two_before);
        if modified.whitespace {
            return class;
        }
        if is_cjk_ambiguous_punctuation(two_before, before) {
            return Self {
                punctuation: true,
                cjk: true,
                ..Self::default()
            };
        }
        Self {
            ivs: false,
            ..modified
        }
    }

    fn non_cjk_punctuation(self) -> bool {
        self.punctuation && !self.cjk
    }

    fn space_or_non_cjk_punctuation(self) -> bool {
        self.whitespace || self.non_cjk_punctuation()
    }

    fn space_or_punctuation(self) -> bool {
        self.whitespace || self.punctuation
    }
}

fn is_cjk_ambiguous_punctuation(main: Option<char>, vs: Option<char>) -> bool {
    vs == Some('\u{FE01}')
        && matches!(
            main,
            Some('\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}')
        )
}

/// Classify a `*`/`_` run given the two characters before it and the one
/// after it in the source (`None` = start/end of the span being parsed,
/// treated as whitespace per spec), with the CJK-friendly amendment
/// (https://github.com/tats-u/markdown-cjk-friendly/blob/main/specification.md).
fn classify(
    ch: char,
    two_before: Option<char>,
    before: Option<char>,
    after: Option<char>,
) -> (bool, bool) {
    let before = CharClass::preceding(before, two_before);
    let after = CharClass::of(after);

    let open = !after.space_or_non_cjk_punctuation()
        || after.non_cjk_punctuation()
            && (before.space_or_non_cjk_punctuation() || before.cjk || before.ivs);
    let close = !before.space_or_non_cjk_punctuation()
        || before.non_cjk_punctuation() && (after.space_or_non_cjk_punctuation() || after.cjk);

    if ch == '*' {
        (open, close)
    } else {
        // '_' has the extra intraword restriction (spec rules 3/4, 7/8).
        (
            open && (before.space_or_punctuation() || !close),
            close && (after.space_or_punctuation() || !open),
        )
    }
}

/// Tokenize a `*`/`_` run starting at `remaining` (which must begin with
/// `ch`), given the two characters preceding `remaining` in the top-level span
/// (or `None` at the start of the span). Returns the built [`Delimiter`]
/// and the rest of the input after the run.
pub(super) fn tokenize_delimiter_run(
    remaining: GrammarSpan,
    two_before: Option<char>,
    before: Option<char>,
) -> (Delimiter, GrammarSpan) {
    let ch = remaining
        .fragment()
        .chars()
        .next()
        .expect("caller guarantees non-empty input starting with a delimiter char");
    let run_len = remaining
        .fragment()
        .chars()
        .take_while(|&c| c == ch)
        .count();
    let (rest, run) = remaining.take_split(run_len);
    let after = rest.fragment().chars().next();
    let (can_open, can_close) = classify(ch, two_before, before, after);
    (
        Delimiter {
            run,
            ch,
            original_count: run_len,
            can_open,
            can_close,
        },
        rest,
    )
}

// ---------------------------------------------------------------------------
// Resolution: doubly-linked delimiter stack over the flat `items` sequence.
// ---------------------------------------------------------------------------

/// One entry in the delimiter stack, indexing into `items`. Linked via
/// `prev`/`next` arena indices so unlinking a matched/dead entry is O(1)
/// (no index shifting), which is what keeps resolution linear-amortized on
/// adversarial input (many unmatched delimiters).
struct StackEntry {
    item_idx: usize,
    ch: char,
    original_count: usize,
    can_open: bool,
    can_close: bool,
    prev: Option<usize>,
    next: Option<usize>,
}

fn bucket(ch: char, original_count: usize) -> (char, u8) {
    (ch, (original_count % 3) as u8)
}

/// Resolve all `Item::Delim` entries in `items` into properly nested
/// `Emphasis`/`Strong` (and, for the plain triple-run case, the flat
/// `StrongEmphasis` convenience node) `Node`s, per the CommonMark delimiter
/// stack algorithm. Any delimiter that never matches becomes literal text.
pub(super) fn resolve_emphasis(mut items: Vec<Item>) -> Vec<Node> {
    let mut arena: Vec<StackEntry> = Vec::new();
    for (idx, item) in items.iter().enumerate() {
        if let Item::Delim(d) = item {
            let prev_idx = arena.len().checked_sub(1);
            if let Some(p) = prev_idx {
                arena[p].next = Some(arena.len());
            }
            arena.push(StackEntry {
                item_idx: idx,
                ch: d.ch,
                original_count: d.original_count,
                can_open: d.can_open,
                can_close: d.can_close,
                prev: prev_idx,
                next: None,
            });
        }
    }

    use std::collections::{HashMap, HashSet};
    let mut openers_bottom: HashMap<(char, u8), Option<usize>> = HashMap::new();
    // Tracks (opener_arena_idx, closer_arena_idx) pairs that have already
    // matched once. A *repeat* match between the exact same pair only
    // happens when re-examining a partially-consumed run against itself
    // (e.g. the classic "***text***" triple-run case, matched once for
    // `**` then again for the leftover `*`) — see `is_pure_triple_run` below.
    let mut matched_pairs: HashSet<(usize, usize)> = HashSet::new();

    let mut closer_idx = if arena.is_empty() { None } else { Some(0) };

    while let Some(ci) = closer_idx {
        if !arena[ci].can_close {
            closer_idx = arena[ci].next;
            continue;
        }

        let b = bucket(arena[ci].ch, arena[ci].original_count);
        let bound = openers_bottom.get(&b).copied().flatten();

        let mut oi_opt = arena[ci].prev;
        let mut found: Option<usize> = None;
        while let Some(oi) = oi_opt {
            // `bound` is an arena index snapshot from an earlier failed
            // scan of this bucket. Arena indices are assigned once, in
            // left-to-right order, and never reused, so comparing by value
            // (rather than re-walking to the same linked node, which may
            // since have been unlinked by an unrelated match) is safe: any
            // index at or below `bound` was already confirmed unmatchable
            // for this bucket.
            if let Some(b) = bound {
                if oi <= b {
                    break;
                }
            }
            if arena[oi].ch == arena[ci].ch && arena[oi].can_open {
                let either_both = (arena[oi].can_open && arena[oi].can_close)
                    || (arena[ci].can_open && arena[ci].can_close);
                let compatible = if either_both {
                    let sum_mod3_zero =
                        (arena[oi].original_count + arena[ci].original_count).is_multiple_of(3);
                    let both_mod3_zero = arena[oi].original_count.is_multiple_of(3)
                        && arena[ci].original_count.is_multiple_of(3);
                    !sum_mod3_zero || both_mod3_zero
                } else {
                    true
                };
                if compatible {
                    found = Some(oi);
                    break;
                }
            }
            oi_opt = arena[oi].prev;
        }

        match found {
            Some(oi) => {
                let opener_count = current_count(&items, arena[oi].item_idx);
                let closer_count = current_count(&items, arena[ci].item_idx);
                let use_n = if opener_count >= 2 && closer_count >= 2 {
                    2
                } else {
                    1
                };

                let is_repeat_pair = !matched_pairs.insert((oi, ci));

                // Unlink (but do not delete) every stack entry strictly
                // between the opener and closer: once wrapped, they can
                // never match anything outside this new node.
                let mut k = arena[oi].next;
                while let Some(kk) = k {
                    if kk == ci {
                        break;
                    }
                    let next_k = arena[kk].next;
                    unlink(&mut arena, kk);
                    k = next_k;
                }
                arena[oi].next = Some(ci);
                arena[ci].prev = Some(oi);

                let oi_item = arena[oi].item_idx;
                let ci_item = arena[ci].item_idx;

                // Absorb everything strictly between the two delimiter
                // items (there is always at least one slot — see module
                // notes) into the new node's children.
                let mut children: Vec<Node> = Vec::new();
                for slot in items.iter_mut().take(ci_item).skip(oi_item + 1) {
                    match std::mem::replace(slot, Item::Consumed) {
                        Item::Node(n) => children.push(n),
                        Item::Delim(d) => children.push(d.into_text_node()),
                        Item::Consumed => {}
                    }
                }

                let opener_span = match &mut items[oi_item] {
                    Item::Delim(d) => d.consume_as_opener(use_n),
                    _ => unreachable!("opener slot must still hold its delimiter"),
                };
                let closer_end_span = match &mut items[ci_item] {
                    Item::Delim(d) => {
                        let _ = d.consume_as_closer(use_n);
                        d.run
                    }
                    _ => unreachable!("closer slot must still hold its delimiter"),
                };
                let span = opt_span_range(opener_span, closer_end_span);

                let kind_is_strong = use_n == 2;
                let is_pure_triple_run = is_repeat_pair
                    && arena[oi].original_count == 3
                    && arena[ci].original_count == 3;
                let wrapped = if is_pure_triple_run
                    && children.len() == 1
                    && is_pure_triple_pair(&children[0], kind_is_strong)
                {
                    // Collapse the classic "***text***"/"___text___" pattern
                    // (an inner Strong immediately re-wrapped by an outer
                    // Emphasis from the same original 3-length run on both
                    // sides) back into the single flat `StrongEmphasis` node,
                    // matching this crate's existing convenience shape for
                    // that case instead of exposing it as nested Emphasis(Strong).
                    let inner_children = match children.into_iter().next() {
                        Some(Node { children, .. }) => children,
                        None => unreachable!(),
                    };
                    Node {
                        kind: NodeKind::StrongEmphasis,
                        span,
                        children: inner_children,
                    }
                } else {
                    Node {
                        kind: if kind_is_strong {
                            NodeKind::Strong
                        } else {
                            NodeKind::Emphasis
                        },
                        span,
                        children,
                    }
                };
                items[oi_item + 1] = Item::Node(wrapped);

                let opener_remaining = current_count(&items, oi_item);
                if opener_remaining == 0 {
                    items[oi_item] = Item::Consumed;
                    unlink(&mut arena, oi);
                }

                let closer_remaining = current_count(&items, ci_item);
                if closer_remaining == 0 {
                    items[ci_item] = Item::Consumed;
                    unlink(&mut arena, ci);
                    closer_idx = arena[ci].next;
                }
                // else: re-examine the same closer; it may match an
                // earlier opener now exposed by the unlink above.
            }
            None => {
                openers_bottom.insert(b, arena[ci].prev);
                if !arena[ci].can_open {
                    unlink(&mut arena, ci);
                }
                closer_idx = arena[ci].next;
            }
        }
    }

    // Flatten: any remaining `Item::Delim` never matched anything and
    // becomes literal text; `Item::Consumed` slots contribute nothing.
    items
        .into_iter()
        .filter_map(|item| match item {
            Item::Node(n) => Some(n),
            Item::Delim(d) => Some(d.into_text_node()),
            Item::Consumed => None,
        })
        .collect()
}

fn current_count(items: &[Item], item_idx: usize) -> usize {
    match &items[item_idx] {
        Item::Delim(d) => d.count(),
        _ => 0,
    }
}

fn unlink(arena: &mut [StackEntry], i: usize) {
    let (p, n) = (arena[i].prev, arena[i].next);
    if let Some(p) = p {
        arena[p].next = n;
    }
    if let Some(n) = n {
        arena[n].prev = p;
    }
}

/// True if `node` is a `Strong` (when wrapping with emphasis) or `Emphasis`
/// (when wrapping with strong) node produced by this same resolver from a
/// 3-length run on both sides — the exact shape the old dedicated
/// "```***text***```" grammar used to special-case as one flat node.
fn is_pure_triple_pair(node: &Node, outer_is_strong: bool) -> bool {
    match node.kind {
        NodeKind::Strong => !outer_is_strong,
        NodeKind::Emphasis => outer_is_strong,
        _ => false,
    }
}
