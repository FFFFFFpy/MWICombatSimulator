use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

use crate::{Result, SimError, SimTime};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EventHandle {
    id: u64,
    generation: u32,
}

impl EventHandle {
    pub const fn id(self) -> u64 {
        self.id
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EventKey {
    pub at: SimTime,
    pub sequence: u64,
}

#[derive(Debug)]
pub struct ScheduledEvent<T> {
    pub handle: EventHandle,
    pub key: EventKey,
    pub payload: T,
}

#[derive(Debug)]
struct HeapEntry<T> {
    event: ScheduledEvent<T>,
}

impl<T> PartialEq for HeapEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.event.key == other.event.key && self.event.handle == other.event.handle
    }
}

impl<T> Eq for HeapEntry<T> {}

impl<T> PartialOrd for HeapEntry<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for HeapEntry<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap. Reverse the stable key so the earliest event is popped first.
        other
            .event
            .key
            .cmp(&self.event.key)
            .then_with(|| other.event.handle.id.cmp(&self.event.handle.id))
    }
}

#[derive(Debug)]
pub struct StableEventQueue<T> {
    heap: BinaryHeap<HeapEntry<T>>,
    live_generations: HashMap<u64, u32>,
    next_id: u64,
    next_sequence: u64,
}

impl<T> Default for StableEventQueue<T> {
    fn default() -> Self {
        Self {
            heap: BinaryHeap::new(),
            live_generations: HashMap::new(),
            next_id: 0,
            next_sequence: 0,
        }
    }
}

impl<T> StableEventQueue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schedule(&mut self, at: SimTime, payload: T) -> EventHandle {
        let handle = EventHandle {
            id: self.next_id,
            generation: 0,
        };
        self.next_id = self.next_id.checked_add(1).expect("event id overflow");

        let key = EventKey {
            at,
            sequence: self.next_sequence,
        };
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("event sequence overflow");

        self.live_generations.insert(handle.id, handle.generation);
        self.heap.push(HeapEntry {
            event: ScheduledEvent {
                handle,
                key,
                payload,
            },
        });
        handle
    }

    pub fn cancel(&mut self, handle: EventHandle) -> Result<()> {
        match self.live_generations.get(&handle.id) {
            Some(generation) if *generation == handle.generation => {
                self.live_generations.remove(&handle.id);
                Ok(())
            }
            _ => Err(SimError::StaleEventHandle),
        }
    }

    pub fn pop_next(&mut self) -> Option<ScheduledEvent<T>> {
        while let Some(entry) = self.heap.pop() {
            let handle = entry.event.handle;
            let is_live = self
                .live_generations
                .get(&handle.id)
                .is_some_and(|generation| *generation == handle.generation);
            if !is_live {
                continue;
            }
            self.live_generations.remove(&handle.id);
            return Some(entry.event);
        }
        None
    }

    pub fn peek_key(&mut self) -> Option<EventKey> {
        loop {
            let entry = self.heap.peek()?;
            let handle = entry.event.handle;
            let is_live = self
                .live_generations
                .get(&handle.id)
                .is_some_and(|generation| *generation == handle.generation);
            if is_live {
                return Some(entry.event.key);
            }
            self.heap.pop();
        }
    }

    pub fn live_len(&self) -> usize {
        self.live_generations.len()
    }

    pub fn heap_len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.live_generations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn time(value: f64) -> SimTime {
        SimTime::new(value).unwrap()
    }

    #[test]
    fn pops_earlier_events_first_and_preserves_fifo_for_equal_times() {
        let mut queue = StableEventQueue::new();
        queue.schedule(time(10.0), "late");
        queue.schedule(time(5.0), "equal-first");
        queue.schedule(time(5.0), "equal-second");

        assert_eq!(queue.pop_next().unwrap().payload, "equal-first");
        assert_eq!(queue.pop_next().unwrap().payload, "equal-second");
        assert_eq!(queue.pop_next().unwrap().payload, "late");
        assert!(queue.pop_next().is_none());
    }

    #[test]
    fn cancellation_is_lazy_but_removed_from_the_live_count_immediately() {
        let mut queue = StableEventQueue::new();
        let cancelled = queue.schedule(time(1.0), "cancelled");
        queue.schedule(time(2.0), "live");

        queue.cancel(cancelled).unwrap();
        assert_eq!(queue.live_len(), 1);
        assert_eq!(queue.heap_len(), 2);
        assert_eq!(queue.peek_key().unwrap().at, time(2.0));
        assert_eq!(queue.pop_next().unwrap().payload, "live");
        assert!(queue.is_empty());
    }

    #[test]
    fn stale_or_consumed_handles_cannot_cancel_another_event() {
        let mut queue = StableEventQueue::new();
        let first = queue.schedule(time(1.0), "first");
        let second = queue.schedule(time(2.0), "second");

        assert_eq!(queue.pop_next().unwrap().handle, first);
        assert!(matches!(
            queue.cancel(first),
            Err(SimError::StaleEventHandle)
        ));
        queue.cancel(second).unwrap();
        assert!(queue.pop_next().is_none());
    }
}
