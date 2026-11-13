//! A faithful port of vanilla's packrat parser (`net.minecraft.util.parsing.packrat`), which
//! vanilla's SNBT grammar and other command parsers are built on.
//!
//! It is ported rather than replaced because its mechanics decide which error a player sees:
//!
//! - every named rule's result is memoized per input position, including failures, so a rule
//!   reports its errors only the first time it is tried at a position;
//! - errors are kept only for the furthest position reached (`ErrorCollector.LongestOnly`);
//! - terms inside a lookahead are "silent" (their errors are dropped), but named rules they
//!   call are not, because those run on the main parse state;
//! - alternatives work on a split copy of the scope's frame and merge it back by position.
//!
//! Values flow through the scope type-erased (`Rc<dyn Any>`), as Java's `Object`s do;
//! [`Atom`]s give them static types at the edges.

mod scope;
mod state;
pub mod terms;

use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use cerium_brigadier::{CommandSyntaxException, Message, StringReader};
use cerium_java::JString;

pub use scope::Scope;
pub use state::ParseState;

/// A parsed value, type-erased.
pub type Value = Rc<dyn Any>;

/// `Atom<T>`: a name for values of type `T` in the scope. Atoms compare by identity: two
/// atoms made with the same name are different.
pub struct Atom<T> {
    id: u32,
    name: &'static str,
    _type: PhantomData<fn() -> T>,
}

impl<T> Clone for Atom<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Atom<T> {}

impl<T> std::fmt::Debug for Atom<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}>", self.name)
    }
}

impl<T: 'static> Atom<T> {
    /// `Atom.of(name)`: a new atom.
    #[must_use]
    pub fn of(name: &'static str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            name,
            _type: PhantomData,
        }
    }

    /// The atom's name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn id(&self) -> u32 {
        self.id
    }
}

/// Wraps a value for the scope or a marker.
pub fn value<T: 'static>(value: T) -> Value {
    Rc::new(value)
}

/// Suggestions offered where an error was recorded (`SuggestionSupplier`).
#[derive(Clone, Debug, Default)]
pub enum Suggestions {
    /// None (`SuggestionSupplier.empty()`).
    #[default]
    None,
    /// A fixed set of values.
    Values(Rc<[JString]>),
}

/// `ErrorEntry`: an error recorded at a cursor.
#[derive(Clone, Debug)]
pub struct ErrorEntry {
    /// Where the error was recorded.
    pub cursor: usize,
    /// What could be typed there.
    pub suggestions: Suggestions,
    /// The `DelayedException`'s message.
    pub reason: Message,
}

/// `Control`: whether a cut was made in the innermost alternative. `None` is
/// `Control.UNBOUND`, on which a cut has no effect.
pub type Control<'a> = Option<&'a std::cell::Cell<bool>>;

/// `Term<S>`: a parsing step on the current scope frame.
pub trait Term {
    /// Parses at the current position. `silent` drops the errors this term records directly
    /// (inside a lookahead). On failure the term restores what it consumed only where Java
    /// does; callers restore positions as Java's do.
    fn parse(&self, state: &mut ParseState<'_>, silent: bool, control: Control<'_>) -> bool;
}

/// `Rule<S, T>`: produces a value, or `None` (Java's `null`) to fail.
pub trait Rule {
    /// Parses at the current position.
    fn parse(&self, state: &mut ParseState<'_>) -> Option<Value>;
}

/// A shared term.
pub type TermRef = Rc<dyn Term>;

/// `Dictionary.Entry` / `NamedRule<S, T>`: a rule with a name, bound once, possibly after
/// being referenced (`Dictionary.forward`).
pub struct NamedRule<T> {
    inner: Rc<NamedRuleInner>,
    _type: PhantomData<fn() -> T>,
}

impl<T> Clone for NamedRule<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
            _type: PhantomData,
        }
    }
}

pub(crate) struct NamedRuleInner {
    id: u32,
    name: &'static str,
    rule: RefCell<Option<Rc<dyn Rule>>>,
}

impl<T: 'static> NamedRule<T> {
    /// The rule's atom name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.inner.name
    }

    pub(crate) fn inner(&self) -> &Rc<NamedRuleInner> {
        &self.inner
    }
}

/// `Dictionary<S>`: the named rules of a grammar.
#[derive(Default)]
pub struct Dictionary {
    entries: Vec<Rc<NamedRuleInner>>,
}

