//! Pure download-queue scheduler.
//!
//! Mirrors the event-as-data pattern of `crate::services::watch_state`: the
//! tracker holds the ordered item list and the concurrency limit, and every
//! method returns a `Vec<DownloadEvent>` describing what the runner should do
//! (start a transfer, cancel an in-flight one) — no GTK, no network, no disk.
//! The runner (`app::download_handlers`) performs the effects and feeds results
//! back in via `mark_completed` / `mark_failed`, which trigger rescheduling.
//!
//! Authoritative *state* persistence is the runner's job (via `DownloadsRepo`);
//! this tracker is the in-memory scheduling brain, rebuilt from the repo on
//! startup via [`DownloadQueue::from_items`].

use crate::models::download::DownloadState;

/// A queue entry the scheduler reasons about. The `queue_order`-equivalent is
/// the position in the tracker's `Vec`.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub struct QueueItem {
    pub media_item_id: String,
    pub part_key: String,
    pub state: DownloadState,
}

/// What the runner should do in response to a queue transition. The queue only
/// drives transfer start/cancel; state persistence is the runner's job.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum DownloadEvent {
    /// Begin (or resume) transferring this item. The item is now `Downloading`.
    StartTransfer {
        media_item_id: String,
        part_key: String,
    },
    /// Abort an in-flight transfer (the item was paused, cancelled, or deleted
    /// while `Downloading`). The `.part` file is kept on pause and removed by
    /// the runner on cancel/delete.
    CancelTransfer { media_item_id: String },
}

/// In-memory bounded-concurrency scheduler over an ordered set of downloads.
#[derive(Debug)]
#[allow(dead_code)]
pub struct DownloadQueue {
    items: Vec<QueueItem>,
    concurrency: usize,
}

#[allow(dead_code)]
impl DownloadQueue {
    pub fn new(concurrency: usize) -> Self {
        Self {
            items: Vec::new(),
            concurrency: concurrency.max(1),
        }
    }

