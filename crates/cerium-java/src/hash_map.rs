//! [`JavaHashMap`]: `java.util.HashMap` of JDK 25, with its exact iteration order.
//!
//! Vanilla writes many maps in the order a `HashMap` iterates them; NBT compounds are the most
//! visible case. That order depends on the keys' `hashCode`s, the table's capacity history and
//! the order of insertions and removals, so this is a transcription of the JDK's
//! implementation rather than a wrapper around Rust's map:
//!
//! - the table doubles when the size exceeds `capacity * 0.75`, and resizing splits each bin
//!   into a "lo" and a "hi" list that keep their relative order;
//! - `put` appends a new key at the end of its bin, but `computeIfAbsent`, `compute` and
//!   `merge` insert it at the front;
//! - a bin of more than 8 keys (in a table of at least 64) becomes a red-black tree. Inserting
//!   into a tree links the new node right after its tree parent, and the root is then moved
//!   to the front of the bin, so tree bins iterate in a different order than list bins.
//!
//! Nodes live in an arena and refer to each other by index, mirroring Java's references.
//!
//! Java breaks ties between keys that are not `Comparable` (or compare as equal without being
//! equal) with `System.identityHashCode`, which is not reproducible even in Java; the port
//! orders such keys as Java does when the identity hash of the new key is the smaller one.
//! Keys like strings never reach that case.

use std::cmp::Ordering;
use std::fmt;

use crate::JString;

/// A key of a [`JavaHashMap`]: Java's `hashCode`, and `compareTo` when the key is
/// `Comparable` to keys of its own class. Equality is Rust's `Eq`, which must agree with
/// Java's `equals`.
pub trait JavaKey: Eq {
    /// The key's Java `hashCode()`.
    fn java_hash_code(&self) -> i32;

    /// `compareTo` if `self` and `other` are instances of the same `Comparable` class
    /// (`HashMap.compareComparables`), else `None`.
    fn java_compare(&self, other: &Self) -> Option<Ordering>;
}

/// A value a [`JavaHashMap`] can be searched with: a key, or something equal to one such as
/// a `&str` for [`JString`] keys.
pub trait Equivalent<K: ?Sized> {
    /// The Java `hashCode()` of the key this value is equal to.
    fn java_hash_code(&self) -> i32;

    /// Whether `key.equals(this)`.
    fn equivalent(&self, key: &K) -> bool;

    /// `compareTo` against `key`, as [`JavaKey::java_compare`].
    fn java_compare(&self, key: &K) -> Option<Ordering>;
}

impl<K: JavaKey> Equivalent<K> for K {
    fn java_hash_code(&self) -> i32 {
        JavaKey::java_hash_code(self)
    }

    fn equivalent(&self, key: &K) -> bool {
        self == key
    }

    fn java_compare(&self, key: &K) -> Option<Ordering> {
        JavaKey::java_compare(self, key)
    }
}

impl JavaKey for JString {
    fn java_hash_code(&self) -> i32 {
        self.hash_code()
    }

    fn java_compare(&self, other: &Self) -> Option<Ordering> {
        Some(self.compare_to(other))
    }
}

impl Equivalent<JString> for str {
    fn java_hash_code(&self) -> i32 {
        self.encode_utf16().fold(0i32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(i32::from(unit))
        })
    }

    fn equivalent(&self, key: &JString) -> bool {
        key.equals_str(self)
    }

    fn java_compare(&self, key: &JString) -> Option<Ordering> {
        Some(self.encode_utf16().cmp(key.units()))
    }
}

/// No node: Java's `null`.
const NIL: u32 = u32::MAX;
const DEFAULT_INITIAL_CAPACITY: usize = 16;
const MAXIMUM_CAPACITY: usize = 1 << 30;
const DEFAULT_LOAD_FACTOR: f32 = 0.75;
const TREEIFY_THRESHOLD: usize = 8;
const UNTREEIFY_THRESHOLD: usize = 6;
const MIN_TREEIFY_CAPACITY: usize = 64;

#[derive(Clone)]
struct Node<K, V> {
    hash: i32,
    key: K,
    value: V,
    next: u32,
    /// Whether this is a `TreeNode`; the remaining fields are only used if so.
    tree: bool,
    parent: u32,
    left: u32,
    right: u32,
    prev: u32,
    red: bool,
}

/// `java.util.HashMap<K, V>` (without `null` keys or values).
///
/// [`Clone`] copies the map's exact state, so a clone iterates in the same order: it stands
/// for sharing the Java object. Java's copying constructor, which can lay keys out
/// differently, is [`JavaHashMap::from_map`].
#[derive(Clone)]
pub struct JavaHashMap<K, V> {
    /// The bins; empty until the first insertion (Java's `null` table).
    table: Vec<u32>,
    nodes: Vec<Option<Node<K, V>>>,
    free: Vec<u32>,
    size: usize,
    /// Java's `threshold`: the next resize size, or the initial capacity before allocation.
    threshold: i32,
    load_factor: f32,
}

/// `HashMap.hash`: the key's hash code with its high half folded into the low half.
fn spread(h: i32) -> i32 {
    h ^ ((h as u32) >> 16) as i32
}

/// `HashMap.tableSizeFor`: the smallest power of two at least `cap` (capped).
fn table_size_for(cap: i32) -> i32 {
    let n = (u32::MAX >> (cap.wrapping_sub(1) as u32).leading_zeros()) as i32;
    if n < 0 {
        1
    } else if n as usize >= MAXIMUM_CAPACITY {
        MAXIMUM_CAPACITY as i32
    } else {
        n + 1
    }
}

impl<K, V> Default for JavaHashMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> JavaHashMap<K, V> {
    /// `new HashMap<>()`: initial capacity 16, load factor 0.75.
    #[must_use]
    pub fn new() -> Self {
        Self {
            table: Vec::new(),
            nodes: Vec::new(),
            free: Vec::new(),
            size: 0,
            threshold: 0,
            load_factor: DEFAULT_LOAD_FACTOR,
        }
    }