impl Dictionary {
    /// An empty dictionary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn entry<T: 'static>(&mut self, name: Atom<T>) -> NamedRule<T> {
        let inner = if let Some(existing) = self.entries.iter().find(|e| e.id == name.id()) {
            Rc::clone(existing)
        } else {
            let inner = Rc::new(NamedRuleInner {
                id: name.id(),
                name: name.name(),
                rule: RefCell::new(None),
            });
            self.entries.push(Rc::clone(&inner));
            inner
        };
        NamedRule {
            inner,
            _type: PhantomData,
        }
    }

    /// `put(name, rule)`: binds the rule for `name`.
    ///
    /// # Panics
    ///
    /// If `name` already has a rule (`"Trying to override rule"`).
    pub fn put<T: 'static>(&mut self, name: Atom<T>, rule: Rc<dyn Rule>) -> NamedRule<T> {
        let named = self.entry(name);
        let mut slot = named.inner.rule.borrow_mut();
        assert!(slot.is_none(), "Trying to override rule: {name:?}");
        *slot = Some(rule);
        drop(slot);
        named
    }

    /// `put(name, term, scope -> value)`: a rule from a term and an action on its scope.
    pub fn put_simple<T: 'static>(
        &mut self,
        name: Atom<T>,
        term: TermRef,
        action: impl Fn(&Scope) -> T + 'static,
    ) -> NamedRule<T> {
        self.put(
            name,
            Rc::new(WrappedTerm {
                term,
                action: move |state: &mut ParseState<'_>| Some(value(action(state.scope()))),
            }),
        )
    }

    /// `putComplex(name, term, state -> value or null)`.
    pub fn put_complex<T: 'static>(
        &mut self,
        name: Atom<T>,
        term: TermRef,
        action: impl Fn(&mut ParseState<'_>) -> Option<T> + 'static,
    ) -> NamedRule<T> {
        self.put(
            name,
            Rc::new(WrappedTerm {
                term,
                action: move |state: &mut ParseState<'_>| action(state).map(value),
            }),
        )
    }

    /// `forward(name)`: the named rule, bound now or later.
    pub fn forward<T: 'static>(&mut self, name: Atom<T>) -> NamedRule<T> {
        self.entry(name)
    }

    /// `named(name)`: a term parsing the rule and storing its value under `name`.
    pub fn named<T: 'static>(&mut self, name: Atom<T>) -> TermRef {
        self.named_with_alias(name, name)
    }

    /// `namedWithAlias(toParse, toStore)`.
    pub fn named_with_alias<T: 'static>(&mut self, to_parse: Atom<T>, to_store: Atom<T>) -> TermRef {
        let rule = self.entry(to_parse);
        Rc::new(terms::Reference {
            rule: Rc::clone(rule.inner()),
            store: to_store.id(),
        })
    }

    /// `checkAllBound`.
    ///
    /// # Panics
    ///
    /// If a referenced rule was never bound.
    pub fn check_all_bound(&self) {
        let unbound: Vec<&str> = self
            .entries
            .iter()
            .filter(|e| e.rule.borrow().is_none())
            .map(|e| e.name)
            .collect();
        assert!(unbound.is_empty(), "Unbound names: {unbound:?}");
    }
}

/// `Rule.WrappedTerm`: parses the term in a new scope frame, then runs the action.
struct WrappedTerm<F> {
    term: TermRef,
    action: F,
}

impl<F: Fn(&mut ParseState<'_>) -> Option<Value>> Rule for WrappedTerm<F> {
    fn parse(&self, state: &mut ParseState<'_>) -> Option<Value> {
        state.scope_mut().push_frame();
        let result = if self.term.parse(state, false, None) {
            (self.action)(state)
        } else {
            None
        };
        state.scope_mut().pop_frame();
        result
    }
}

/// Why a whole parse failed (`Grammar.parseForCommands`).
#[derive(Clone, Debug, PartialEq)]
pub enum ParseFailure {
    /// The first recorded error, as Brigadier's exception.
    Syntax(CommandSyntaxException),
    /// No error was recorded (`IllegalStateException("Failed to parse: ")`).
    NoErrors,
}

/// `Grammar<T>`: a dictionary with a top rule, parsing Brigadier input.
pub struct Grammar<T> {
    rules: Dictionary,
    top: NamedRule<T>,
}

impl<T: Clone + 'static> Grammar<T> {
    /// A grammar; every referenced rule must be bound.
    ///
    /// # Panics
    ///
    /// If a rule is unbound.
    #[must_use]
    pub fn new(rules: Dictionary, top: NamedRule<T>) -> Self {
        rules.check_all_bound();
        Self { rules, top }
    }

    /// The dictionary.
    #[must_use]
    pub fn rules(&self) -> &Dictionary {
        &self.rules
    }

    /// `parseForCommands(reader)`: parses from the reader's cursor and leaves the cursor after
    /// what was parsed.
    ///
    /// # Errors
    ///
    /// The first error recorded at the furthest position reached, as an exception at that
    /// position.
    pub fn parse_for_commands(&self, reader: &mut StringReader) -> Result<T, ParseFailure> {
        let input = reader.string().clone();
        let mut state = ParseState::new(reader);
        if let Some(result) = state.parse_top_rule(&self.top) {
            return Ok(result);
        }
        let entries = state.into_errors();
        match entries.into_iter().next() {
            Some(entry) => Err(ParseFailure::Syntax(CommandSyntaxException::at(
                entry.reason,
                input,
                entry.cursor,
            ))),
            None => Err(ParseFailure::NoErrors),
        }
    }

    /// `parseForSuggestions`: the position where suggestions apply and the suggested values
    /// recorded there (Brigadier's `SuggestionsBuilder` filtering comes with commands).
    #[must_use]
    pub fn parse_for_suggestions(&self, input: JString, start: usize) -> (usize, Vec<JString>) {
        let mut reader = StringReader::new(input);
        reader.set_cursor(start);
        let mut state = ParseState::new(&mut reader);
        let _ = state.parse_top_rule(&self.top);
        let cursor = state.error_cursor();
        let mut values = Vec::new();
        if cursor < 0 {
            return (start, values);
        }
        for entry in state.into_errors() {
            if let Suggestions::Values(list) = entry.suggestions {
                values.extend(list.iter().cloned());
            }
        }
        (cursor as usize, values)
    }
}