    /// Rebuild the queue from persisted items (startup). Order is preserved as
    /// given (the caller passes them in `queue_order`).
    pub fn from_items(items: Vec<QueueItem>, concurrency: usize) -> Self {
        Self {
            items,
            concurrency: concurrency.max(1),
        }
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn state_of(&self, media_item_id: &str) -> Option<DownloadState> {
        self.items
            .iter()
            .find(|i| i.media_item_id == media_item_id)
            .map(|i| i.state)
    }

    /// Number of items currently transferring.
    pub fn active_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.state == DownloadState::Downloading)
            .count()
    }

    fn find_mut(&mut self, media_item_id: &str) -> Option<&mut QueueItem> {
        self.items
            .iter_mut()
            .find(|i| i.media_item_id == media_item_id)
    }

    /// Add an item to the queue, idempotently.
    ///
    /// An item already in `{Queued, Downloading, Paused, Completed}` is left
    /// untouched (no duplicate, no restart). A `Failed`/`Removed`/`Pruned` item
    /// is reset to `Queued` (this is the retry / re-add path). New items are
    /// appended as `Queued`. Returns any transfers the new capacity allows.
    pub fn enqueue(&mut self, media_item_id: &str, part_key: &str) -> Vec<DownloadEvent> {
        match self.state_of(media_item_id) {
            Some(DownloadState::Queued)
            | Some(DownloadState::Downloading)
            | Some(DownloadState::Paused)
            | Some(DownloadState::Completed) => {
                // Already present and not in a re-addable state: no-op.
            }
            Some(_) => {
                // Failed / Removed / Pruned -> reset to Queued (retry / re-add).
                if let Some(item) = self.find_mut(media_item_id) {
                    item.state = DownloadState::Queued;
                }
            }
            None => {
                self.items.push(QueueItem {
                    media_item_id: media_item_id.to_string(),
                    part_key: part_key.to_string(),
                    state: DownloadState::Queued,
                });
            }
        }
        self.schedule()
    }

    /// Mark a transfer complete and schedule the next queued item(s).
    pub fn mark_completed(&mut self, media_item_id: &str) -> Vec<DownloadEvent> {
        if let Some(item) = self.find_mut(media_item_id) {
            item.state = DownloadState::Completed;
        }
        self.schedule()
    }

    /// Mark a transfer failed and schedule the next queued item(s). The failure
    /// reason is persisted by the runner; the tracker only frees the slot.
    pub fn mark_failed(&mut self, media_item_id: &str) -> Vec<DownloadEvent> {
        if let Some(item) = self.find_mut(media_item_id) {
            item.state = DownloadState::Failed;
        }
        self.schedule()
    }

    /// Pause an item. A `Downloading` item is cancelled (its `.part` is kept)
    /// and the freed slot is rescheduled; a `Queued` item simply parks.
    pub fn pause(&mut self, media_item_id: &str) -> Vec<DownloadEvent> {
        let mut events = Vec::new();
        let was_downloading = self.state_of(media_item_id) == Some(DownloadState::Downloading);
        if let Some(item) = self.find_mut(media_item_id)
            && matches!(
                item.state,
                DownloadState::Downloading | DownloadState::Queued
            )
        {
            item.state = DownloadState::Paused;
        }
        if was_downloading {
            events.push(DownloadEvent::CancelTransfer {
                media_item_id: media_item_id.to_string(),
            });
        }
        events.extend(self.schedule());
        events
    }

    /// Resume a `Paused` (or `Failed`) item back to `Queued` and reschedule.
    pub fn resume(&mut self, media_item_id: &str) -> Vec<DownloadEvent> {
        if let Some(item) = self.find_mut(media_item_id)
            && matches!(item.state, DownloadState::Paused | DownloadState::Failed)
        {
            item.state = DownloadState::Queued;
        }
        self.schedule()
    }

    /// Cancel/delete an item. An in-flight transfer is cancelled; the item is
    /// marked `Removed`. The freed slot is rescheduled.
    pub fn cancel(&mut self, media_item_id: &str) -> Vec<DownloadEvent> {
        let mut events = Vec::new();
        let was_downloading = self.state_of(media_item_id) == Some(DownloadState::Downloading);
        if let Some(item) = self.find_mut(media_item_id) {
            item.state = DownloadState::Removed;
        }
        if was_downloading {
            events.push(DownloadEvent::CancelTransfer {
                media_item_id: media_item_id.to_string(),
            });
        }
        events.extend(self.schedule());
        events
    }

    /// Crash recovery: a `Downloading` row has no running task after a restart,
    /// so reset it to `Queued` for re-validation. Does NOT emit events — the
    /// runner calls [`schedule`] after `verify_downloads` so the first schedule
    /// emits exactly `concurrency` starts.
    pub fn recover_on_startup(&mut self) {
        for item in &mut self.items {
            if item.state == DownloadState::Downloading {
                item.state = DownloadState::Queued;
            }
        }
    }

    /// Reorder a `Queued` item to a new position among the items. No-op for
    /// non-`Queued` items (reorder is not preemptive — a `Downloading` item
    /// keeps its slot). Reordering changes which queued item starts next.
    pub fn reorder(&mut self, media_item_id: &str, new_index: usize) {
        let Some(cur) = self
            .items
            .iter()
            .position(|i| i.media_item_id == media_item_id)
        else {
            return;
        };
        if self.items[cur].state != DownloadState::Queued {
            return;
        }
        let item = self.items.remove(cur);
        let idx = new_index.min(self.items.len());
        self.items.insert(idx, item);
    }

    /// Emit `StartTransfer` for queued items until the concurrency limit is
    /// reached, transitioning each started item to `Downloading`.
    pub fn schedule(&mut self) -> Vec<DownloadEvent> {
        let mut events = Vec::new();
        let mut slots = self.concurrency.saturating_sub(self.active_count());
        for item in &mut self.items {
            if slots == 0 {
                break;
            }
            if item.state == DownloadState::Queued {
                item.state = DownloadState::Downloading;
                events.push(DownloadEvent::StartTransfer {
                    media_item_id: item.media_item_id.clone(),
                    part_key: item.part_key.clone(),
                });
                slots -= 1;
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started_ids(events: &[DownloadEvent]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                DownloadEvent::StartTransfer { media_item_id, .. } => Some(media_item_id.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn enqueue_respects_concurrency_limit() {
        let mut q = DownloadQueue::new(2);
        let mut started = Vec::new();
        for i in 0..5 {
            let id = format!("m{i}");
            started.extend(started_ids(&q.enqueue(&id, "/p")));
        }
        // Exactly 2 transfers started; the rest stay queued.
        assert_eq!(started.len(), 2);
        assert_eq!(started, vec!["m0", "m1"]);
        assert_eq!(q.active_count(), 2);
        assert_eq!(q.state_of("m2"), Some(DownloadState::Queued));
    }

    #[test]
    fn enqueue_completed_item_is_noop() {
        let mut q = DownloadQueue::new(2);
        q.enqueue("m1", "/p");
        q.mark_completed("m1");
        let events = q.enqueue("m1", "/p");
        assert!(events.is_empty());
        assert_eq!(q.state_of("m1"), Some(DownloadState::Completed));
        assert_eq!(q.items().len(), 1); // no duplicate
    }

    #[test]
    fn enqueue_failed_item_resets_to_queued_and_retries() {
        let mut q = DownloadQueue::new(1);
        q.enqueue("m1", "/p"); // -> Downloading
        q.mark_failed("m1"); // -> Failed
        assert_eq!(q.state_of("m1"), Some(DownloadState::Failed));
        let events = q.enqueue("m1", "/p"); // retry
        assert_eq!(started_ids(&events), vec!["m1"]);
        assert_eq!(q.state_of("m1"), Some(DownloadState::Downloading));
    }

    #[test]
    fn completion_schedules_next_queued() {
        let mut q = DownloadQueue::new(1);
        q.enqueue("m1", "/p"); // m1 downloading
        q.enqueue("m2", "/p"); // m2 queued
        assert_eq!(q.state_of("m2"), Some(DownloadState::Queued));
        let events = q.mark_completed("m1");
        assert_eq!(started_ids(&events), vec!["m2"]);
    }

    #[test]
    fn pause_active_frees_slot_and_cancels() {
        let mut q = DownloadQueue::new(1);
        q.enqueue("m1", "/p"); // downloading
        q.enqueue("m2", "/p"); // queued
        let events = q.pause("m1");
        // Pausing m1 cancels its transfer and starts m2.
        assert!(events.contains(&DownloadEvent::CancelTransfer {
            media_item_id: "m1".to_string()
        }));
        assert_eq!(started_ids(&events), vec!["m2"]);
        assert_eq!(q.state_of("m1"), Some(DownloadState::Paused));
    }

    #[test]
    fn resume_requeues_and_schedules_when_slot_free() {
        let mut q = DownloadQueue::new(1);
        q.enqueue("m1", "/p");
        q.pause("m1"); // paused, no active
        let events = q.resume("m1");
        assert_eq!(started_ids(&events), vec!["m1"]);
        assert_eq!(q.state_of("m1"), Some(DownloadState::Downloading));
    }

    #[test]
    fn cancel_active_emits_cancel_and_marks_removed() {
        let mut q = DownloadQueue::new(2);
        q.enqueue("m1", "/p");
        let events = q.cancel("m1");
        assert!(events.contains(&DownloadEvent::CancelTransfer {
            media_item_id: "m1".to_string()
        }));
        assert_eq!(q.state_of("m1"), Some(DownloadState::Removed));
    }

    #[test]
    fn recover_on_startup_then_schedule_emits_exactly_concurrency() {
        // Simulate a crash: 3 rows left as Downloading, concurrency 2.
        let items = vec![
            QueueItem {
                media_item_id: "m1".into(),
                part_key: "/p".into(),
                state: DownloadState::Downloading,
            },
            QueueItem {
                media_item_id: "m2".into(),
                part_key: "/p".into(),
                state: DownloadState::Downloading,
            },
            QueueItem {
                media_item_id: "m3".into(),
                part_key: "/p".into(),
                state: DownloadState::Downloading,
            },
        ];
        let mut q = DownloadQueue::from_items(items, 2);
        q.recover_on_startup();
        // All reset to Queued, none auto-resumed yet.
        assert_eq!(q.active_count(), 0);
        assert!(q.items().iter().all(|i| i.state == DownloadState::Queued));
        let events = q.schedule();
        // First schedule emits exactly `concurrency` (2) starts.
        assert_eq!(started_ids(&events), vec!["m1", "m2"]);
        assert_eq!(q.state_of("m3"), Some(DownloadState::Queued));
    }

    #[test]
    fn reorder_changes_start_order_of_queued() {
        let mut q = DownloadQueue::new(1);
        q.enqueue("m1", "/p"); // downloading
        q.enqueue("m2", "/p"); // queued
        q.enqueue("m3", "/p"); // queued
        // Move m3 ahead of m2.
        q.reorder("m3", 1);
        let events = q.mark_completed("m1");
        assert_eq!(started_ids(&events), vec!["m3"]);
    }

    #[test]
    fn reorder_ignores_downloading_item() {
        let mut q = DownloadQueue::new(1);
        q.enqueue("m1", "/p"); // downloading
        q.enqueue("m2", "/p"); // queued
        let before: Vec<String> = q.items().iter().map(|i| i.media_item_id.clone()).collect();
        q.reorder("m1", 5); // m1 is Downloading -> no-op
        let after: Vec<String> = q.items().iter().map(|i| i.media_item_id.clone()).collect();
        assert_eq!(before, after);
        assert_eq!(q.state_of("m1"), Some(DownloadState::Downloading));
    }

    #[test]
    fn cancel_from_queued_state_removes_without_cancel_event() {
        let mut q = DownloadQueue::new(1);
        q.enqueue("m1", "/p"); // downloading
        q.enqueue("m2", "/p"); // queued
        let events = q.cancel("m2");
        // m2 was only queued -> no CancelTransfer needed.
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, DownloadEvent::CancelTransfer { .. }))
        );
        assert_eq!(q.state_of("m2"), Some(DownloadState::Removed));
    }
}