    /// `new HashMap<>(initialCapacity)`.
    ///
    /// # Panics
    ///
    /// If `initial_capacity` is negative (Java's `IllegalArgumentException`).
    #[must_use]
    pub fn with_capacity(initial_capacity: i32) -> Self {
        assert!(
            initial_capacity >= 0,
            "Illegal initial capacity: {initial_capacity}"
        );
        let capacity = initial_capacity.min(MAXIMUM_CAPACITY as i32);
        Self {
            threshold: table_size_for(capacity),
            ..Self::new()
        }
    }

    /// The number of entries (`size()`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.size
    }

    /// Whether the map is empty (`isEmpty()`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// The table's current capacity (0 before the first insertion).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.table.len()
    }

    /// `clear()`: removes every entry but keeps the table's capacity, as Java does.
    pub fn clear(&mut self) {
        if !self.table.is_empty() && self.size > 0 {
            self.size = 0;
            self.table.fill(NIL);
        }
        self.nodes.clear();
        self.free.clear();
    }

    /// The entries in Java's iteration order.
    #[must_use]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            map: self,
            index: 0,
            next: NIL,
            remaining: self.size,
        }
    }

    /// The keys in Java's iteration order (`keySet()`).
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }

    /// The values in Java's iteration order (`values()`).
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }

    /// Mutable access to the values in Java's iteration order.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        let order = self.node_order();
        let mut slots: Vec<Option<&mut V>> = self
            .nodes
            .iter_mut()
            .map(|slot| slot.as_mut().map(|node| &mut node.value))
            .collect();
        order
            .into_iter()
            .filter_map(move |i| slots[i as usize].take())
    }

    /// The node indices in iteration order.
    fn node_order(&self) -> Vec<u32> {
        let mut order = Vec::with_capacity(self.size);
        for &head in &self.table {
            let mut e = head;
            while e != NIL {
                order.push(e);
                e = self.node(e).next;
            }
        }
        order
    }

    fn node(&self, i: u32) -> &Node<K, V> {
        self.nodes[i as usize].as_ref().expect("live node")
    }

    fn node_mut(&mut self, i: u32) -> &mut Node<K, V> {
        self.nodes[i as usize].as_mut().expect("live node")
    }

    fn alloc(&mut self, hash: i32, key: K, value: V, next: u32) -> u32 {
        let node = Node {
            hash,
            key,
            value,
            next,
            tree: false,
            parent: NIL,
            left: NIL,
            right: NIL,
            prev: NIL,
            red: false,
        };
        if let Some(i) = self.free.pop() {
            self.nodes[i as usize] = Some(node);
            i
        } else {
            let i = u32::try_from(self.nodes.len()).expect("fewer than 2^32 nodes");
            self.nodes.push(Some(node));
            i
        }
    }

    fn release(&mut self, i: u32) -> Node<K, V> {
        self.free.push(i);
        self.nodes[i as usize].take().expect("live node")
    }

    fn bin_index(&self, hash: i32) -> usize {
        (self.table.len() - 1) & hash as u32 as usize
    }

    /// `HashMap.resize`.
    fn resize(&mut self)
    where
        K: JavaKey,
    {
        let old_cap = self.table.len();
        let old_thr = self.threshold;
        let new_cap;
        let mut new_thr = 0i32;
        if old_cap > 0 {
            if old_cap >= MAXIMUM_CAPACITY {
                self.threshold = i32::MAX;
                return;
            }
            new_cap = old_cap << 1;
            if new_cap < MAXIMUM_CAPACITY && old_cap >= DEFAULT_INITIAL_CAPACITY {
                new_thr = old_thr << 1;
            }
        } else if old_thr > 0 {
            new_cap = old_thr as usize;
        } else {
            new_cap = DEFAULT_INITIAL_CAPACITY;
            new_thr = (DEFAULT_LOAD_FACTOR * DEFAULT_INITIAL_CAPACITY as f32) as i32;
        }
        if new_thr == 0 {
            let ft = new_cap as f32 * self.load_factor;
            new_thr = if new_cap < MAXIMUM_CAPACITY && ft < MAXIMUM_CAPACITY as f32 {
                ft as i32
            } else {
                i32::MAX
            };
        }
        self.threshold = new_thr;
        let old_tab = std::mem::replace(&mut self.table, vec![NIL; new_cap]);
        for (j, &head) in old_tab.iter().enumerate() {
            if head == NIL {
                continue;
            }
            let e = head;
            if self.node(e).next == NIL {
                let index = self.bin_index(self.node(e).hash);
                self.table[index] = e;
            } else if self.node(e).tree {
                self.split(e, j, old_cap);
            } else {
                // Preserve order.
                let (mut lo_head, mut lo_tail, mut hi_head, mut hi_tail) = (NIL, NIL, NIL, NIL);
                let mut e = e;
                while e != NIL {
                    let next = self.node(e).next;
                    if self.node(e).hash as u32 as usize & old_cap == 0 {
                        if lo_tail == NIL {
                            lo_head = e;
                        } else {
                            self.node_mut(lo_tail).next = e;
                        }
                        lo_tail = e;
                    } else {
                        if hi_tail == NIL {
                            hi_head = e;
                        } else {
                            self.node_mut(hi_tail).next = e;
                        }
                        hi_tail = e;
                    }
                    e = next;
                }
                if lo_tail != NIL {
                    self.node_mut(lo_tail).next = NIL;
                    self.table[j] = lo_head;
                }
                if hi_tail != NIL {
                    self.node_mut(hi_tail).next = NIL;
                    self.table[j + old_cap] = hi_head;
                }
            }
        }
    }

    /// `HashMap.treeifyBin`: turns the bin of `hash` into a tree, or resizes a small table.
    fn treeify_bin(&mut self, hash: i32)
    where
        K: JavaKey,
    {
        let n = self.table.len();
        if n < MIN_TREEIFY_CAPACITY {
            self.resize();
            return;
        }
        let index = (n - 1) & hash as u32 as usize;
        let mut e = self.table[index];
        if e == NIL {
            return;
        }
        let (mut hd, mut tl) = (NIL, NIL);
        while e != NIL {
            let next = self.node(e).next;
            // `replacementTreeNode(e, null)`.
            let node = self.node_mut(e);
            node.tree = true;
            node.next = NIL;
            node.parent = NIL;
            node.left = NIL;
            node.right = NIL;
            node.prev = NIL;
            node.red = false;
            if tl == NIL {
                hd = e;
            } else {
                self.node_mut(e).prev = tl;
                self.node_mut(tl).next = e;
            }
            tl = e;
            e = next;
        }
        self.table[index] = hd;
        self.treeify(hd);
    }

    /// `TreeNode.root()`.
    fn root(&self, mut r: u32) -> u32 {
        loop {
            let p = self.node(r).parent;
            if p == NIL {
                return r;
            }
            r = p;
        }
    }

    /// `TreeNode.moveRootToFront`.
    fn move_root_to_front(&mut self, root: u32) {
        let n = self.table.len();
        if root == NIL || n == 0 {
            return;
        }
        let index = (n - 1) & self.node(root).hash as u32 as usize;
        let first = self.table[index];
        if root != first {
            self.table[index] = root;
            let rp = self.node(root).prev;
            let rn = self.node(root).next;
            if rn != NIL {
                self.node_mut(rn).prev = rp;
            }
            if rp != NIL {
                self.node_mut(rp).next = rn;
            }
            if first != NIL {
                self.node_mut(first).prev = root;
            }
            let root_node = self.node_mut(root);
            root_node.next = first;
            root_node.prev = NIL;
        }
    }

    /// `TreeNode.find` from node `p`.
    fn find<Q>(&self, mut p: u32, h: i32, k: &Q) -> u32
    where
        Q: Equivalent<K> + ?Sized,
    {
        loop {
            let node = self.node(p);
            let (pl, pr) = (node.left, node.right);
            if node.hash > h {
                p = pl;
            } else if node.hash < h {
                p = pr;
            } else if k.equivalent(&node.key) {
                return p;
            } else if pl == NIL {
                p = pr;
            } else if pr == NIL {
                p = pl;
            } else if let Some(dir) = k.java_compare(&node.key).filter(|&d| d != Ordering::Equal) {
                p = if dir == Ordering::Less { pl } else { pr };
            } else {
                let q = self.find(pr, h, k);
                if q != NIL {
                    return q;
                }
                p = pl;
            }
            if p == NIL {
                return NIL;
            }
        }
    }

    /// `TreeNode.getTreeNode` on the bin whose first node is `first`.
    fn get_tree_node<Q>(&self, first: u32, h: i32, k: &Q) -> u32
    where
        Q: Equivalent<K> + ?Sized,
    {
        let start = if self.node(first).parent == NIL {
            first
        } else {
            self.root(first)
        };
        self.find(start, h, k)
    }

    /// The direction of `x` relative to `p` when inserting into a tree: Java's hash order,
    /// then `compareTo`, then `tieBreakOrder`.
    fn tree_dir(&self, x_hash: i32, x_key: &K, p: u32) -> i32
    where
        K: JavaKey,
    {
        let node = self.node(p);
        match node.hash.cmp(&x_hash) {
            Ordering::Greater => -1,
            Ordering::Less => 1,
            // Equal hashes: `compareTo`, else `tieBreakOrder` (class names, then identity hash
            // codes), which places the new key first.
            Ordering::Equal => match x_key.java_compare(&node.key) {
                Some(Ordering::Greater) => 1,
                _ => -1,
            },
        }
    }

    /// `TreeNode.treeify` of the list starting at `head`.
    fn treeify(&mut self, head: u32)
    where
        K: JavaKey,
    {
        let mut root = NIL;
        let mut x = head;
        while x != NIL {
            let next = self.node(x).next;
            {
                let node = self.node_mut(x);
                node.left = NIL;
                node.right = NIL;
            }
            if root == NIL {
                let node = self.node_mut(x);
                node.parent = NIL;
                node.red = false;
                root = x;
            } else {
                let h = self.node(x).hash;
                let mut p = root;
                loop {
                    let dir = self.tree_dir(h, &self.node(x).key, p);
                    let xp = p;
                    p = if dir <= 0 {
                        self.node(p).left
                    } else {
                        self.node(p).right
                    };
                    if p == NIL {
                        self.node_mut(x).parent = xp;
                        if dir <= 0 {
                            self.node_mut(xp).left = x;
                        } else {
                            self.node_mut(xp).right = x;
                        }
                        root = self.balance_insertion(root, x);
                        break;
                    }
                }
            }
            x = next;
        }
        self.move_root_to_front(root);
    }

    /// `TreeNode.untreeify`: the list starting at `head` becomes plain nodes again.
    fn untreeify(&mut self, head: u32) -> u32 {
        let mut q = head;
        while q != NIL {
            let node = self.node_mut(q);
            node.tree = false;
            q = node.next;
        }
        head
    }

    /// `TreeNode.putTreeVal`: inserts the entry and returns its new node, or returns the node
    /// that already holds the key together with the value that was not inserted.
    fn put_tree_val(&mut self, first: u32, h: i32, k: K, v: V) -> Result<u32, (u32, V)>
    where
        K: JavaKey,
    {
        let root = if self.node(first).parent == NIL {
            first
        } else {
            self.root(first)
        };
        let mut searched = false;
        let mut p = root;
        loop {
            let node = self.node(p);
            let dir = if node.hash > h {
                -1
            } else if node.hash < h {
                1
            } else if node.key == k {
                return Err((p, v));
            } else {
                match k.java_compare(&node.key) {
                    Some(Ordering::Less) => -1,
                    Some(Ordering::Greater) => 1,
                    _ => {
                        if !searched {
                            searched = true;
                            let (left, right) = (node.left, node.right);
                            if left != NIL {
                                let q = self.find(left, h, &k);
                                if q != NIL {
                                    return Err((q, v));
                                }
                            }
                            if right != NIL {
                                let q = self.find(right, h, &k);
                                if q != NIL {
                                    return Err((q, v));
                                }
                            }
                        }
                        // `tieBreakOrder`.
                        -1
                    }
                }
            };
            let xp = p;
            p = if dir <= 0 {
                self.node(xp).left
            } else {
                self.node(xp).right
            };
            if p == NIL {
                let xpn = self.node(xp).next;
                let x = self.alloc(h, k, v, xpn);
                {
                    let node = self.node_mut(x);
                    node.tree = true;
                    node.parent = xp;
                    node.prev = xp;
                }
                if dir <= 0 {
                    self.node_mut(xp).left = x;
                } else {
                    self.node_mut(xp).right = x;
                }
                self.node_mut(xp).next = x;
                if xpn != NIL {
                    self.node_mut(xpn).prev = x;
                }
                let root = self.balance_insertion(root, x);
                self.move_root_to_front(root);
                return Ok(x);
            }
        }
    }

    /// `TreeNode.removeTreeNode` for node `p` (already found in its bin).
    #[allow(
        clippy::too_many_lines,
        reason = "a line-by-line transcription of the JDK method"
    )]
    fn remove_tree_node(&mut self, p: u32, movable: bool) {
        let n = self.table.len();
        if n == 0 {
            return;
        }
        let index = (n - 1) & self.node(p).hash as u32 as usize;
        let mut first = self.table[index];
        let mut root = first;
        let succ = self.node(p).next;
        let pred = self.node(p).prev;
        if pred == NIL {
            first = succ;
            self.table[index] = first;
        } else {
            self.node_mut(pred).next = succ;
        }
        if succ != NIL {
            self.node_mut(succ).prev = pred;
        }
        if first == NIL {
            return;
        }
        if self.node(root).parent != NIL {
            root = self.root(root);
        }
        let too_small = movable && {
            let root_node = self.node(root);
            let rl = root_node.left;
            root_node.right == NIL || rl == NIL || self.node(rl).left == NIL
        };
        if too_small {
            self.table[index] = self.untreeify(first);
            return;
        }
        let (pl, pr) = (self.node(p).left, self.node(p).right);
        let replacement = if pl != NIL && pr != NIL {
            let mut s = pr;
            loop {
                let sl = self.node(s).left;
                if sl == NIL {
                    break;
                }
                s = sl;
            }
            // Swap colors.
            let c = self.node(s).red;
            self.node_mut(s).red = self.node(p).red;
            self.node_mut(p).red = c;
            let sr = self.node(s).right;
            let pp = self.node(p).parent;
            if s == pr {
                // p was s's direct parent.
                self.node_mut(p).parent = s;
                self.node_mut(s).right = p;
            } else {
                let sp = self.node(s).parent;
                self.node_mut(p).parent = sp;
                if sp != NIL {
                    if s == self.node(sp).left {
                        self.node_mut(sp).left = p;
                    } else {
                        self.node_mut(sp).right = p;
                    }
                }
                self.node_mut(s).right = pr;
                if pr != NIL {
                    self.node_mut(pr).parent = s;
                }
            }
            self.node_mut(p).left = NIL;
            self.node_mut(p).right = sr;
            if sr != NIL {
                self.node_mut(sr).parent = p;
            }
            self.node_mut(s).left = pl;
            if pl != NIL {
                self.node_mut(pl).parent = s;
            }
            self.node_mut(s).parent = pp;
            if pp == NIL {
                root = s;
            } else if p == self.node(pp).left {
                self.node_mut(pp).left = s;
            } else {
                self.node_mut(pp).right = s;
            }
            if sr == NIL { p } else { sr }
        } else if pl != NIL {
            pl
        } else if pr != NIL {
            pr
        } else {
            p
        };
        if replacement != p {
            let pp = self.node(p).parent;
            self.node_mut(replacement).parent = pp;
            if pp == NIL {
                root = replacement;
                self.node_mut(root).red = false;
            } else if p == self.node(pp).left {
                self.node_mut(pp).left = replacement;
            } else {
                self.node_mut(pp).right = replacement;
            }
            let node = self.node_mut(p);
            node.left = NIL;
            node.right = NIL;
            node.parent = NIL;
        }
        let r = if self.node(p).red {
            root
        } else {
            self.balance_deletion(root, replacement)
        };
        if replacement == p {
            // Detach.
            let pp = self.node(p).parent;
            self.node_mut(p).parent = NIL;
            if pp != NIL {
                if p == self.node(pp).left {
                    self.node_mut(pp).left = NIL;
                } else if p == self.node(pp).right {
                    self.node_mut(pp).right = NIL;
                }
            }
        }
        if movable {
            self.move_root_to_front(r);
        }
    }

    /// `TreeNode.split`: splits the tree bin at `index` during a resize.
    fn split(&mut self, b: u32, index: usize, bit: usize)
    where
        K: JavaKey,
    {
        let (mut lo_head, mut lo_tail, mut hi_head, mut hi_tail) = (NIL, NIL, NIL, NIL);
        let (mut lc, mut hc) = (0usize, 0usize);
        let mut e = b;
        while e != NIL {
            let next = self.node(e).next;
            self.node_mut(e).next = NIL;
            if self.node(e).hash as u32 as usize & bit == 0 {
                self.node_mut(e).prev = lo_tail;
                if lo_tail == NIL {
                    lo_head = e;
                } else {
                    self.node_mut(lo_tail).next = e;
                }
                lo_tail = e;
                lc += 1;
            } else {
                self.node_mut(e).prev = hi_tail;
                if hi_tail == NIL {
                    hi_head = e;
                } else {
                    self.node_mut(hi_tail).next = e;
                }
                hi_tail = e;
                hc += 1;
            }
            e = next;
        }
        if lo_head != NIL {
            if lc <= UNTREEIFY_THRESHOLD {
                self.table[index] = self.untreeify(lo_head);
            } else {
                self.table[index] = lo_head;
                if hi_head != NIL {
                    self.treeify(lo_head);
                }
            }
        }
        if hi_head != NIL {
            if hc <= UNTREEIFY_THRESHOLD {
                self.table[index + bit] = self.untreeify(hi_head);
            } else {
                self.table[index + bit] = hi_head;
                if lo_head != NIL {
                    self.treeify(hi_head);
                }
            }
        }
    }

    /// `TreeNode.rotateLeft`.
    fn rotate_left(&mut self, mut root: u32, p: u32) -> u32 {
        if p == NIL {
            return root;
        }
        let r = self.node(p).right;
        if r == NIL {
            return root;
        }
        let rl = self.node(r).left;
        self.node_mut(p).right = rl;
        if rl != NIL {
            self.node_mut(rl).parent = p;
        }
        let pp = self.node(p).parent;
        self.node_mut(r).parent = pp;
        if pp == NIL {
            root = r;
            self.node_mut(r).red = false;
        } else if self.node(pp).left == p {
            self.node_mut(pp).left = r;
        } else {
            self.node_mut(pp).right = r;
        }
        self.node_mut(r).left = p;
        self.node_mut(p).parent = r;
        root
    }

    /// `TreeNode.rotateRight`.
    fn rotate_right(&mut self, mut root: u32, p: u32) -> u32 {
        if p == NIL {
            return root;
        }
        let l = self.node(p).left;
        if l == NIL {
            return root;
        }
        let lr = self.node(l).right;
        self.node_mut(p).left = lr;
        if lr != NIL {
            self.node_mut(lr).parent = p;
        }
        let pp = self.node(p).parent;
        self.node_mut(l).parent = pp;
        if pp == NIL {
            root = l;
            self.node_mut(l).red = false;
        } else if self.node(pp).right == p {
            self.node_mut(pp).right = l;
        } else {
            self.node_mut(pp).left = l;
        }
        self.node_mut(l).right = p;
        self.node_mut(p).parent = l;
        root
    }

    fn parent_of(&self, x: u32) -> u32 {
        if x == NIL { NIL } else { self.node(x).parent }
    }

    fn is_red(&self, x: u32) -> bool {
        x != NIL && self.node(x).red
    }

    /// `TreeNode.balanceInsertion`.
    fn balance_insertion(&mut self, mut root: u32, mut x: u32) -> u32 {
        self.node_mut(x).red = true;
        loop {
            let mut xp = self.node(x).parent;
            if xp == NIL {
                self.node_mut(x).red = false;
                return x;
            }
            if !self.node(xp).red {
                return root;
            }
            let mut xpp = self.node(xp).parent;
            if xpp == NIL {
                return root;
            }
            let xppl = self.node(xpp).left;
            if xp == xppl {
                let xppr = self.node(xpp).right;
                if self.is_red(xppr) {
                    self.node_mut(xppr).red = false;
                    self.node_mut(xp).red = false;
                    self.node_mut(xpp).red = true;
                    x = xpp;
                } else {
                    if x == self.node(xp).right {
                        x = xp;
                        root = self.rotate_left(root, x);
                        xp = self.node(x).parent;
                        xpp = self.parent_of(xp);
                    }
                    if xp != NIL {
                        self.node_mut(xp).red = false;
                        if xpp != NIL {
                            self.node_mut(xpp).red = true;
                            root = self.rotate_right(root, xpp);
                        }
                    }
                }
            } else if self.is_red(xppl) {
                self.node_mut(xppl).red = false;
                self.node_mut(xp).red = false;
                self.node_mut(xpp).red = true;
                x = xpp;
            } else {
                if x == self.node(xp).left {
                    x = xp;
                    root = self.rotate_right(root, x);
                    xp = self.node(x).parent;
                    xpp = self.parent_of(xp);
                }
                if xp != NIL {
                    self.node_mut(xp).red = false;
                    if xpp != NIL {
                        self.node_mut(xpp).red = true;
                        root = self.rotate_left(root, xpp);
                    }
                }
            }
        }
    }

    /// `TreeNode.balanceDeletion`.
    fn balance_deletion(&mut self, mut root: u32, mut x: u32) -> u32 {
        loop {
            if x == NIL || x == root {
                return root;
            }
            let mut xp = self.node(x).parent;
            if xp == NIL {
                self.node_mut(x).red = false;
                return x;
            }
            if self.node(x).red {
                self.node_mut(x).red = false;
                return root;
            }
            let xpl = self.node(xp).left;
            if xpl == x {
                let mut xpr = self.node(xp).right;
                if self.is_red(xpr) {
                    self.node_mut(xpr).red = false;
                    self.node_mut(xp).red = true;
                    root = self.rotate_left(root, xp);
                    xp = self.node(x).parent;
                    xpr = if xp == NIL { NIL } else { self.node(xp).right };
                }
                if xpr == NIL {
                    x = xp;
                } else {
                    let sl = self.node(xpr).left;
                    let mut sr = self.node(xpr).right;
                    if !self.is_red(sr) && !self.is_red(sl) {
                        self.node_mut(xpr).red = true;
                        x = xp;
                    } else {
                        if !self.is_red(sr) {
                            if sl != NIL {
                                self.node_mut(sl).red = false;
                            }
                            self.node_mut(xpr).red = true;
                            root = self.rotate_right(root, xpr);
                            xp = self.node(x).parent;
                            xpr = if xp == NIL { NIL } else { self.node(xp).right };
                        }
                        if xpr != NIL {
                            let red = xp != NIL && self.node(xp).red;
                            self.node_mut(xpr).red = red;
                            sr = self.node(xpr).right;
                            if sr != NIL {
                                self.node_mut(sr).red = false;
                            }
                        }
                        if xp != NIL {
                            self.node_mut(xp).red = false;
                            root = self.rotate_left(root, xp);
                        }
                        x = root;
                    }
                }
            } else {
                // Symmetric.
                let mut xpl = xpl;
                if self.is_red(xpl) {
                    self.node_mut(xpl).red = false;
                    self.node_mut(xp).red = true;
                    root = self.rotate_right(root, xp);
                    xp = self.node(x).parent;
                    xpl = if xp == NIL { NIL } else { self.node(xp).left };
                }
                if xpl == NIL {
                    x = xp;
                } else {
                    let mut sl = self.node(xpl).left;
                    let sr = self.node(xpl).right;
                    if !self.is_red(sl) && !self.is_red(sr) {
                        self.node_mut(xpl).red = true;
                        x = xp;
                    } else {
                        if !self.is_red(sl) {
                            if sr != NIL {
                                self.node_mut(sr).red = false;
                            }
                            self.node_mut(xpl).red = true;
                            root = self.rotate_left(root, xpl);
                            xp = self.node(x).parent;
                            xpl = if xp == NIL { NIL } else { self.node(xp).left };
                        }
                        if xpl != NIL {
                            let red = xp != NIL && self.node(xp).red;
                            self.node_mut(xpl).red = red;
                            sl = self.node(xpl).left;
                            if sl != NIL {
                                self.node_mut(sl).red = false;
                            }
                        }
                        if xp != NIL {
                            self.node_mut(xp).red = false;
                            root = self.rotate_right(root, xp);
                        }
                        x = root;
                    }
                }
            }
        }
    }
}

