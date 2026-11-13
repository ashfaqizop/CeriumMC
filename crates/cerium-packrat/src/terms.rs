//! The terms of `Term` and `StringReaderTerms`, and the reader rules of
//! `net.minecraft.util.parsing.packrat.commands`.

use std::cell::Cell;
use std::rc::Rc;

use cerium_brigadier::{Message, StringReader, builtin};
use cerium_java::JString;

use crate::{
    Atom, Control, NamedRule, NamedRuleInner, ParseState, Rule, Suggestions, Term, TermRef, Value,
};

/// `Dictionary.Reference`: parses a named rule and stores its value.
pub(crate) struct Reference {
    pub(crate) rule: Rc<NamedRuleInner>,
    pub(crate) store: u32,
}

impl Term for Reference {
    fn parse(&self, state: &mut ParseState<'_>, _silent: bool, _control: Control<'_>) -> bool {
        match state.parse_inner(&self.rule) {
            Some(value) => {
                state.scope_mut().put_raw(self.store, Some(value));
                true
            }
            None => false,
        }
    }
}

struct Marker {
    name: u32,
    value: Value,
}

impl Term for Marker {
    fn parse(&self, state: &mut ParseState<'_>, _silent: bool, _control: Control<'_>) -> bool {
        state
            .scope_mut()
            .put_raw(self.name, Some(Rc::clone(&self.value)));
        true
    }
}

/// `Term.marker(name, value)`: stores `value` under `name`.
pub fn marker<T: 'static>(name: Atom<T>, value: T) -> TermRef {
    Rc::new(Marker {
        name: name.id(),
        value: Rc::new(value),
    })
}

struct Sequence(Vec<TermRef>);

impl Term for Sequence {
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, control: Control<'_>) -> bool {
        let mark = state.mark();
        for element in &self.0 {
            if !element.parse(state, silent, control) {
                state.restore(mark);
                return false;
            }
        }
        true
    }
}

/// `Term.sequence(terms...)`.
#[must_use]
pub fn sequence(terms: Vec<TermRef>) -> TermRef {
    Rc::new(Sequence(terms))
}

struct Alternative(Vec<TermRef>);

impl Term for Alternative {
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, _control: Control<'_>) -> bool {
        let control_for_this = Cell::new(false);
        let mark = state.mark();
        state.scope_mut().split_frame();
        for element in &self.0 {
            if element.parse(state, silent, Some(&control_for_this)) {
                state.scope_mut().merge_frame();
                return true;
            }
            state.scope_mut().clear_frame_values();
            state.restore(mark);
            if control_for_this.get() {
                break;
            }
        }
        state.scope_mut().pop_frame();
        false
    }
}

/// `Term.alternative(terms...)`.
#[must_use]
pub fn alternative(terms: Vec<TermRef>) -> TermRef {
    Rc::new(Alternative(terms))
}

struct Maybe(TermRef);

impl Term for Maybe {
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, control: Control<'_>) -> bool {
        let mark = state.mark();
        if !self.0.parse(state, silent, control) {
            state.restore(mark);
        }
        true
    }
}

/// `Term.optional(term)`.
#[must_use]
pub fn optional(term: TermRef) -> TermRef {
    Rc::new(Maybe(term))
}

struct LookAhead {
    term: TermRef,
    positive: bool,
}

impl Term for LookAhead {
    fn parse(&self, state: &mut ParseState<'_>, _silent: bool, control: Control<'_>) -> bool {
        let mark = state.mark();
        // `state.silent()`: errors recorded directly by the term are dropped.
        let result = self.term.parse(state, true, control);
        state.restore(mark);
        self.positive == result
    }
}

/// `Term.positiveLookahead(term)`.
#[must_use]
pub fn positive_lookahead(term: TermRef) -> TermRef {
    Rc::new(LookAhead {
        term,
        positive: true,
    })
}

/// `Term.negativeLookahead(term)`.
#[must_use]
pub fn negative_lookahead(term: TermRef) -> TermRef {
    Rc::new(LookAhead {
        term,
        positive: false,
    })
}

struct Cut;

impl Term for Cut {
    fn parse(&self, _state: &mut ParseState<'_>, _silent: bool, control: Control<'_>) -> bool {
        if let Some(control) = control {
            control.set(true);
        }
        true
    }
}

/// `Term.cut()`: commits the innermost alternative to its current branch.
#[must_use]
pub fn cut() -> TermRef {
    Rc::new(Cut)
}

struct Empty;

impl Term for Empty {
    fn parse(&self, _state: &mut ParseState<'_>, _silent: bool, _control: Control<'_>) -> bool {
        true
    }
}

/// `Term.empty()`.
#[must_use]
pub fn empty() -> TermRef {
    Rc::new(Empty)
}

struct Fail(Message);

impl Term for Fail {
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, _control: Control<'_>) -> bool {
        let cursor = state.mark();
        state.store_error(silent, cursor, Suggestions::None, self.0.clone());
        false
    }
}

/// `Term.fail(reason)`.
#[must_use]
pub fn fail(reason: Message) -> TermRef {
    Rc::new(Fail(reason))
}

