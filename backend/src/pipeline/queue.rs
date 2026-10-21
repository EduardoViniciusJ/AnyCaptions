use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex, MutexGuard},
};

use super::Message;

struct State<T> {
    items: VecDeque<Message<T>>,
    closed: bool,
}

/// The lock only protects queue operations; processing never runs under it.
pub(super) struct Queue<T> {
    state: Mutex<State<T>>,
    available: Condvar,
    capacity: usize,
}

impl<T> Queue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new(State {
                items: VecDeque::with_capacity(capacity),
                closed: false,
            }),
            available: Condvar::new(),
            capacity,
        }
    }

    fn lock(&self) -> MutexGuard<'_, State<T>> {
        // No external code runs under this lock. Closing must still wake waiters
        // if an internal operation ever unwinds while holding it.
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Never waits for capacity. The next surviving item marks any lost work.
    pub fn push(&self, mut item: Message<T>) -> bool {
        let mut state = self.lock();
        if state.closed {
            return false;
        }
        let discarded = if state.items.len() == self.capacity {
            let oldest = state.items.pop_front();
            if let Some(next) = state.items.front_mut() {
                next.discontinuity = true;
            } else {
                item.discontinuity = true;
            }
            oldest
        } else {
            None
        };
        state.items.push_back(item);
        self.available.notify_one();
        drop(state);
        drop(discarded);
        true
    }

    pub fn recv(&self) -> Option<Message<T>> {
        let mut state = self.lock();
        while state.items.is_empty() && !state.closed {
            state = self
                .available
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        state.items.pop_front()
    }

    pub fn try_recv(&self) -> Option<Message<T>> {
        self.lock().items.pop_front()
    }

    #[cfg(test)]
    pub fn last_sequence(&self) -> Option<u64> {
        self.lock().items.back().map(|message| message.sequence)
    }

    pub fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        let discarded = std::mem::take(&mut state.items);
        self.available.notify_all();
        drop(state);
        drop(discarded);
    }

    /// Stops accepting new work while allowing the worker to drain queued items.
    pub fn finish(&self) {
        let mut state = self.lock();
        state.closed = true;
        self.available.notify_all();
    }
}