/// Where a key belongs in its bin: an existing node, or a new node to link in.
enum Slot {
    /// The key is already in the node.
    Found(u32),
    /// The bin is empty.
    EmptyBin,
    /// A list bin: append after the last node; `bin_count` is Java's loop counter.
    Append { last: u32, bin_count: usize },
    /// A tree bin whose first node is given.
    Tree(u32),
}

impl<K: JavaKey, V> JavaHashMap<K, V> {
    /// `new HashMap<>(m)`: a new map holding `m`'s entries, sized for them and filled in
    /// `m`'s iteration order.
    #[must_use]
    pub fn from_map(m: &Self) -> Self
    where
        K: Clone,
        V: Clone,
    {
        let mut map = Self::new();
        map.put_all(m);
        map
    }

    /// `putAll(m)`.
    pub fn put_all(&mut self, m: &Self)
    where
        K: Clone,
        V: Clone,
    {
        self.put_map_entries(m.iter().map(|(k, v)| (k.clone(), v.clone())), m.len());
    }

    /// `HashMap.putMapEntries`: puts the entries of a map that has `s` entries, in order.
    pub fn put_map_entries(&mut self, entries: impl IntoIterator<Item = (K, V)>, s: usize) {
        if s == 0 {
            return;
        }
        if self.table.is_empty() {
            // Pre-size.
            let dt = (s as f64 / f64::from(self.load_factor)).ceil();
            let t = if dt < MAXIMUM_CAPACITY as f64 {
                dt as i32
            } else {
                MAXIMUM_CAPACITY as i32
            };
            if t > self.threshold {
                self.threshold = table_size_for(t);
            }
        } else {
            while s > self.threshold as usize && self.table.len() < MAXIMUM_CAPACITY {
                self.resize();
            }
        }
        for (key, value) in entries {
            self.insert(key, value);
        }
    }

