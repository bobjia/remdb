//! In-process pub/sub manager for the TCP server.
//!
//! `PubSubManager` maps topic names to sets of subscriber channels. Each
//! connection gets a `mpsc::Sender<Event>` that the push thread reads from.
//! The manager is shared across all connection threads via `Arc<PubSubManager>`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex};

/// A pub/sub event: `(topic, payload)`.
pub type Event = (String, Vec<u8>);

/// Shared topic-to-subscriber registry.
///
/// Each subscription is identified by a unique `u64` so the owning connection
/// can unsubscribe a specific topic without affecting other subscriptions on
/// the same connection.
pub struct PubSubManager {
    next_id: AtomicU64,
    topics: Mutex<HashMap<String, HashMap<u64, mpsc::Sender<Event>>>>,
}

impl PubSubManager {
    /// Create a new, empty manager.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            topics: Mutex::new(HashMap::new()),
        }
    }

    /// Subscribe `tx` to `topic`.  Returns a subscription id that can be
    /// passed to `unsubscribe()` or collected in a batch and passed to
    /// `unsubscribe_all()`.
    pub fn subscribe(&self, topic: &str, tx: mpsc::Sender<Event>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut topics = self
            .topics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        topics
            .entry(topic.to_string())
            .or_insert_with(HashMap::new)
            .insert(id, tx);
        id
    }

    /// Remove a single subscription from `topic`.
    pub fn unsubscribe(&self, topic: &str, sub_id: u64) {
        let mut topics = self
            .topics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(subs) = topics.get_mut(topic) {
            subs.remove(&sub_id);
            if subs.is_empty() {
                topics.remove(topic);
            }
        }
    }

    /// Remove all subscriptions listed in `sub_ids` (used during connection
    /// teardown so the connection doesn't need to enumerate topics one-by-one).
    pub fn unsubscribe_all(&self, sub_ids: &[(String, u64)]) {
        let mut topics = self
            .topics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for (topic, sub_id) in sub_ids {
            if let Some(subs) = topics.get_mut(topic.as_str()) {
                subs.remove(sub_id);
                if subs.is_empty() {
                    topics.remove(topic.as_str());
                }
            }
        }
    }

    /// Publish `payload` to all subscribers of `topic`.  Returns the number of
    /// subscribers that were successfully notified.
    pub fn publish(&self, topic: &str, payload: Vec<u8>) -> usize {
        let topics = self
            .topics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut count = 0;
        if let Some(subs) = topics.get(topic) {
            for tx in subs.values() {
                if tx.send((topic.to_string(), payload.clone())).is_ok() {
                    count += 1;
                }
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_and_publish_delivers_event() {
        let mgr = PubSubManager::new();
        let (tx, rx) = mpsc::channel();
        mgr.subscribe("sensors", tx);
        assert_eq!(mgr.publish("sensors", vec![1, 2, 3]), 1);
        let (topic, payload) = rx.recv().unwrap();
        assert_eq!(topic, "sensors");
        assert_eq!(payload, vec![1, 2, 3]);
    }

    #[test]
    fn unsubscribe_removes_subscriber() {
        let mgr = PubSubManager::new();
        let (tx, _rx) = mpsc::channel();
        let id = mgr.subscribe("sensors", tx);
        mgr.unsubscribe("sensors", id);
        assert_eq!(mgr.publish("sensors", vec![]), 0);
    }

    #[test]
    fn publish_to_empty_topic_returns_zero() {
        let mgr = PubSubManager::new();
        assert_eq!(mgr.publish("ghost", vec![]), 0);
    }

    #[test]
    fn multiple_subscribers_all_receive() {
        let mgr = PubSubManager::new();
        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        mgr.subscribe("t", tx1);
        mgr.subscribe("t", tx2);
        assert_eq!(mgr.publish("t", vec![42]), 2);
        assert_eq!(rx1.recv().unwrap().1, vec![42]);
        assert_eq!(rx2.recv().unwrap().1, vec![42]);
    }

    #[test]
    fn unsubscribe_all_cleans_batch() {
        let mgr = PubSubManager::new();
        let (tx1, _) = mpsc::channel();
        let (tx2, _) = mpsc::channel();
        let id1 = mgr.subscribe("a", tx1);
        let id2 = mgr.subscribe("b", tx2);
        mgr.unsubscribe_all(&[("a".into(), id1), ("b".into(), id2)]);
        assert_eq!(mgr.publish("a", vec![]), 0);
        assert_eq!(mgr.publish("b", vec![]), 0);
    }
}