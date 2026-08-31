//! Handle registries: the runtime state behind an integer a program holds.
//!
//! Several native capabilities hand a program an `int` and then look it up
//! later — `Memory.alloc` returns an allocation handle, `GPU.createBuffer` a
//! buffer id, `Socket.listen` a listener id. Each was written out longhand
//! inside `Evaluator` as a `HashMap<i64, T>` beside a `next_id: i64`, so the
//! evaluator owned the storage, the id allocation and the lifetime rules for
//! every one of them.
//!
//! That is the coupling `MATURITY_AUDIT.md`'s P2 section is about: a single
//! struct that is simultaneously the language evaluator, a memory manager, a
//! GPU runtime and a socket manager. This module owns the part of those roles
//! that has nothing to do with evaluating Serez — which integer means which
//! object, and when it stops meaning anything.
//!
//! ## What the registry guarantees
//!
//! **Ids start at 1 and only ever increase.** Nothing is reused, so a handle
//! that has been removed stays invalid for the life of the registry rather
//! than silently coming to mean a later object. That property is the reason
//! `Memory.free` twice is a clean diagnostic instead of a second free of
//! someone else's allocation, and it is now stated in one place and tested
//! directly, rather than being an emergent consequence of `+= 1` at three
//! call sites.
//!
//! **A registry belongs to one evaluator.** A `Task` worker runs its own
//! evaluator with its own registries, so the parent's handles are not visible
//! to it — a worker reading handle `1` gets a diagnostic, not the parent's
//! bytes. Isolation comes from ownership, not from a check anybody has to
//! remember to write.
//!
//! ## What it deliberately does not do
//!
//! There is **no aggregate ceiling**. `spec/limits.md` records that as a known
//! gap: `Memory.alloc` bounds one allocation at 256 MiB and nothing bounds the
//! sum, so a loop allocating 4 MiB at a time will exhaust the host. Adding a
//! budget would refuse programs that run today, so it is not smuggled in with a
//! refactor. What changes here is that such a budget would now have **one place
//! to live** instead of three ad-hoc counters.
//!
//! ## What lives here
//!
//! `HandleRegistry<T>` for the one-map cases — the raw-memory heap and the GPU
//! buffers — and `SocketTable` for the case that is not one map: connections
//! and listeners are two maps drawing from **one** id space, because
//! `spec/socket.md` promises a listener id and a connection id are never equal.
//! `HandleAllocator` is the counter both rest on, so that promise is one tested
//! rule rather than several `+= 1`s that happen to agree.

use std::collections::HashMap;

/// Issues handles: starts at 1, only ever increases, never repeats.
///
/// It exists separately from `HandleRegistry` because the socket table needs
/// **two** maps drawing from **one** id space — `spec/socket.md` promises a
/// listener id and a connection id are never equal — and that promise should
/// rest on the same tested counter as everything else rather than on three
/// hand-written `+= 1`s agreeing.
#[derive(Debug, Default)]
pub struct HandleAllocator {
    next: i64,
}

impl HandleAllocator {
    pub fn new() -> Self {
        HandleAllocator { next: 1 }
    }

    /// The next unused handle. Zero is never issued.
    pub fn issue(&mut self) -> i64 {
        if self.next == 0 {
            self.next = 1;
        }
        let id = self.next;
        self.next += 1;
        id
    }
}

/// A map from an integer handle to the object it names, plus the counter that
/// issues those handles.
#[derive(Debug)]
pub struct HandleRegistry<T> {
    entries: HashMap<i64, T>,
    ids: HandleAllocator,
}

impl<T> Default for HandleRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> HandleRegistry<T> {
    /// An empty registry whose first handle will be `1`.
    ///
    /// Zero is never issued, so a caller that defaults an unset handle to `0`
    /// gets a lookup failure rather than the first allocation.
    pub fn new() -> Self {
        HandleRegistry {
            entries: HashMap::new(),
            ids: HandleAllocator::new(),
        }
    }

    /// Store `value` and return the handle that now names it.
    pub fn insert(&mut self, value: T) -> i64 {
        let id = self.ids.issue();
        self.entries.insert(id, value);
        id
    }

    pub fn get(&self, id: i64) -> Option<&T> {
        self.entries.get(&id)
    }

    pub fn get_mut(&mut self, id: i64) -> Option<&mut T> {
        self.entries.get_mut(&id)
    }

    /// Remove the entry, returning it. A second removal of the same handle
    /// returns `None` — the id is not reissued, so it cannot come to name
    /// anything else later.
    pub fn remove(&mut self, id: i64) -> Option<T> {
        self.entries.remove(&id)
    }

    pub fn contains(&self, id: i64) -> bool {
        self.entries.contains_key(&id)
    }

    /// How many live entries the registry holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Connections and listeners, in one id space.
///
/// `Socket.listen` and `Socket.connect` both hand back an `int`, and
/// `spec/socket.md` promises the two kinds never collide — a promise that used
/// to rest on two `HashMap`s in `Evaluator` sharing a `socket_next_id` field
/// that three separate call sites incremented by hand. The table owns it now.
#[derive(Debug, Default)]
pub struct SocketTable {
    connections: HashMap<i64, std::net::TcpStream>,
    listeners: HashMap<i64, std::net::TcpListener>,
    ids: HandleAllocator,
}

impl SocketTable {
    pub fn new() -> Self {
        SocketTable {
            connections: HashMap::new(),
            listeners: HashMap::new(),
            ids: HandleAllocator::new(),
        }
    }