    /// The node holding `key`, or `NIL` (`HashMap.getNode`).
    fn get_node<Q>(&self, key: &Q) -> u32
    where
        Q: Equivalent<K> + ?Sized,
    {
        let n = self.table.len();
        if n == 0 {
            return NIL;
        }
        let hash = spread(key.java_hash_code());
        let first = self.table[(n - 1) & hash as u32 as usize];
        if first == NIL {
            return NIL;
        }
        let node = self.node(first);
        if node.hash == hash && key.equivalent(&node.key) {
            return first;
        }
        let mut e = node.next;
        if e == NIL {
            return NIL;
        }
        if node.tree {
            return self.get_tree_node(first, hash, key);
        }
        while e != NIL {
            let node = self.node(e);
            if node.hash == hash && key.equivalent(&node.key) {
                return e;
            }
            e = node.next;
        }
        NIL
    }

    /// The value for `key` (`get`).
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        Q: Equivalent<K> + ?Sized,
    {
        let e = self.get_node(key);
        (e != NIL).then(|| &self.node(e).value)
    }

    /// The value for `key`, mutably.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        Q: Equivalent<K> + ?Sized,
    {
        let e = self.get_node(key);
        if e == NIL {
            None
        } else {
            Some(&mut self.node_mut(e).value)
        }
    }