/// Parses a named rule from a term (`state.parse(element)`, which always runs on the main
/// state).
fn parse_element(state: &mut ParseState<'_>, element: &Rc<NamedRuleInner>) -> Option<Value> {
    state.parse_inner(element)
}

struct Repeated {
    element: Rc<NamedRuleInner>,
    list: u32,
    min: usize,
}

impl Term for Repeated {
    fn parse(&self, state: &mut ParseState<'_>, _silent: bool, _control: Control<'_>) -> bool {
        let mark = state.mark();
        let mut elements: Vec<Value> = Vec::new();
        loop {
            let entry_mark = state.mark();
            let Some(parsed) = parse_element(state, &self.element) else {
                state.restore(entry_mark);
                if elements.len() < self.min {
                    state.restore(mark);
                    return false;
                }
                state
                    .scope_mut()
                    .put_raw(self.list, Some(Rc::new(ValueList(elements))));
                return true;
            };
            elements.push(parsed);
        }
    }
}

/// The type-erased list a repetition stores; read it with [`list`].
pub struct ValueList(Vec<Value>);

/// The elements of a repetition's list atom, typed.
///
/// # Panics
///
/// If the list holds other types than `T`.
#[must_use]
pub fn list<T: Clone + 'static>(scope: &crate::Scope, name: Atom<Vec<T>>) -> Option<Vec<T>> {
    let value = scope.get_value(name.id())?;
    let list = value
        .downcast_ref::<ValueList>()
        .expect("a repetition's list");
    Some(
        list.0
            .iter()
            .map(|element| element.downcast_ref::<T>().expect("element type").clone())
            .collect(),
    )
}

/// `Term.repeated(element, listName, minRepetitions)`.
#[must_use]
pub fn repeated<T: 'static>(
    element: &NamedRule<T>,
    list_name: Atom<Vec<T>>,
    min: usize,
) -> TermRef {
    Rc::new(Repeated {
        element: Rc::clone(element.inner()),
        list: list_name.id(),
        min,
    })
}

struct RepeatedWithSeparator {
    element: Rc<NamedRuleInner>,
    list: u32,
    separator: TermRef,
    min: usize,
    allow_trailing_separator: bool,
}

impl Term for RepeatedWithSeparator {
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, control: Control<'_>) -> bool {
        let list_mark = state.mark();
        let mut elements: Vec<Value> = Vec::new();
        let mut first = true;
        loop {
            let mark_before_separator = state.mark();
            if !first && !self.separator.parse(state, silent, control) {
                state.restore(mark_before_separator);
                break;
            }
            let mark_after_separator = state.mark();
            let Some(parsed) = parse_element(state, &self.element) else {
                if first || self.allow_trailing_separator {
                    state.restore(mark_after_separator);
                } else {
                    state.restore(list_mark);
                    return false;
                }
                break;
            };
            elements.push(parsed);
            first = false;
        }
        if elements.len() < self.min {
            state.restore(list_mark);
            return false;
        }
        state
            .scope_mut()
            .put_raw(self.list, Some(Rc::new(ValueList(elements))));
        true
    }
}

/// `Term.repeatedWithTrailingSeparator(element, listName, separator)`.
#[must_use]
pub fn repeated_with_trailing_separator<T: 'static>(
    element: &NamedRule<T>,
    list_name: Atom<Vec<T>>,
    separator: TermRef,
) -> TermRef {
    Rc::new(RepeatedWithSeparator {
        element: Rc::clone(element.inner()),
        list: list_name.id(),
        separator,
        min: 0,
        allow_trailing_separator: true,
    })
}

/// `Term.repeatedWithoutTrailingSeparator(element, listName, separator)`.
#[must_use]
pub fn repeated_without_trailing_separator<T: 'static>(
    element: &NamedRule<T>,
    list_name: Atom<Vec<T>>,
    separator: TermRef,
) -> TermRef {
    Rc::new(RepeatedWithSeparator {
        element: Rc::clone(element.inner()),
        list: list_name.id(),
        separator,
        min: 0,
        allow_trailing_separator: false,
    })
}

/// `StringReaderTerms.TerminalCharacters`: skips whitespace, then reads one character that
/// must be accepted. Like Java, a rejected character is consumed; callers restore.
struct TerminalCharacters<F> {
    accept: F,
    error: Message,
    suggestions: Suggestions,
}

impl<F: Fn(u16) -> bool> Term for TerminalCharacters<F> {
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, _control: Control<'_>) -> bool {
        state.input().skip_whitespace();
        let cursor = state.mark();
        if state.input().can_read() && (self.accept)(state.input().read()) {
            return true;
        }
        state.store_error(silent, cursor, self.suggestions.clone(), self.error.clone());
        false
    }
}

fn unit_string(c: u16) -> JString {
    JString::from_utf16(&[c])
}

/// A terminal with an arbitrary predicate; `values` are the characters it lists in its error
/// (joined with `|`) and suggestions.
pub fn terminal_characters(values: &[u16], accept: impl Fn(u16) -> bool + 'static) -> TermRef {
    let mut joined: Vec<u16> = Vec::new();
    for (i, &c) in values.iter().enumerate() {
        if i > 0 {
            joined.push(u16::from(b'|'));
        }
        joined.push(c);
    }
    Rc::new(TerminalCharacters {
        accept,
        error: builtin::literal_incorrect(JString::from_utf16(&joined)),
        suggestions: Suggestions::Values(values.iter().map(|&c| unit_string(c)).collect()),
    })
}