    pub fn add_connection(&mut self, stream: std::net::TcpStream) -> i64 {
        let id = self.ids.issue();
        self.connections.insert(id, stream);
        id
    }

    pub fn add_listener(&mut self, listener: std::net::TcpListener) -> i64 {
        let id = self.ids.issue();
        self.listeners.insert(id, listener);
        id
    }

    pub fn connection_mut(&mut self, id: i64) -> Option<&mut std::net::TcpStream> {
        self.connections.get_mut(&id)
    }

    pub fn listener(&self, id: i64) -> Option<&std::net::TcpListener> {
        self.listeners.get(&id)
    }

    /// Drop whichever kind holds `id`, if either does.
    ///
    /// Returns whether anything was removed. `Socket.close` discards that:
    /// closing an id that was never issued is a documented no-op, unlike
    /// `send`/`recv`/`accept`, which are all errors on an unknown id.
    pub fn close(&mut self, id: i64) -> bool {
        self.connections.remove(&id).is_some() | self.listeners.remove(&id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listener_id_and_a_connection_id_are_never_equal() {
        // The promise spec/socket.md makes, resting on one counter rather than
        // on two maps that happen to be incremented in step.
        let mut table = SocketTable::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let listener_id = table.add_listener(listener);

        let port = table
            .listener(listener_id)
            .expect("the listener is addressable by its handle")
            .local_addr()
            .expect("local addr")
            .port();
        let stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect loopback");
        let connection_id = table.add_connection(stream);

        assert_ne!(listener_id, connection_id);
        assert!(table.connection_mut(listener_id).is_none());
        assert!(table.listener(connection_id).is_none());
    }

    #[test]
    fn closing_an_id_that_was_never_issued_removes_nothing() {
        let mut table = SocketTable::new();
        assert!(!table.close(1), "nothing to close in an empty table");
        assert!(!table.close(0), "and zero is not a handle either");
    }

    #[test]
    fn the_allocator_starts_at_one_and_never_repeats() {
        let mut ids = HandleAllocator::new();
        let issued: Vec<i64> = (0..5).map(|_| ids.issue()).collect();
        assert_eq!(issued, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn two_maps_can_share_one_id_space() {
        // What the socket table needs: a listener id and a connection id are
        // drawn from the same counter, so they can never collide.
        let mut ids = HandleAllocator::new();
        let mut listeners: HashMap<i64, &str> = HashMap::new();
        let mut connections: HashMap<i64, &str> = HashMap::new();

        let listener = ids.issue();
        listeners.insert(listener, "listener");
        let connection = ids.issue();
        connections.insert(connection, "connection");

        assert_ne!(listener, connection);
        assert!(!connections.contains_key(&listener));
        assert!(!listeners.contains_key(&connection));
    }

    #[test]
    fn handles_start_at_one_and_increase() {
        let mut registry: HandleRegistry<Vec<u8>> = HandleRegistry::new();
        assert_eq!(registry.insert(vec![0; 4]), 1);
        assert_eq!(registry.insert(vec![0; 4]), 2);
        assert_eq!(registry.insert(vec![0; 4]), 3);
    }

    #[test]
    fn zero_is_never_a_valid_handle() {
        // A caller that initialises a handle variable to 0 must get a lookup
        // failure, not the first allocation ever made.
        let mut registry: HandleRegistry<u8> = HandleRegistry::new();
        registry.insert(7);
        assert!(registry.get(0).is_none());
        assert!(!registry.contains(0));
    }

    #[test]
    fn a_removed_handle_is_never_reissued() {
        // The property behind `Memory.free` twice being a diagnostic rather
        // than a second free of someone else's allocation.
        let mut registry: HandleRegistry<&str> = HandleRegistry::new();
        let first = registry.insert("first");
        assert_eq!(registry.remove(first), Some("first"));
        assert_eq!(registry.remove(first), None);

        let second = registry.insert("second");
        assert_ne!(second, first, "a fresh handle must not repeat a freed one");
        assert!(registry.get(first).is_none(), "the old handle stays dead");
        assert_eq!(registry.get(second), Some(&"second"));
    }

    #[test]
    fn removal_does_not_lower_the_counter() {
        let mut registry: HandleRegistry<u8> = HandleRegistry::new();
        for _ in 0..5 {
            let id = registry.insert(0);
            registry.remove(id);
        }
        assert!(registry.is_empty(), "nothing is left");
        assert_eq!(
            registry.insert(0),
            6,
            "ids keep climbing even when the registry has been emptied"
        );
    }

    #[test]
    fn contents_are_readable_and_writable_through_the_handle() {
        let mut registry: HandleRegistry<Vec<u8>> = HandleRegistry::new();
        let id = registry.insert(vec![0u8; 4]);
        registry.get_mut(id).unwrap()[2] = 9;
        assert_eq!(registry.get(id).unwrap(), &vec![0, 0, 9, 0]);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn two_registries_do_not_share_a_handle_space() {
        // Each evaluator owns its own registries, which is what keeps a Task
        // worker from reaching the parent's allocations. Note this is the
        // opposite of what the socket registries need, and why they are not
        // migrated here: they deliberately share one id space.
        let mut left: HandleRegistry<&str> = HandleRegistry::new();
        let mut right: HandleRegistry<&str> = HandleRegistry::new();
        let a = left.insert("left");
        let b = right.insert("right");
        assert_eq!(a, b, "both hand out 1 first — the spaces are independent");
        assert_eq!(left.get(a), Some(&"left"));
        assert_eq!(right.get(b), Some(&"right"));
    }
}