    /// The stored key and its value for `key`.
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        Q: Equivalent<K> + ?Sized,
    {
        let e = self.get_node(key);
        (e != NIL).then(|| {
            let node = self.node(e);
            (&node.key, &node.value)
        })
    }

    /// Whether `key` is present (`containsKey`).
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        Q: Equivalent<K> + ?Sized,
    {
        self.get_node(key) != NIL
    }

    /// Finds where `key` (with spread hash `hash`) is or would be linked in its bin, the way
    /// `putVal` walks a list bin.
    fn locate_for_put(&self, hash: i32, key: &K) -> (usize, Slot) {
        let i = self.bin_index(hash);
        let p = self.table[i];
        if p == NIL {
            return (i, Slot::EmptyBin);
        }
        let first = self.node(p);
        if first.hash == hash && first.key == *key {
            return (i, Slot::Found(p));
        }
        if first.tree {
            return (i, Slot::Tree(p));
        }
        let mut p = p;
        let mut bin_count = 0;
        loop {
            let e = self.node(p).next;
            if e == NIL {
                return (i, Slot::Append { last: p, bin_count });
            }
            let node = self.node(e);
            if node.hash == hash && node.key == *key {
                return (i, Slot::Found(e));
            }
            p = e;
            bin_count += 1;
        }
    }