/// `StringReaderTerms.character(c)`.
#[must_use]
pub fn character(c: char) -> TermRef {
    let unit = c as u16;
    terminal_characters(&[unit], move |v| v == unit)
}

/// `StringReaderTerms.characters(c1, c2)`.
#[must_use]
pub fn characters(c1: char, c2: char) -> TermRef {
    let (a, b) = (c1 as u16, c2 as u16);
    terminal_characters(&[a, b], move |v| v == a || v == b)
}

struct TerminalWord {
    value: JString,
    error: Message,
    suggestions: Suggestions,
}

impl Term for TerminalWord {
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, _control: Control<'_>) -> bool {
        state.input().skip_whitespace();
        let cursor = state.mark();
        let value = state.input().read_unquoted_string();
        if value != self.value {
            state.store_error(silent, cursor, self.suggestions.clone(), self.error.clone());
            return false;
        }
        true
    }
}

/// `StringReaderTerms.word(value)`.
#[must_use]
pub fn word(value: &str) -> TermRef {
    let value = JString::from(value);
    Rc::new(TerminalWord {
        error: builtin::literal_incorrect(value.clone()),
        suggestions: Suggestions::Values(Rc::from([value.clone()])),
        value,
    })
}

/// `GreedyPredicateParseRule`: between `min` and `max` accepted characters, from the cursor
/// (no whitespace skipping).
pub struct GreedyPredicateParseRule<F> {
    min: usize,
    max: usize,
    error: Message,
    accept: F,
}

impl<F: Fn(u16) -> bool> GreedyPredicateParseRule<F> {
    /// A rule taking at least `min` and at most `max` characters.
    pub fn new(min: usize, max: usize, error: Message, accept: F) -> Self {
        Self {
            min,
            max,
            error,
            accept,
        }
    }
}

impl<F: Fn(u16) -> bool> Rule for GreedyPredicateParseRule<F> {
    fn parse(&self, state: &mut ParseState<'_>) -> Option<Value> {
        let reader = state.reader();
        let full = reader.string();
        let start = reader.cursor();
        let mut pos = start;
        while pos < full.len() && (self.accept)(full.char_at(pos)) && pos - start < self.max {
            pos += 1;
        }
        if pos - start < self.min {
            state.error_here(self.error.clone());
            return None;
        }
        let text = full.substring(start, pos);
        state.input().set_cursor(pos);
        Some(Rc::new(text))
    }
}

/// `NumberRunParseRule`: skips whitespace, then a non-empty run of accepted characters that
/// neither starts nor ends with `_`.
pub struct NumberRunParseRule<F> {
    no_value_error: Message,
    underscore_not_allowed_error: Message,
    accept: F,
}

impl<F: Fn(u16) -> bool> NumberRunParseRule<F> {
    /// A rule with its two errors and its character class.
    pub fn new(no_value_error: Message, underscore_not_allowed_error: Message, accept: F) -> Self {
        Self {
            no_value_error,
            underscore_not_allowed_error,
            accept,
        }
    }
}

impl<F: Fn(u16) -> bool> Rule for NumberRunParseRule<F> {
    fn parse(&self, state: &mut ParseState<'_>) -> Option<Value> {
        state.input().skip_whitespace();
        let reader = state.reader();
        let full = reader.string();
        let start = reader.cursor();
        let mut pos = start;
        while pos < full.len() && (self.accept)(full.char_at(pos)) {
            pos += 1;
        }
        if pos == start {
            state.error_here(self.no_value_error.clone());
            return None;
        }
        let underscore = u16::from(b'_');
        if full.char_at(start) == underscore || full.char_at(pos - 1) == underscore {
            state.error_here(self.underscore_not_allowed_error.clone());
            return None;
        }
        let text = full.substring(start, pos);
        state.input().set_cursor(pos);
        Some(Rc::new(text))
    }
}

/// `UnquotedStringParseRule`: skips whitespace, then reads an unquoted string of at least
/// `min` characters.
pub struct UnquotedStringParseRule {
    min: usize,
    error: Message,
}

impl UnquotedStringParseRule {
    /// A rule requiring `min` characters.
    #[must_use]
    pub fn new(min: usize, error: Message) -> Self {
        Self { min, error }
    }
}

impl Rule for UnquotedStringParseRule {
    fn parse(&self, state: &mut ParseState<'_>) -> Option<Value> {
        state.input().skip_whitespace();
        let cursor = state.mark();
        let value = state.input().read_unquoted_string();
        if value.len() < self.min {
            state.store_error(false, cursor, Suggestions::None, self.error.clone());
            return None;
        }
        Some(Rc::new(value))
    }
}

/// Reads the whole input text of a reader (for rules that match patterns).
#[must_use]
pub fn reader_text(reader: &StringReader) -> &JString {
    reader.string()
}
