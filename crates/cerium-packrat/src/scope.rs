//! [`Scope`]: the stack of named values rules build their results from.

use std::rc::Rc;

use crate::{Atom, Value};

/// A slot's key: a frame marker (remembering the previous marker) or an atom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Frame { previous_marker: usize },
    Atom(u32),
}

#[derive(Clone)]
struct Slot {
    key: Key,
    value: Option<Value>,
}

/// `Scope`: key/value slots in frames, searched from the top of the current frame down.
///
/// The slot arithmetic is Java's (one slot here is a key/value pair of its array), including
/// `mergeFrame`, which writes a split frame back over its parent position by position.
pub struct Scope {
    stack: Vec<Slot>,
    top_entry: usize,
    top_marker: usize,
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Scope {
    /// A scope with the single root frame.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: vec![Slot {
                key: Key::Frame { previous_marker: 0 },
                value: None,
            }],
            top_entry: 0,
            top_marker: 0,
        }
    }

    fn set(&mut self, index: usize, slot: Slot) {
        if index < self.stack.len() {
            self.stack[index] = slot;
        } else {
            debug_assert_eq!(index, self.stack.len());
            self.stack.push(slot);
        }
    }

    fn previous_marker(&self, marker: usize) -> usize {
        match self.stack[marker].key {
            Key::Frame { previous_marker } => previous_marker,
            Key::Atom(_) => unreachable!("not a frame marker"),
        }
    }

    fn value_index(&self, id: u32) -> Option<usize> {
        (self.top_marker + 1..=self.top_entry)
            .rev()
            .find(|&i| self.stack[i].key == Key::Atom(id))
    }

    fn value_index_for_any(&self, ids: &[u32]) -> Option<usize> {
        (self.top_marker + 1..=self.top_entry)
            .rev()
            .find(|&i| ids.iter().any(|&id| self.stack[i].key == Key::Atom(id)))
    }

    fn setup_new_frame(&mut self) {
        self.top_entry += 1;
        let marker = Slot {
            key: Key::Frame {
                previous_marker: self.top_marker,
            },
            value: None,
        };
        self.set(self.top_entry, marker);
        self.top_marker = self.top_entry;
    }

    /// `pushFrame`.
    pub fn push_frame(&mut self) {
        self.setup_new_frame();
    }

    /// `popFrame`.
    pub fn pop_frame(&mut self) {
        debug_assert_ne!(self.top_marker, 0);
        self.top_entry = self.top_marker - 1;
        self.top_marker = self.previous_marker(self.top_marker);
    }

    /// `splitFrame`: a new frame with the current frame's keys and no values.
    pub fn split_frame(&mut self) {
        let current_marker = self.top_marker;
        let entries = self.top_entry - self.top_marker;
        self.setup_new_frame();
        for i in 0..entries {
            let key = self.stack[current_marker + 1 + i].key;
            self.top_entry += 1;
            self.set(self.top_entry, Slot { key, value: None });
        }
    }

    /// `clearFrameValues`.
    pub fn clear_frame_values(&mut self) {
        for i in self.top_marker + 1..=self.top_entry {
            self.stack[i].value = None;
        }
    }

    /// `mergeFrame`: writes the current frame over its parent slot by slot (a different key
    /// replaces the slot, the same key takes a non-null value), then pops it.
    pub fn merge_frame(&mut self) {
        let previous_marker = self.previous_marker(self.top_marker);
        let mut previous_cursor = previous_marker;
        let mut current_cursor = self.top_marker;
        while current_cursor < self.top_entry {
            previous_cursor += 1;
            current_cursor += 1;
            let new_key = self.stack[current_cursor].key;
            let new_value = self.stack[current_cursor].value.clone();
            let slot = &mut self.stack[previous_cursor];
            if slot.key != new_key {
                *slot = Slot {
                    key: new_key,
                    value: new_value,
                };
            } else if new_value.is_some() {
                slot.value = new_value;
            }
        }
        self.top_entry = previous_cursor;
        self.top_marker = previous_marker;
    }

    pub(crate) fn put_raw(&mut self, id: u32, value: Option<Value>) {
        if let Some(index) = self.value_index(id) {
            self.stack[index].value = value;
        } else {
            self.top_entry += 1;
            self.set(
                self.top_entry,
                Slot {
                    key: Key::Atom(id),
                    value,
                },
            );
        }
    }

    /// `put(name, value)`.
    pub fn put<T: 'static>(&mut self, name: Atom<T>, value: T) {
        self.put_raw(name.id(), Some(Rc::new(value)));
    }

    /// The raw value in the current frame, if set.
    pub(crate) fn get_value(&self, id: u32) -> Option<Value> {
        self.get_raw(id).cloned()
    }

    fn get_raw(&self, id: u32) -> Option<&Value> {
        self.value_index(id)
            .and_then(|i| self.stack[i].value.as_ref())
    }

    /// `get(name)`: the value in the current frame, if set.
    ///
    /// # Panics
    ///
    /// If the stored value has another type than the atom's (a grammar bug).
    #[must_use]
    pub fn get<T: Clone + 'static>(&self, name: Atom<T>) -> Option<T> {
        self.get_raw(name.id())
            .map(|value| value.downcast_ref::<T>().expect("atom type").clone())
    }

    /// `getOrThrow(name)`.
    ///
    /// # Panics
    ///
    /// If the atom has no value (Java's `IllegalArgumentException`).
    #[must_use]
    pub fn get_or_throw<T: Clone + 'static>(&self, name: Atom<T>) -> T {
        self.get(name)
            .unwrap_or_else(|| panic!("No value for atom {name:?}"))
    }

    /// `getOrDefault(name, fallback)`. Like Java, a slot that exists with a null value yields
    /// that null, which here means the fallback is not used: `None`.
    ///
    /// # Panics
    ///
    /// If the stored value has another type than the atom's (a grammar bug).
    #[must_use]
    pub fn get_or_default<T: Clone + 'static>(&self, name: Atom<T>, fallback: T) -> Option<T> {
        match self.value_index(name.id()) {
            None => Some(fallback),
            Some(i) => self.stack[i]
                .value
                .as_ref()
                .map(|value| value.downcast_ref::<T>().expect("atom type").clone()),
        }
    }

    /// `getAny(names...)`: the topmost value stored under any of the atoms.
    ///
    /// # Panics
    ///
    /// If the stored value has another type than the atom's (a grammar bug).
    #[must_use]
    pub fn get_any<T: Clone + 'static>(&self, names: &[Atom<T>]) -> Option<T> {
        let ids: Vec<u32> = names.iter().map(Atom::id).collect();
        self.value_index_for_any(&ids)
            .and_then(|i| self.stack[i].value.as_ref())
            .map(|value| value.downcast_ref::<T>().expect("atom type").clone())
    }

    /// `hasOnlySingleFrame`.
    #[must_use]
    pub fn has_only_single_frame(&self) -> bool {
        (1..=self.top_entry)
            .rev()
            .all(|i| !matches!(self.stack[i].key, Key::Frame { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_hide_and_merge() {
        let a: Atom<i32> = Atom::of("a");
        let b: Atom<i32> = Atom::of("b");
        let mut scope = Scope::new();
        scope.push_frame();
        scope.put(a, 1);
        scope.split_frame();
        assert_eq!(scope.get(a), None);
        scope.put(a, 2);
        scope.put(b, 3);
        scope.merge_frame();
        assert_eq!((scope.get(a), scope.get(b)), (Some(2), Some(3)));
        scope.pop_frame();
        assert!(scope.has_only_single_frame());
    }

    #[test]
    fn get_or_default_keeps_a_cleared_slot() {
        let a: Atom<i32> = Atom::of("a");
        let mut scope = Scope::new();
        scope.push_frame();
        assert_eq!(scope.get_or_default(a, 7), Some(7));
        scope.put(a, 1);
        scope.split_frame();
        assert_eq!(scope.get_or_default(a, 7), None);
    }
}