    /// `put(key, value)`: returns the previous value.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let hash = spread(key.java_hash_code());
        self.put_val(hash, key, value, false).err()
    }

    /// `putIfAbsent(key, value)`: inserts only if `key` is absent, and returns whether it did.
    pub fn put_if_absent(&mut self, key: K, value: V) -> bool {
        let hash = spread(key.java_hash_code());
        self.put_val(hash, key, value, true).is_ok()
    }

    /// `HashMap.putVal`. Returns `Ok` when a new entry was added. Otherwise returns the
    /// replaced value, or with `only_if_absent` the value that was not stored.
    fn put_val(&mut self, hash: i32, key: K, value: V, only_if_absent: bool) -> Result<(), V> {
        if self.table.is_empty() {
            self.resize();
        }
        let (i, slot) = self.locate_for_put(hash, &key);
        let existing = match slot {
            Slot::Found(e) => Some((e, value)),
            Slot::EmptyBin => {
                self.table[i] = self.alloc(hash, key, value, NIL);
                None
            }
            Slot::Append { last, bin_count } => {
                let new = self.alloc(hash, key, value, NIL);
                self.node_mut(last).next = new;
                if bin_count >= TREEIFY_THRESHOLD - 1 {
                    self.treeify_bin(hash);
                }
                None
            }
            Slot::Tree(first) => self.put_tree_val(first, hash, key, value).err(),
        };
        if let Some((e, value)) = existing {
            if only_if_absent {
                return Err(value);
            }
            return Err(std::mem::replace(&mut self.node_mut(e).value, value));
        }
        self.size += 1;
        if self.size > self.threshold as usize {
            self.resize();
        }
        Ok(())
    }

    /// `remove(key)`: returns the removed value.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: Equivalent<K> + ?Sized,
    {
        self.remove_entry(key).map(|(_, value)| value)
    }

    /// Removes `key` and returns the stored key and value.
    pub fn remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: Equivalent<K> + ?Sized,
    {
        let node = self.get_node(key);
        if node == NIL {
            return None;
        }
        self.unlink(node, true);
        let node = self.release(node);
        Some((node.key, node.value))
    }

    /// The bin-unlinking half of `HashMap.removeNode`, for a node known to be in the map.
    fn unlink(&mut self, node: u32, movable: bool) {
        let index = self.bin_index(self.node(node).hash);
        let first = self.table[index];
        if self.node(node).tree {
            self.remove_tree_node(node, movable);
        } else if node == first {
            self.table[index] = self.node(node).next;
        } else {
            let mut p = first;
            while self.node(p).next != node {
                p = self.node(p).next;
            }
            self.node_mut(p).next = self.node(node).next;
        }
        self.size -= 1;
    }

    /// `removeIf` on a view, or removal through an iterator: keeps the entries for which
    /// `keep` returns true, visiting them in iteration order. Java's iterator removal never
    /// moves the remaining nodes (`movable` is false).
    pub fn retain(&mut self, mut keep: impl FnMut(&K, &mut V) -> bool) {
        for e in self.node_order() {
            let node = self.node_mut(e);
            if !keep(&node.key, &mut node.value) {
                self.unlink(e, false);
                self.release(e);
            }
        }
    }

    /// `computeIfAbsent(key, f)`: returns the value for `key`, computing and inserting it
    /// when `key` is absent. `f` returning `None` (Java's `null`) inserts nothing.
    pub fn compute_if_absent(&mut self, key: K, f: impl FnOnce(&K) -> Option<V>) -> Option<&mut V> {
        let hash = spread(key.java_hash_code());
        if self.size > self.threshold as usize || self.table.is_empty() {
            self.resize();
        }
        let (i, first, old, bin_count) = self.find_for_compute(hash, &key);
        if old != NIL {
            return Some(&mut self.node_mut(old).value);
        }
        let value = f(&key)?;
        let node = self.insert_for_compute(i, first, hash, key, value, bin_count);
        Some(&mut self.node_mut(node).value)
    }

    /// `compute(key, f)`: replaces, inserts or (when `f` returns `None`) removes the value for
    /// `key`, and returns the new value.
    pub fn compute(
        &mut self,
        key: K,
        f: impl FnOnce(&K, Option<&V>) -> Option<V>,
    ) -> Option<&mut V> {
        let hash = spread(key.java_hash_code());
        if self.size > self.threshold as usize || self.table.is_empty() {
            self.resize();
        }
        let (i, first, old, bin_count) = self.find_for_compute(hash, &key);
        let old_value = (old != NIL).then(|| &self.node(old).value);
        let value = f(&key, old_value);
        if old != NIL {
            if let Some(value) = value {
                self.node_mut(old).value = value;
                return Some(&mut self.node_mut(old).value);
            }
            self.unlink(old, true);
            self.release(old);
            return None;
        }
        let node = self.insert_for_compute(i, first, hash, key, value?, bin_count);
        Some(&mut self.node_mut(node).value)
    }

    /// `merge(key, value, f)`: inserts `value`, or replaces the existing value `v` with
    /// `f(v, value)`, removing the entry when that is `None`. Returns the new value.
    pub fn merge(
        &mut self,
        key: K,
        value: V,
        f: impl FnOnce(&V, V) -> Option<V>,
    ) -> Option<&mut V> {
        let hash = spread(key.java_hash_code());
        if self.size > self.threshold as usize || self.table.is_empty() {
            self.resize();
        }
        let (i, first, old, bin_count) = self.find_for_compute(hash, &key);
        if old != NIL {
            if let Some(merged) = f(&self.node(old).value, value) {
                self.node_mut(old).value = merged;
                return Some(&mut self.node_mut(old).value);
            }
            self.unlink(old, true);
            self.release(old);
            return None;
        }
        let node = self.insert_for_compute(i, first, hash, key, value, bin_count);
        Some(&mut self.node_mut(node).value)
    }

    /// The bin search shared by `computeIfAbsent`, `compute` and `merge`: the bin index, its
    /// first node, the node holding `key` (or `NIL`) and the number of list nodes passed.
    fn find_for_compute(&self, hash: i32, key: &K) -> (usize, u32, u32, usize) {
        let i = self.bin_index(hash);
        let first = self.table[i];
        let mut bin_count = 0;
        let mut old = NIL;
        if first != NIL {
            if self.node(first).tree {
                old = self.get_tree_node(first, hash, key);
            } else {
                let mut e = first;
                while e != NIL {
                    let node = self.node(e);
                    if node.hash == hash && node.key == *key {
                        old = e;
                        break;
                    }
                    bin_count += 1;
                    e = node.next;
                }
            }
        }
        (i, first, old, bin_count)
    }

    /// The insertion shared by `computeIfAbsent`, `compute` and `merge`: into the tree, or
    /// at the FRONT of a list bin. Returns the new node.
    fn insert_for_compute(
        &mut self,
        i: usize,
        first: u32,
        hash: i32,
        key: K,
        value: V,
        bin_count: usize,
    ) -> u32 {
        let node = if first != NIL && self.node(first).tree {
            match self.put_tree_val(first, hash, key, value) {
                Ok(node) => node,
                Err(_) => unreachable!("the key was absent from the tree"),
            }
        } else {
            let node = self.alloc(hash, key, value, first);
            self.table[i] = node;
            if bin_count >= TREEIFY_THRESHOLD - 1 {
                self.treeify_bin(hash);
            }
            node
        };
        self.size += 1;
        node
    }
}

