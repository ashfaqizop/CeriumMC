//! [`ParseState`]: `CachedParseState` over a Brigadier `StringReader`, with its memo cache and
//! `ErrorCollector.LongestOnly`.

use std::rc::Rc;

use cerium_brigadier::{Message, StringReader};

use crate::{ErrorEntry, NamedRule, NamedRuleInner, Scope, Suggestions, Value};

/// A memoized rule result at a position: `None` while the rule is being parsed there.
enum CacheEntry {
    /// The rule failed (`CacheEntry.NEGATIVE`).
    Negative,
    /// The rule succeeded, ending at `mark_after`.
    Value { value: Value, mark_after: usize },
}

/// `ErrorCollector.LongestOnly`: keeps the errors recorded at the furthest cursor.
struct LongestOnly {
    entries: Vec<ErrorEntry>,
    last_cursor: i64,
}

impl LongestOnly {
    fn discard_errors_from_shorter_parse(&mut self, cursor: i64) {
        if cursor > self.last_cursor {
            self.last_cursor = cursor;
            self.entries.clear();
        }
    }

    fn store(&mut self, cursor: usize, suggestions: Suggestions, reason: Message) {
        let cursor_i = i64::try_from(cursor).unwrap_or(i64::MAX);
        self.discard_errors_from_shorter_parse(cursor_i);
        if cursor_i == self.last_cursor {
            self.entries.push(ErrorEntry {
                cursor,
                suggestions,
                reason,
            });
        }
    }
}

/// `StringReaderParserState`: the reader, the scope, the memo cache and the errors.
pub struct ParseState<'r> {
    reader: &'r mut StringReader,
    scope: Scope,
    /// Per position: the rules tried there (by atom id) and their results, in the order they
    /// were first tried (`PositionCache`).
    cache: Vec<Vec<(u32, Option<CacheEntry>)>>,
    errors: LongestOnly,
}

impl<'r> ParseState<'r> {
    /// A state reading from `reader`'s cursor.
    pub fn new(reader: &'r mut StringReader) -> Self {
        Self {
            reader,
            scope: Scope::new(),
            cache: Vec::new(),
            errors: LongestOnly {
                entries: Vec::new(),
                last_cursor: -1,
            },
        }
    }

    /// The input (`input()`).
    pub fn input(&mut self) -> &mut StringReader {
        self.reader
    }

    /// The input, read-only.
    #[must_use]
    pub fn reader(&self) -> &StringReader {
        self.reader
    }

    /// The current position (`mark()`).
    #[must_use]
    pub fn mark(&self) -> usize {
        self.reader.cursor()
    }

    /// Moves back to a position (`restore`).
    pub fn restore(&mut self, mark: usize) {
        self.reader.set_cursor(mark);
    }

    /// The scope (`scope()`).
    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// The scope, mutably.
    pub fn scope_mut(&mut self) -> &mut Scope {
        &mut self.scope
    }

    /// `errorCollector().store(cursor, suggestions, reason)`, unless `silent`.
    pub fn store_error(
        &mut self,
        silent: bool,
        cursor: usize,
        suggestions: Suggestions,
        reason: Message,
    ) {
        if !silent {
            self.errors.store(cursor, suggestions, reason);
        }
    }

    /// `errorCollector().store(mark(), reason)` from a rule action, which always runs on the
    /// main (not silent) state.
    pub fn error_here(&mut self, reason: Message) {
        let cursor = self.mark();
        self.store_error(false, cursor, Suggestions::None, reason);
    }

    /// `errorCollector().store(mark(), suggestions, reason)` from a rule action.
    pub fn error_here_with(&mut self, suggestions: Suggestions, reason: Message) {
        let cursor = self.mark();
        self.store_error(false, cursor, suggestions, reason);
    }

    /// `CachedParseState.parse(rule)`: the memoized result of `rule` at the current position.
    pub(crate) fn parse_inner(&mut self, rule: &Rc<NamedRuleInner>) -> Option<Value> {
        let mark_before = self.mark();
        if self.cache.len() <= mark_before {
            self.cache.resize_with(mark_before + 1, Vec::new);
        }
        let position = &self.cache[mark_before];
        let found = position.iter().position(|(id, _)| *id == rule.id);
        let index = if let Some(index) = found {
            match &position[index].1 {
                Some(CacheEntry::Negative) => return None,
                Some(CacheEntry::Value { value, mark_after }) => {
                    let (value, mark_after) = (Rc::clone(value), *mark_after);
                    self.restore(mark_after);
                    return Some(value);
                }
                // Being parsed at this position already: parse again.
                None => {}
            }
            index
        } else {
            self.cache[mark_before].push((rule.id, None));
            self.cache[mark_before].len() - 1
        };
        let body = rule.rule.borrow().clone().expect("bound rule");
        let result = body.parse(self);
        let entry = match &result {
            None => CacheEntry::Negative,
            Some(value) => CacheEntry::Value {
                value: Rc::clone(value),
                mark_after: self.mark(),
            },
        };
        self.cache[mark_before][index].1 = Some(entry);
        result
    }

    /// `parse(rule)`: the rule's value at the current position.
    ///
    /// # Panics
    ///
    /// If the rule produced another type than its atom's (a grammar bug).
    pub fn parse<T: Clone + 'static>(&mut self, rule: &NamedRule<T>) -> Option<T> {
        self.parse_inner(rule.inner())
            .map(|value| value.downcast_ref::<T>().expect("rule type").clone())
    }

    /// `parseTopRule`: parses `rule`, then (on success) drops errors recorded before the end.
    ///
    /// # Panics
    ///
    /// If the scope was left with extra frames (`"Malformed scope"`), which a correct grammar
    /// never does.
    pub fn parse_top_rule<T: Clone + 'static>(&mut self, rule: &NamedRule<T>) -> Option<T> {
        let result = self.parse(rule);
        if result.is_some() {
            let mark = i64::try_from(self.mark()).unwrap_or(i64::MAX);
            self.errors.discard_errors_from_shorter_parse(mark);
        }
        assert!(self.scope.has_only_single_frame(), "Malformed scope");
        result
    }

    /// The recorded errors (`LongestOnly.entries()`).
    #[must_use]
    pub fn into_errors(self) -> Vec<ErrorEntry> {
        self.errors.entries
    }

    /// The furthest error cursor, or -1 (`LongestOnly.cursor()`).
    #[must_use]
    pub fn error_cursor(&self) -> i64 {
        self.errors.last_cursor
    }
}