impl<K: JavaKey, V: PartialEq> PartialEq for JavaHashMap<K, V> {
    /// `AbstractMap.equals`: the same keys mapped to equal values, in any order.
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl<K: JavaKey, V: Eq> Eq for JavaHashMap<K, V> {}

impl<K: JavaKey, V> JavaHashMap<K, V> {
    /// `AbstractMap.hashCode`: the sum of `key.hashCode() ^ value.hashCode()` over the
    /// entries, with `value_hash` giving the values' Java hash codes.
    pub fn java_hash_code(&self, value_hash: impl Fn(&V) -> i32) -> i32 {
        self.iter().fold(0i32, |sum, (k, v)| {
            sum.wrapping_add(k.java_hash_code() ^ value_hash(v))
        })
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for JavaHashMap<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<'a, K, V> IntoIterator for &'a JavaHashMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// The entries of a [`JavaHashMap`] in Java's iteration order: bin by bin, and along each
/// bin's `next` links.
pub struct Iter<'a, K, V> {
    map: &'a JavaHashMap<K, V>,
    /// The next bin to look at.
    index: usize,
    /// The next node to return, or `NIL` to search the following bins.
    next: u32,
    remaining: usize,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        let table = &self.map.table;
        while self.next == NIL && self.index < table.len() {
            self.next = table[self.index];
            self.index += 1;
        }
        if self.next == NIL {
            return None;
        }
        let node = self.map.node(self.next);
        self.next = node.next;
        self.remaining -= 1;
        Some((&node.key, &node.value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(map: &JavaHashMap<JString, i32>) -> Vec<String> {
        map.keys().map(JString::to_string_lossy).collect()
    }

    fn map_of(keys: &[&str]) -> JavaHashMap<JString, i32> {
        let mut map = JavaHashMap::new();
        for (i, key) in keys.iter().enumerate() {
            map.insert(JString::from(*key), i32::try_from(i).unwrap());
        }
        map
    }

    /// `n`-character strings over {"Aa", "BB"}: all `2^n` share one hash code.
    fn colliding(n: u32) -> Vec<String> {
        let pair = |bits: u32, i: u32| if (bits >> i) & 1 == 0 { "Aa" } else { "BB" };
        (0..1u32 << n)
            .map(|bits| (0..n).map(|i| pair(bits, i)).collect())
            .collect()
    }

    #[test]
    fn iterates_in_bucket_order() {
        // Expected orders printed by Java 25's HashMap for the same insertions.
        let map = map_of(&["pos", "state", "nbt", "DataVersion", "id", "Count"]);
        assert_eq!(
            keys(&map),
            ["nbt", "pos", "state", "DataVersion", "id", "Count"]
        );
        assert_eq!(map.capacity(), 16);
    }

    #[test]
    fn insert_get_remove() {
        let mut map = map_of(&["a", "b", "c"]);
        assert_eq!(map.insert(JString::from("b"), 9), Some(1));
        assert_eq!(map.get("b"), Some(&9));
        assert_eq!(map.remove("a"), Some(0));
        assert!(!map.contains_key("a"));
        assert_eq!(map.len(), 2);
        assert!(!map.put_if_absent(JString::from("c"), 5));
        assert_eq!(map.get("c"), Some(&2));
    }

    #[test]
    fn clear_keeps_the_capacity() {
        let mut map = map_of(&["a"; 1]);
        for i in 0..20 {
            map.insert(JString::from(i.to_string()), i);
        }
        assert_eq!(map.capacity(), 32);
        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.capacity(), 32);
    }

    #[test]
    fn colliding_keys_become_a_tree_and_stay_findable() {
        let strings = colliding(5);
        let names: Vec<&str> = strings.iter().map(String::as_str).collect();
        let mut map = map_of(&names);
        assert_eq!(map.len(), 32);
        assert_eq!(map.capacity(), 64);
        // Java 25's order: the tree root first, then each node after its tree parent.
        let java = [
            "BBAaAaAaAa",
            "AaAaAaAaAa",
            "AaAaAaAaBB",
            "AaBBAaAaAa",
            "BBBBAaAaAa",
            "BBBBAaAaBB",
            "BBAaBBBBAa",
            "BBAaBBBBBB",
            "BBAaBBAaBB",
            "AaAaBBAaAa",
            "AaAaAaBBBB",
            "AaAaBBBBAa",
            "AaAaBBBBBB",
            "AaAaBBAaBB",
            "BBAaBBAaAa",
            "AaBBBBAaAa",
            "AaBBBBBBAa",
            "AaBBBBBBBB",
            "AaBBBBAaBB",
            "BBBBBBAaAa",
            "BBBBAaBBBB",
            "BBBBBBBBAa",
            "BBBBBBBBBB",
            "BBBBBBAaBB",
            "BBBBAaBBAa",
            "AaAaAaBBAa",
            "BBAaAaBBAa",
            "BBAaAaBBBB",
            "BBAaAaAaBB",
            "AaBBAaBBAa",
            "AaBBAaBBBB",
            "AaBBAaAaBB",
        ];
        assert_eq!(keys(&map), java);
        for (i, key) in names.iter().enumerate() {
            assert_eq!(map.get(*key), Some(&i32::try_from(i).unwrap()));
        }
        for key in &names[..28] {
            assert!(map.remove(*key).is_some());
        }
        assert_eq!(names.len() - 28, map.len());
        for key in &names[28..] {
            assert!(map.contains_key(*key));
        }
    }

    #[test]
    fn compute_inserts_at_the_front_of_a_bin() {
        // "Aa" and "BB" share a bin: put appends, computeIfAbsent prepends.
        let mut put = JavaHashMap::new();
        put.insert(JString::from("Aa"), 1);
        put.insert(JString::from("BB"), 2);
        assert_eq!(keys(&put), ["Aa", "BB"]);
        let mut computed = JavaHashMap::new();
        computed.insert(JString::from("Aa"), 1);
        computed.compute_if_absent(JString::from("BB"), |_| Some(2));
        assert_eq!(keys(&computed), ["BB", "Aa"]);
    }

    #[test]
    fn equality_ignores_order_and_copies_resize() {
        let a = map_of(&["x", "y", "z"]);
        let b = map_of(&["z", "y", "x"]);
        assert_ne!(a.get("x"), b.get("x"));
        assert_eq!(map_of(&["k"]), map_of(&["k"]));
        let copy = JavaHashMap::from_map(&a);
        assert_eq!(copy, a);
        assert_eq!(keys(&copy), keys(&a));
    }
}
